use std::fmt;
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use reqwest::{StatusCode, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;
use zot_core::net::USER_AGENT;
use zot_core::{ZotError, ZotResult};

use crate::model::{
    BRIDGE_PROTOCOL_VERSION, BaseRequest, BridgeEnvelope, BridgeHealth, BridgePairResult,
    BridgeRevokeResult, BridgeStatus, ClientIdentity, LocalHttpStatus, PairRequest,
};

pub const BRIDGE_BASE_URL: &str = "http://127.0.0.1:23119/zot-bridge/v1/";
const LOCAL_API_URL: &str = "http://127.0.0.1:23119/api/";
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct DesktopClient {
    client: reqwest::Client,
    base_url: Url,
    local_api_url: Url,
    request_timeout: Duration,
}

impl fmt::Debug for DesktopClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DesktopClient")
            .field("base_url", &self.base_url)
            .field("local_api_url", &self.local_api_url)
            .field("request_timeout", &self.request_timeout)
            .finish_non_exhaustive()
    }
}

impl DesktopClient {
    pub fn new() -> ZotResult<Self> {
        Self::build(BRIDGE_BASE_URL, LOCAL_API_URL, Duration::from_secs(10))
    }

    fn build(base_url: &str, local_api_url: &str, request_timeout: Duration) -> ZotResult<Self> {
        let base_url = parse_loopback_url(base_url, "bridge-base-url")?;
        let local_api_url = parse_loopback_url(local_api_url, "local-api-url")?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| bridge_error("bridge-client", err.to_string(), None, None))?;
        Ok(Self {
            client,
            base_url,
            local_api_url,
            request_timeout,
        })
    }

    #[cfg(test)]
    fn with_base_url(base_url: &str, request_timeout: Duration) -> ZotResult<Self> {
        Self::build(base_url, base_url, request_timeout)
    }

    pub async fn probe_local_http(&self) -> ZotResult<LocalHttpStatus> {
        let response = self
            .client
            .get(self.local_api_url.clone())
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(status_error(response.status(), None));
        }
        Ok(LocalHttpStatus {
            available: true,
            zotero_version: header_value(&response, "x-zotero-version"),
            connector_api_version: header_value(&response, "x-zotero-connector-api-version"),
        })
    }

    pub async fn health(&self) -> ZotResult<BridgeHealth> {
        let request = self.base_request();
        let request_id = request.request_id.clone();
        let health: BridgeHealth = self.post("health", &request, None, &request_id).await?;
        if health.protocol_version != BRIDGE_PROTOCOL_VERSION {
            return Err(protocol_error(health.protocol_version));
        }
        Ok(health)
    }

    pub async fn pair(&self, code: &str, client_instance_id: &str) -> ZotResult<BridgePairResult> {
        let request = PairRequest {
            base: self.base_request(),
            code,
            client_instance_id,
        };
        let request_id = request.base.request_id.clone();
        let pair: BridgePairResult = self.post("pair", &request, None, &request_id).await?;
        if pair.protocol_version != BRIDGE_PROTOCOL_VERSION {
            return Err(protocol_error(pair.protocol_version));
        }
        Ok(pair)
    }

    pub async fn status(&self, token: &str) -> ZotResult<BridgeStatus> {
        let request = self.base_request();
        let request_id = request.request_id.clone();
        self.post("status", &request, Some(token), &request_id)
            .await
    }

    pub async fn revoke(&self, token: &str) -> ZotResult<BridgeRevokeResult> {
        let request = self.base_request();
        let request_id = request.request_id.clone();
        self.post("auth/revoke", &request, Some(token), &request_id)
            .await
    }

    async fn post<T, B>(
        &self,
        path: &str,
        body: &B,
        token: Option<&str>,
        expected_request_id: &str,
    ) -> ZotResult<T>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let url = self
            .base_url
            .join(path)
            .map_err(|err| bridge_error("bridge-url", err.to_string(), None, None))?;
        let mut request = self
            .client
            .post(url)
            .timeout(self.request_timeout)
            .json(body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.map_err(map_request_error)?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(map_request_error)?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(bridge_error(
                "bridge-response-too-large",
                "Desktop bridge response exceeded 64 KiB".to_string(),
                Some("Update or reinstall the Zotero bridge plugin".to_string()),
                Some(status.as_u16()),
            ));
        }
        let envelope = match serde_json::from_slice::<BridgeEnvelope<T>>(&bytes) {
            Ok(envelope) => envelope,
            Err(_) if !status.is_success() => return Err(status_error(status, None)),
            Err(_) => {
                return Err(bridge_error(
                    "bridge-invalid-response",
                    "Desktop bridge returned invalid JSON".to_string(),
                    Some("Update or reinstall the Zotero bridge plugin".to_string()),
                    Some(status.as_u16()),
                ));
            }
        };
        if !status.is_success() || !envelope.ok {
            if let Some(error) = envelope.error {
                return Err(bridge_error(
                    &error.code,
                    error.message,
                    error.hint,
                    Some(status.as_u16()),
                ));
            }
            return Err(status_error(status, Some("Desktop bridge request failed")));
        }
        let meta = envelope.meta.ok_or_else(|| {
            bridge_error(
                "bridge-invalid-response",
                "Desktop bridge response is missing protocol metadata".to_string(),
                None,
                Some(status.as_u16()),
            )
        })?;
        if meta.protocol_version != BRIDGE_PROTOCOL_VERSION {
            return Err(bridge_error(
                "bridge-protocol",
                format!(
                    "Desktop bridge protocol {} is incompatible with client protocol {}",
                    meta.protocol_version, BRIDGE_PROTOCOL_VERSION
                ),
                Some("Update zot or reinstall the matching bridge plugin".to_string()),
                Some(status.as_u16()),
            ));
        }
        if meta.request_id != expected_request_id {
            return Err(bridge_error(
                "bridge-response-mismatch",
                "Desktop bridge response request_id did not match the request".to_string(),
                Some("Retry the operation; update the plugin if the error persists".to_string()),
                Some(status.as_u16()),
            ));
        }
        let _ = (meta.plugin_version, meta.zotero_version);
        envelope.data.ok_or_else(|| {
            bridge_error(
                "bridge-invalid-response",
                "Desktop bridge response is missing data".to_string(),
                None,
                Some(status.as_u16()),
            )
        })
    }

    fn base_request(&self) -> BaseRequest<'static> {
        BaseRequest {
            protocol_version: BRIDGE_PROTOCOL_VERSION,
            request_id: Uuid::new_v4().to_string(),
            sent_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            client: ClientIdentity {
                name: "zot",
                version: env!("CARGO_PKG_VERSION"),
            },
        }
    }
}

