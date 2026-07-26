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

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use reqwest::header::RETRY_AFTER;
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use zot_core::net::{CONNECT_TIMEOUT, REQUEST_TIMEOUT, USER_AGENT};
use zot_core::{ZotError, ZotResult};

const MAX_ATTEMPTS: usize = 3;
const MAX_RETRY_DELAY: Duration = Duration::from_secs(5);
const BASE_BACKOFF: Duration = Duration::from_millis(100);
const MAX_JITTER_MILLIS: u64 = 50;
const MAX_ERROR_BODY_BYTES: usize = 4 * 1024;
const WRITE_TOKEN_HEADER: &str = "zotero-write-token";

/// Shared HTTP runtime. Cloning yields a new handle backed by the same
/// connection pool (see `reqwest::Client::clone`).
#[derive(Clone, Debug)]
pub struct HttpRuntime {
    client: reqwest::Client,
    download_client: reqwest::Client,
}

impl HttpRuntime {
    /// Build a runtime with sensible defaults: 15s connect timeout, 60s
    /// request timeout, identifying User-Agent.
    pub fn new() -> ZotResult<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| ZotError::Remote {
                code: "http-runtime-build".to_string(),
                message: err.to_string(),
                hint: None,
                status: None,
            })?;
        let download_client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| ZotError::Remote {
                code: "http-runtime-build".to_string(),
                message: err.to_string(),
                hint: None,
                status: None,
            })?;
        Ok(Self {
            client,
            download_client,
        })
    }

    /// Borrow the underlying client for request dispatch.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Cheap clone of the client (reqwest internally Arc-wraps state).
    pub fn client_clone(&self) -> reqwest::Client {
        self.client.clone()
    }

    /// Borrow the client whose automatic redirect policy is disabled. Callers
    /// must validate each redirect target before following it.
    pub(crate) fn download_client(&self) -> &reqwest::Client {
        &self.download_client
    }
}

impl Default for HttpRuntime {
    /// Fall back to an unconfigured client if builder fails. In practice this
    /// path only runs if `reqwest::Client::builder()` errors, which requires a
    /// broken TLS backend — vanishingly rare.
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            client: reqwest::Client::new(),
            download_client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        })
    }
}

/// Send a request with the shared retry contract. Only GET requests and
/// requests carrying a Zotero write token are replayed.
pub(crate) async fn send_with_retry(
    request: RequestBuilder,
    code: &'static str,
) -> ZotResult<Response> {
    if !request_is_retryable(&request) {
        return request.send().await.map_err(remote_err(code));
    }

    for attempt in 0..MAX_ATTEMPTS {
        let Some(current) = request.try_clone() else {
            return request.send().await.map_err(remote_err(code));
        };
        match current.send().await {
            Ok(response) => {
                if !retryable_status(response.status()) || attempt + 1 == MAX_ATTEMPTS {
                    return Ok(response);
                }
                let delay = retry_delay(response.headers().get(RETRY_AFTER), attempt);
                drop(response);
                tokio::time::sleep(delay).await;
            }
            Err(err) if attempt + 1 < MAX_ATTEMPTS => {
                tokio::time::sleep(retry_delay(None, attempt)).await;
                drop(err);
            }
            Err(err) => return Err(remote_err(code)(err)),
        }
    }

    Err(ZotError::Remote {
        code: code.to_string(),
        message: "Retry attempts exhausted".to_string(),
        hint: None,
        status: None,
    })
}

fn request_is_retryable(request: &RequestBuilder) -> bool {
    request
        .try_clone()
        .and_then(|clone| clone.build().ok())
        .is_some_and(|request| {
            request.method() == Method::GET || request.headers().contains_key(WRITE_TOKEN_HEADER)
        })
}

fn retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn retry_delay(retry_after: Option<&reqwest::header::HeaderValue>, attempt: usize) -> Duration {
    if let Some(delay) = retry_after.and_then(parse_retry_after) {
        return delay.min(MAX_RETRY_DELAY);
    }

    let exponent = u32::try_from(attempt).unwrap_or(u32::MAX).min(6);
    let backoff = BASE_BACKOFF
        .saturating_mul(2_u32.saturating_pow(exponent))
        .min(MAX_RETRY_DELAY);
    let jitter = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
        % (MAX_JITTER_MILLIS + 1);
    backoff
        .saturating_add(Duration::from_millis(jitter))
        .min(MAX_RETRY_DELAY)
}

fn parse_retry_after(value: &reqwest::header::HeaderValue) -> Option<Duration> {
    let value = value.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let retry_at = DateTime::parse_from_rfc2822(value)
        .ok()?
        .with_timezone(&Utc);
    let millis = retry_at
        .signed_duration_since(Utc::now())
        .num_milliseconds();
    Some(Duration::from_millis(
        u64::try_from(millis).unwrap_or_default(),
    ))
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
        let body = bounded_error_body(response).await;
        let suffix = if body.is_empty() {
            String::new()
        } else {
            format!(": {body}")
        };
        Err(ZotError::Remote {
            code: code.to_string(),
            message: format!("Request failed with status {}{suffix}", status.as_u16()),
            hint: http_hint(Some(status)),
            status: Some(status.as_u16()),
        })
    }
}

async fn bounded_error_body(mut response: Response) -> String {
    let mut bytes = Vec::with_capacity(MAX_ERROR_BODY_BYTES);
    let mut truncated = response
        .content_length()
        .is_some_and(|length| length > MAX_ERROR_BODY_BYTES as u64);
    while bytes.len() < MAX_ERROR_BODY_BYTES {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(_) => {
                truncated = true;
                break;
            }
        };
        let remaining = MAX_ERROR_BODY_BYTES - bytes.len();
        if chunk.len() > remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    if !truncated && bytes.len() == MAX_ERROR_BODY_BYTES {
        truncated = match response.chunk().await {
            Ok(Some(_)) | Err(_) => true,
            Ok(None) => false,
        };
    }

    let mut sanitized = String::new();
    let mut previous_space = true;
    for character in String::from_utf8_lossy(&bytes).chars() {
        if character.is_whitespace() {
            if !previous_space && sanitized.len() < MAX_ERROR_BODY_BYTES {
                sanitized.push(' ');
                previous_space = true;
            }
        } else if !character.is_control()
            && sanitized.len() + character.len_utf8() <= MAX_ERROR_BODY_BYTES
        {
            sanitized.push(character);
            previous_space = false;
        } else if sanitized.len() + character.len_utf8() > MAX_ERROR_BODY_BYTES {
            truncated = true;
            break;
        }
    }
    let mut sanitized = sanitized.trim().to_string();
    if truncated {
        if !sanitized.is_empty() {
            sanitized.push(' ');
        }
        sanitized.push_str("[truncated]");
    }
    sanitized
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
    use std::time::Duration;

    use super::{http_hint, parse_retry_after, remote_err};
    use reqwest::StatusCode;
    use reqwest::header::HeaderValue;
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

    #[test]
    fn parses_retry_after_seconds_and_http_date() {
        assert_eq!(
            parse_retry_after(&HeaderValue::from_static("2")),
            Some(Duration::from_secs(2))
        );
        let future = (chrono::Utc::now() + chrono::Duration::minutes(2)).to_rfc2822();
        let future = HeaderValue::from_bytes(future.as_bytes()).expect("valid future HTTP date");
        assert!(parse_retry_after(&future).is_some_and(|delay| delay > Duration::from_secs(1)));
        assert_eq!(
            parse_retry_after(&HeaderValue::from_static("invalid")),
            None
        );
    }
}
