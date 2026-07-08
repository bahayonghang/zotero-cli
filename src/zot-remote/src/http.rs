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

use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
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

/// Map a `reqwest::Error` (network/transport failure) to `ZotError::Remote`,
/// preserving the operation `code` and any HTTP status the error carries. This
/// is the single shared error-mapper for every remote client.
pub(crate) fn remote_err(code: &'static str) -> impl Fn(reqwest::Error) -> ZotError {
    move |err| ZotError::Remote {
        code: code.to_string(),
        message: err.to_string(),
        hint: http_hint(err.status()),
        status: err.status().map(|status| status.as_u16()),
    }
}

/// Map selected HTTP statuses to actionable hints shared across remote clients.
pub(crate) fn http_hint(status: Option<StatusCode>) -> Option<String> {
    match status {
        Some(StatusCode::FORBIDDEN) => Some("Check that the API key has write access".to_string()),
        Some(StatusCode::PRECONDITION_FAILED) => {
            Some("Object changed remotely; re-fetch before retrying".to_string())
        }
        Some(StatusCode::PRECONDITION_REQUIRED) => {
            Some("Missing version or If-Match precondition".to_string())
        }
        Some(StatusCode::CONFLICT) => Some("The target library is locked".to_string()),
        _ => None,
    }
}

/// Return the response when the status is success, otherwise consume the body
/// and map it to a `ZotError::Remote` carrying the HTTP status and remote body.
pub(crate) async fn ensure_status(response: Response, code: &str) -> ZotResult<Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(ZotError::Remote {
            code: code.to_string(),
            message: format!("Request failed with status {}: {body}", status.as_u16()),
            hint: http_hint(Some(status)),
            status: Some(status.as_u16()),
        })
    }
}

/// Ensure success then decode the JSON body into `T`, mapping decode failures
/// to `ZotError::Remote`.
pub(crate) async fn read_json<T: DeserializeOwned>(response: Response, code: &str) -> ZotResult<T> {
    let response = ensure_status(response, code).await?;
    response.json::<T>().await.map_err(|err| ZotError::Remote {
        code: code.to_string(),
        message: err.to_string(),
        hint: http_hint(err.status()),
        status: err.status().map(|status| status.as_u16()),
    })
}

/// Ensure success and discard the body; used for write requests that return no
/// meaningful payload.
pub(crate) async fn ensure_empty(response: Response, code: &str) -> ZotResult<()> {
    ensure_status(response, code).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{http_hint, remote_err};
    use reqwest::StatusCode;
    use zot_core::ZotError;

    #[test]
    fn http_hint_maps_known_statuses() {
        assert_eq!(
            http_hint(Some(StatusCode::FORBIDDEN)).as_deref(),
            Some("Check that the API key has write access")
        );
        assert_eq!(
            http_hint(Some(StatusCode::PRECONDITION_FAILED)).as_deref(),
            Some("Object changed remotely; re-fetch before retrying")
        );
        assert_eq!(
            http_hint(Some(StatusCode::PRECONDITION_REQUIRED)).as_deref(),
            Some("Missing version or If-Match precondition")
        );
        assert_eq!(
            http_hint(Some(StatusCode::CONFLICT)).as_deref(),
            Some("The target library is locked")
        );
    }

    #[test]
    fn http_hint_returns_none_for_unmapped_or_absent_status() {
        assert_eq!(http_hint(Some(StatusCode::NOT_FOUND)), None);
        assert_eq!(http_hint(Some(StatusCode::INTERNAL_SERVER_ERROR)), None);
        assert_eq!(http_hint(None), None);
    }

    #[test]
    fn maps_status_less_error_preserving_code() {
        // A relative URL fails to build offline, yielding a status-less
        // `reqwest::Error`; the mapper must keep the operation code and derive
        // no hint or status from it.
        let err = reqwest::Client::new()
            .get("not-a-valid-url")
            .build()
            .expect_err("relative url should fail to build");
        match remote_err("test-code")(err) {
            ZotError::Remote {
                code, status, hint, ..
            } => {
                assert_eq!(code, "test-code");
                assert_eq!(status, None);
                assert_eq!(hint, None);
            }
            other => panic!("expected Remote, got {other:?}"),
        }
    }
}