fn parse_loopback_url(value: &str, code: &str) -> ZotResult<Url> {
    let url = Url::parse(value).map_err(|err| bridge_error(code, err.to_string(), None, None))?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"));
    if url.scheme() != "http" || !loopback {
        return Err(bridge_error(
            code,
            "Desktop bridge URL must use HTTP on a loopback host".to_string(),
            None,
            None,
        ));
    }
    Ok(url)
}

fn header_value(response: &reqwest::Response, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn map_request_error(err: reqwest::Error) -> ZotError {
    if err.is_timeout() {
        bridge_error(
            "bridge-timeout",
            "Desktop bridge request timed out".to_string(),
            Some("Ensure Zotero is running and responsive".to_string()),
            err.status().map(|status| status.as_u16()),
        )
    } else if err.is_connect() {
        bridge_error(
            "bridge-unreachable",
            "Could not connect to the Zotero desktop service".to_string(),
            Some("Start Zotero and run `zot bridge status`".to_string()),
            err.status().map(|status| status.as_u16()),
        )
    } else {
        bridge_error(
            "bridge-request",
            "Desktop bridge request failed".to_string(),
            Some("Run `zot bridge status` for diagnostics".to_string()),
            err.status().map(|status| status.as_u16()),
        )
    }
}

fn status_error(status: StatusCode, message: Option<&str>) -> ZotError {
    let (code, hint) = match status {
        StatusCode::UNAUTHORIZED => (
            "bridge-auth",
            Some("Run `zot bridge pair <code>` to authorize this profile".to_string()),
        ),
        StatusCode::NOT_FOUND => (
            "bridge-not-installed",
            Some("Run `zot bridge setup`, install the XPI, and restart Zotero".to_string()),
        ),
        _ => (
            "bridge-http",
            Some("Run `zot bridge status` for diagnostics".to_string()),
        ),
    };
    bridge_error(
        code,
        message
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("Desktop bridge returned HTTP {}", status.as_u16())),
        hint,
        Some(status.as_u16()),
    )
}

