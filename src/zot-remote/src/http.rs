//! Shared HTTP runtime for remote clients.
//!
//! `reqwest::Client` internally holds an Arc'd connection pool and TLS
//! resolver, so cloning is cheap. Constructing one per remote client means
//! each `ZoteroRemote`, `OaClient`, `SciteClient`, etc. warms its own
//! pool/TLS resolver from scratch on first request, and re-does the work
//! per process (worse under MCP server mode). `HttpRuntime` solves that by
//! building a single pre-warmed `reqwest::Client` and handing out cheap
//! clones; per-request headers (e.g. Zotero's API key) are attached by the
//! individual clients when they issue the request.

use std::time::Duration;

use zot_core::{ZotError, ZotResult};

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const USER_AGENT: &str = concat!("zot-cli/", env!("CARGO_PKG_VERSION"));

/// Shared HTTP runtime. Cloning yields a new handle backed by the same
/// connection pool (see `reqwest::Client::clone`).
#[derive(Clone, Debug)]
pub struct HttpRuntime {
    client: reqwest::Client,
}

impl HttpRuntime {
    /// Build a runtime with sensible defaults: 15s connect timeout, 60s
    /// request timeout, identifying User-Agent.
    pub fn new() -> ZotResult<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| ZotError::Remote {
                code: "http-runtime-build".to_string(),
                message: err.to_string(),
                hint: None,
                status: None,
            })?;
        Ok(Self { client })
    }

    /// Borrow the underlying client for request dispatch.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Cheap clone of the client (reqwest internally Arc-wraps state).
    pub fn client_clone(&self) -> reqwest::Client {
        self.client.clone()
    }
}

impl Default for HttpRuntime {
    /// Fall back to an unconfigured client if builder fails. In practice this
    /// path only runs if `reqwest::Client::builder()` errors, which requires a
    /// broken TLS backend — vanishingly rare.
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            client: reqwest::Client::new(),
        })
    }
}