fn bridge_error(
    code: &str,
    message: String,
    hint: Option<String>,
    status: Option<u16>,
) -> ZotError {
    ZotError::DesktopBridge {
        code: code.to_string(),
        message,
        hint,
        status,
    }
}

fn protocol_error(actual: u32) -> ZotError {
    bridge_error(
        "bridge-protocol",
        format!(
            "Desktop bridge protocol {actual} is incompatible with client protocol {BRIDGE_PROTOCOL_VERSION}"
        ),
        Some("Update zot or reinstall the matching bridge plugin".to_string()),
        None,
    )
}

#[cfg(test)]
mod tests {
    use std::thread::JoinHandle;

    use tiny_http::{Header, Response, Server};

    use super::*;

    struct Captured {
        path: String,
        authorization: Option<String>,
        body: String,
    }

    type FakeResponse = (u16, String, Vec<(&'static str, &'static str)>);

    fn spawn_server(responses: Vec<FakeResponse>) -> (String, JoinHandle<Vec<Captured>>) {
        let server = Server::http(("127.0.0.1", 0)).expect("bind fake server");
        let port = server.server_addr().to_ip().expect("fake server IP").port();
        let base = format!("http://127.0.0.1:{port}/");
        let handle = std::thread::spawn(move || {
            let mut captured = Vec::new();
            for (status, body, headers) in responses {
                let mut request = server.recv().expect("receive request");
                let mut request_body = String::new();
                request
                    .as_reader()
                    .read_to_string(&mut request_body)
                    .expect("read body");
                captured.push(Captured {
                    path: request.url().to_string(),
                    authorization: request
                        .headers()
                        .iter()
                        .find(|header| header.field.equiv("authorization"))
                        .map(|header| header.value.to_string()),
                    body: request_body,
                });
                let request_id = captured
                    .last()
                    .and_then(|value| serde_json::from_str::<serde_json::Value>(&value.body).ok())
                    .and_then(|value| value["request_id"].as_str().map(ToOwned::to_owned))
                    .unwrap_or_else(|| "req-1".to_string());
                let response_body = body.replace("__REQUEST_ID__", &request_id);
                let mut response = Response::from_string(response_body).with_status_code(status);
                for (name, value) in headers {
                    response.add_header(
                        Header::from_bytes(name.as_bytes(), value.as_bytes())
                            .expect("valid header"),
                    );
                }
                request.respond(response).expect("respond");
            }
            captured
        });
        (base, handle)
    }

    fn envelope(data: serde_json::Value, protocol: u32) -> String {
        serde_json::json!({
            "ok": true,
            "data": data,
            "meta": {
                "request_id": "__REQUEST_ID__",
                "protocol_version": protocol,
                "plugin_version": "0.6.0",
                "zotero_version": "9.0.6"
            }
        })
        .to_string()
    }

    #[tokio::test]
    async fn health_and_local_probe_decode_headers() {
        let responses = vec![
            (
                200,
                envelope(
                    serde_json::json!({
                        "plugin_version": "0.6.0",
                        "zotero_version": "9.0.6",
                        "protocol_version": 1,
                        "capabilities": ["status"]
                    }),
                    1,
                ),
                vec![],
            ),
            (
                200,
                "Nothing to see here".to_string(),
                vec![
                    ("X-Zotero-Version", "9.0.6"),
                    ("X-Zotero-Connector-API-Version", "3"),
                ],
            ),
        ];
        let (base, handle) = spawn_server(responses);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");

        let health = client.health().await.expect("health");
        assert_eq!(health.protocol_version, 1);
        let local = client.probe_local_http().await.expect("local probe");
        assert_eq!(local.zotero_version.as_deref(), Some("9.0.6"));

        let captured = handle.join().expect("join server");
        assert_eq!(captured[0].path, "/health");
        assert!(captured[0].body.contains("\"protocol_version\":1"));
    }

    #[tokio::test]
    async fn protected_calls_send_bearer_and_pair_debug_redacts_token() {
        let responses = vec![
            (
                200,
                envelope(
                    serde_json::json!({
                        "token": "super-secret-token",
                        "plugin_version": "0.6.0",
                        "protocol_version": 1
                    }),
                    1,
                ),
                vec![],
            ),
            (
                200,
                envelope(
                    serde_json::json!({
                        "paired": true,
                        "capabilities": [],
                        "libraries": []
                    }),
                    1,
                ),
                vec![],
            ),
            (
                200,
                envelope(serde_json::json!({ "revoked": true }), 1),
                vec![],
            ),
        ];
        let (base, handle) = spawn_server(responses);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");

        let pair = client.pair("PAIRCODE", "client-1").await.expect("pair");
        assert!(!format!("{pair:?}").contains("super-secret-token"));
        client.status(&pair.token).await.expect("status");
        let revoked = client.revoke(&pair.token).await.expect("revoke");
        assert!(revoked.revoked);

        let captured = handle.join().expect("join server");
        assert_eq!(
            captured[1].authorization.as_deref(),
            Some("Bearer super-secret-token")
        );
        assert_eq!(captured[2].path, "/auth/revoke");
        assert_eq!(
            captured[2].authorization.as_deref(),
            Some("Bearer super-secret-token")
        );
    }

    #[tokio::test]
    async fn maps_plugin_error_without_exposing_response_body() {
        let error = serde_json::json!({
            "ok": false,
            "error": {
                "code": "bridge-auth",
                "message": "Authorization failed",
                "hint": "Pair again"
            },
            "meta": {
                "request_id": "req-1",
                "protocol_version": 1,
                "plugin_version": "0.6.0",
                "zotero_version": "9.0.6"
            }
        })
        .to_string();
        let (base, handle) = spawn_server(vec![(401, error, vec![])]);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");
        let err = client.status("bad-token").await.expect_err("auth error");
        match err {
            ZotError::DesktopBridge { code, status, .. } => {
                assert_eq!(code, "bridge-auth");
                assert_eq!(status, Some(401));
            }
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn rejects_incompatible_protocol() {
        let (base, handle) = spawn_server(vec![(
            200,
            envelope(
                serde_json::json!({
                    "plugin_version": "0.6.0",
                    "zotero_version": "9.0.6",
                    "protocol_version": 2,
                    "capabilities": []
                }),
                2,
            ),
            vec![],
        )]);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");
        let err = client.health().await.expect_err("protocol mismatch");
        match err {
            ZotError::DesktopBridge { code, .. } => assert_eq!(code, "bridge-protocol"),
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn rejects_response_for_a_different_request_id() {
        let body = serde_json::json!({
            "ok": true,
            "data": {
                "plugin_version": "0.6.0",
                "zotero_version": "9.0.6",
                "protocol_version": 1,
                "capabilities": []
            },
            "meta": {
                "request_id": "different-request",
                "protocol_version": 1,
                "plugin_version": "0.6.0",
                "zotero_version": "9.0.6"
            }
        })
        .to_string();
        let (base, handle) = spawn_server(vec![(200, body, vec![])]);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");
        let err = client.health().await.expect_err("request mismatch");
        match err {
            ZotError::DesktopBridge { code, .. } => {
                assert_eq!(code, "bridge-response-mismatch")
            }
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn accepts_replayed_response_for_the_same_request() {
        let response = (
            200,
            envelope(
                serde_json::json!({
                    "paired": true,
                    "capabilities": [],
                    "libraries": []
                }),
                1,
            ),
            vec![],
        );
        let (base, handle) = spawn_server(vec![response.clone(), response]);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");
        let request = BaseRequest {
            protocol_version: BRIDGE_PROTOCOL_VERSION,
            request_id: "replay-request".to_string(),
            sent_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            client: ClientIdentity {
                name: "zot",
                version: "0.6.0",
            },
        };

        let first: BridgeStatus = client
            .post(
                "status",
                &request,
                Some("replay-token"),
                &request.request_id,
            )
            .await
            .expect("first status");
        let replay: BridgeStatus = client
            .post(
                "status",
                &request,
                Some("replay-token"),
                &request.request_id,
            )
            .await
            .expect("replayed status");
        assert_eq!(replay, first);

        let captured = handle.join().expect("join server");
        assert_eq!(captured[0].body, captured[1].body);
        assert_eq!(captured[0].authorization, captured[1].authorization);
    }

    #[tokio::test]
    async fn rejects_invalid_json_success_responses() {
        let (base, handle) = spawn_server(vec![(200, "not-json".to_string(), vec![])]);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");
        let err = client.health().await.expect_err("invalid JSON response");
        match err {
            ZotError::DesktopBridge { code, status, .. } => {
                assert_eq!(code, "bridge-invalid-response");
                assert_eq!(status, Some(200));
            }
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn maps_non_json_404_to_not_installed() {
        let (base, handle) = spawn_server(vec![(404, "Not Found".to_string(), vec![])]);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");
        let err = client.health().await.expect_err("not installed");
        match err {
            ZotError::DesktopBridge { code, status, .. } => {
                assert_eq!(code, "bridge-not-installed");
                assert_eq!(status, Some(404));
            }
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn rejects_oversized_responses() {
        let oversized = "x".repeat(MAX_RESPONSE_BYTES + 1);
        let (base, handle) = spawn_server(vec![(200, oversized, vec![])]);
        let client = DesktopClient::with_base_url(&base, Duration::from_secs(1)).expect("client");
        let err = client.health().await.expect_err("oversized response");
        match err {
            ZotError::DesktopBridge { code, .. } => {
                assert_eq!(code, "bridge-response-too-large")
            }
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn maps_slow_server_to_timeout() {
        let server = Server::http(("127.0.0.1", 0)).expect("bind fake server");
        let port = server.server_addr().to_ip().expect("fake server IP").port();
        let base = format!("http://127.0.0.1:{port}/");
        let handle = std::thread::spawn(move || {
            let request = server.recv().expect("receive request");
            std::thread::sleep(Duration::from_millis(100));
            let _ = request.respond(Response::from_string("late"));
        });
        let client =
            DesktopClient::with_base_url(&base, Duration::from_millis(10)).expect("client");
        let err = client.health().await.expect_err("timeout");
        match err {
            ZotError::DesktopBridge { code, .. } => assert_eq!(code, "bridge-timeout"),
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[test]
    fn rejects_non_loopback_urls() {
        let err = DesktopClient::build(
            "https://example.com/zot-bridge/v1/",
            LOCAL_API_URL,
            Duration::from_secs(1),
        )
        .expect_err("remote bridge URL");
        match err {
            ZotError::DesktopBridge { code, .. } => assert_eq!(code, "bridge-base-url"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn production_base_url_preserves_v1_prefix_when_joining() {
        let client = DesktopClient::new().expect("client");
        let health = client.base_url.join("health").expect("health URL");
        assert_eq!(health.path(), "/zot-bridge/v1/health");
    }
}
