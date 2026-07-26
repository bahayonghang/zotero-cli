//! Loopback-only client for Zotero's built-in connector HTTP server
//! (`/connector/*`) plus the adjacent read-only local API (`/api/*`). Both
//! servers ship with Zotero itself and need no plugin or pairing. Connector
//! writes are limited to importing new records into the library/collection
//! currently selected in the Zotero UI.

use std::fmt;
use std::time::Duration;

use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use zot_core::net::USER_AGENT;
use zot_core::{ZotError, ZotResult};

/// Default base URL for Zotero's built-in connector server. Overridable via
/// `ZOT_CONNECTOR_BASE_URL` for non-default setups.
pub const CONNECTOR_BASE_URL: &str = "http://127.0.0.1:23119/";
const CONNECTOR_API_VERSION_HEADER: &str = "X-Zotero-Connector-API-Version";
const CONNECTOR_API_VERSION: &str = "3";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Result of `GET /connector/ping`. `available` is only ever `true`: a
/// failed probe returns `Err` instead, matching `LocalHttpStatus`'s shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectorPing {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zotero_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalHttpStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zotero_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_api_version: Option<String>,
}

/// Result of `POST /connector/getSelectedCollection`: the library/collection
/// currently selected in the Zotero UI. Zotero omits `id`/`name` when the
/// target is not editable, so every field but `editable` is optional; a
/// missing `editable` defaults to `false` (fail closed rather than assume a
/// target is writable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectedTarget {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default, rename = "libraryID", alias = "library_id")]
    pub library_id: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub editable: bool,
    #[serde(default, rename = "libraryEditable", alias = "library_editable")]
    pub library_editable: Option<bool>,
}

impl SelectedTarget {
    /// Combine `editable` and `library_editable` into the single writability
    /// verdict callers gate imports on: blocked unless the collection itself
    /// is editable and the library was not explicitly reported read-only.
    pub fn is_writable(&self) -> bool {
        self.editable && self.library_editable != Some(false)
    }
}

/// Result of `POST /connector/import?session=<id>`. `response` is the raw
/// import response: Zotero returns a JSON array of imported items on
/// success, but a non-JSON body is preserved verbatim as a string so
/// nothing is lost on an unexpected shape.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConnectorImportResult {
    pub status: u16,
    pub session: String,
    pub response: serde_json::Value,
}

#[derive(Clone)]
pub struct ConnectorClient {
    client: reqwest::Client,
    base_url: Url,
    request_timeout: Duration,
}

impl fmt::Debug for ConnectorClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorClient")
            .field("base_url", &self.base_url)
            .field("request_timeout", &self.request_timeout)
            .finish_non_exhaustive()
    }
}

impl ConnectorClient {
    pub fn new() -> ZotResult<Self> {
        let base_url = std::env::var("ZOT_CONNECTOR_BASE_URL")
            .unwrap_or_else(|_| CONNECTOR_BASE_URL.to_string());
        Self::build(&base_url, REQUEST_TIMEOUT)
    }

    /// Construct a client pointed at an explicit base URL (fake server in
    /// tests), bypassing the `ZOT_CONNECTOR_BASE_URL` env override so
    /// parallel tests never race on process-global environment state.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn with_base_url_for_tests(base_url: &str, request_timeout: Duration) -> ZotResult<Self> {
        Self::build(base_url, request_timeout)
    }

    fn build(base_url: &str, request_timeout: Duration) -> ZotResult<Self> {
        let normalized = if base_url.ends_with('/') {
            base_url.to_string()
        } else {
            format!("{base_url}/")
        };
        let base_url = parse_loopback_url(&normalized)?;
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| connector_error("connector-client", err.to_string(), None, None))?;
        Ok(Self {
            client,
            base_url,
            request_timeout,
        })
    }

    /// `GET /connector/ping`: a cheap readiness probe. Used by `doctor` and
    /// as the first step of `item import`, so a closed Zotero fails fast
    /// with a clear `connector-unreachable` error before any target lookup.
    pub async fn ping(&self) -> ZotResult<ConnectorPing> {
        let url = self.join("connector/ping")?;
        let response = self
            .client
            .get(url)
            .header(CONNECTOR_API_VERSION_HEADER, CONNECTOR_API_VERSION)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        Ok(ConnectorPing {
            available: true,
            zotero_version: header_value(&response, "x-zotero-version"),
        })
    }

    /// Probe Zotero's built-in read-only local API. This shares the connector
    /// server's loopback-only client but does not perform a write.
    pub async fn probe_local_http(&self) -> ZotResult<LocalHttpStatus> {
        let url = self.join("api/")?;
        let response = self
            .client
            .get(url)
            .timeout(CONNECT_TIMEOUT)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        Ok(LocalHttpStatus {
            available: true,
            zotero_version: header_value(&response, "x-zotero-version"),
            connector_api_version: header_value(&response, "x-zotero-connector-api-version"),
        })
    }

    /// `POST /connector/getSelectedCollection`: the library/collection
    /// currently selected in the Zotero UI. Callers must check
    /// [`SelectedTarget::is_writable`] before calling [`Self::import`].
    pub async fn selected_target(&self) -> ZotResult<SelectedTarget> {
        let url = self.join("connector/getSelectedCollection")?;
        let response = self
            .client
            .post(url)
            .header(CONNECTOR_API_VERSION_HEADER, CONNECTOR_API_VERSION)
            .json(&serde_json::json!({}))
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(map_request_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(status));
        }
        response.json::<SelectedTarget>().await.map_err(|err| {
            connector_error(
                "connector-invalid-response",
                format!("Zotero connector returned an unexpected response: {err}"),
                None,
                Some(status.as_u16()),
            )
        })
    }

    /// `POST /connector/import?session=<session>`: import BibTeX/RIS `text`
    /// into whatever library/collection is currently selected in Zotero.
    /// Callers own the readonly-target gate; this method always sends the
    /// request.
    pub async fn import(&self, session: &str, text: &str) -> ZotResult<ConnectorImportResult> {
        let mut url = self.join("connector/import")?;
        url.query_pairs_mut().append_pair("session", session);
        let response = self
            .client
            .post(url)
            .header(CONNECTOR_API_VERSION_HEADER, CONNECTOR_API_VERSION)
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(text.to_string())
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(map_request_error)?;
        let status = response.status();
        let body = response.text().await.map_err(map_request_error)?;
        if !status.is_success() {
            return Err(status_error(status));
        }
        let parsed = serde_json::from_str::<serde_json::Value>(&body)
            .unwrap_or(serde_json::Value::String(body));
        Ok(ConnectorImportResult {
            status: status.as_u16(),
            session: session.to_string(),
            response: parsed,
        })
    }

    fn join(&self, path: &str) -> ZotResult<Url> {
        self.base_url
            .join(path)
            .map_err(|err| connector_error("connector-url", err.to_string(), None, None))
    }
}

fn parse_loopback_url(value: &str) -> ZotResult<Url> {
    let url = Url::parse(value)
        .map_err(|err| connector_error("connector-base-url", err.to_string(), None, None))?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"));
    if url.scheme() != "http" || !loopback {
        return Err(connector_error(
            "connector-base-url",
            "Connector URL must use HTTP on a loopback host".to_string(),
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
        connector_error(
            "connector-timeout",
            "Zotero connector request timed out".to_string(),
            Some("Ensure Zotero is running and responsive".to_string()),
            err.status().map(|status| status.as_u16()),
        )
    } else if err.is_connect() {
        connector_error(
            "connector-unreachable",
            "Could not connect to the Zotero connector server".to_string(),
            Some("Start Zotero, then retry".to_string()),
            err.status().map(|status| status.as_u16()),
        )
    } else {
        connector_error(
            "connector-request",
            "Zotero connector request failed".to_string(),
            Some("Ensure Zotero is running with the connector server available".to_string()),
            err.status().map(|status| status.as_u16()),
        )
    }
}

fn status_error(status: StatusCode) -> ZotError {
    connector_error(
        "connector-http",
        format!("Zotero connector returned HTTP {}", status.as_u16()),
        Some("Ensure Zotero is running and try again".to_string()),
        Some(status.as_u16()),
    )
}

fn connector_error(
    code: &str,
    message: String,
    hint: Option<String>,
    status: Option<u16>,
) -> ZotError {
    ZotError::Connector {
        code: code.to_string(),
        message,
        hint,
        status,
    }
}

#[cfg(test)]
mod tests {
    use std::thread::JoinHandle;

    use tiny_http::{Header, Response, Server};

    use super::*;

    struct Captured {
        method: String,
        url: String,
        headers: Vec<(String, String)>,
        body: String,
    }

    impl Captured {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(field, _)| field.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }
    }

    type FakeResponse = (u16, String, Vec<(&'static str, &'static str)>);

    fn spawn_server(responses: Vec<FakeResponse>) -> (String, JoinHandle<Vec<Captured>>) {
        let server = Server::http(("127.0.0.1", 0)).expect("bind fake connector server");
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
                    method: request.method().as_str().to_string(),
                    url: request.url().to_string(),
                    headers: request
                        .headers()
                        .iter()
                        .map(|header| (header.field.to_string(), header.value.to_string()))
                        .collect(),
                    body: request_body,
                });
                let mut response = Response::from_string(body).with_status_code(status);
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

    #[tokio::test]
    async fn ping_reports_available_and_zotero_version_on_success() {
        let (base, handle) = spawn_server(vec![(
            200,
            String::new(),
            vec![("X-Zotero-Version", "7.0.35")],
        )]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let ping = client.ping().await.expect("ping");
        assert!(ping.available);
        assert_eq!(ping.zotero_version.as_deref(), Some("7.0.35"));

        let captured = handle.join().expect("join server");
        assert_eq!(captured[0].method, "GET");
        assert_eq!(captured[0].url, "/connector/ping");
        assert_eq!(
            captured[0].header("x-zotero-connector-api-version"),
            Some("3")
        );
    }

    #[tokio::test]
    async fn local_http_probe_decodes_zotero_headers() {
        let (base, handle) = spawn_server(vec![(
            200,
            "Nothing to see here".to_string(),
            vec![
                ("X-Zotero-Version", "9.0.6"),
                ("X-Zotero-Connector-API-Version", "3"),
            ],
        )]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let status = client.probe_local_http().await.expect("local HTTP probe");
        assert!(status.available);
        assert_eq!(status.zotero_version.as_deref(), Some("9.0.6"));
        assert_eq!(status.connector_api_version.as_deref(), Some("3"));

        let captured = handle.join().expect("join server");
        assert_eq!(captured[0].method, "GET");
        assert_eq!(captured[0].url, "/api/");
    }

    #[tokio::test]
    async fn ping_maps_non_success_status_to_connector_http() {
        let (base, handle) = spawn_server(vec![(503, "unavailable".to_string(), vec![])]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let err = client.ping().await.expect_err("non-2xx ping");
        match err {
            ZotError::Connector { code, status, .. } => {
                assert_eq!(code, "connector-http");
                assert_eq!(status, Some(503));
            }
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn ping_maps_slow_server_to_timeout() {
        let server = Server::http(("127.0.0.1", 0)).expect("bind fake connector server");
        let port = server.server_addr().to_ip().expect("fake server IP").port();
        let base = format!("http://127.0.0.1:{port}/");
        let handle = std::thread::spawn(move || {
            let request = server.recv().expect("receive request");
            std::thread::sleep(Duration::from_millis(100));
            let _ = request.respond(Response::from_string("late"));
        });
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_millis(10))
            .expect("client");

        let err = client.ping().await.expect_err("timeout");
        match err {
            ZotError::Connector { code, .. } => assert_eq!(code, "connector-timeout"),
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[test]
    fn rejects_non_loopback_base_url() {
        let err = ConnectorClient::with_base_url_for_tests(
            "https://example.com/",
            Duration::from_secs(1),
        )
        .expect_err("remote connector URL");
        match err {
            ZotError::Connector { code, .. } => assert_eq!(code, "connector-base-url"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn selected_target_parses_writable_collection() {
        let body = serde_json::json!({
            "id": 42,
            "name": "Reading List",
            "libraryID": 1,
            "editable": true,
            "libraryEditable": true,
        })
        .to_string();
        let (base, handle) = spawn_server(vec![(200, body, vec![])]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let target = client.selected_target().await.expect("selected target");
        assert_eq!(target.id, Some(42));
        assert_eq!(target.library_id, Some(1));
        assert_eq!(target.name.as_deref(), Some("Reading List"));
        assert!(target.is_writable());

        let captured = handle.join().expect("join server");
        assert_eq!(captured[0].method, "POST");
        assert_eq!(captured[0].url, "/connector/getSelectedCollection");
        assert_eq!(
            captured[0].header("x-zotero-connector-api-version"),
            Some("3")
        );
    }

    #[tokio::test]
    async fn selected_target_reports_readonly_target_as_not_writable() {
        // Zotero omits `id`/`name` on a read-only target; a missing/false
        // `editable` or an explicit `libraryEditable: false` must both block
        // writes regardless of which one triggered it.
        let body = serde_json::json!({
            "libraryID": 7,
            "libraryEditable": false,
            "editable": false,
        })
        .to_string();
        let (base, handle) = spawn_server(vec![(200, body, vec![])]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let target = client.selected_target().await.expect("selected target");
        assert!(!target.editable);
        assert_eq!(target.library_editable, Some(false));
        assert!(!target.is_writable());
        assert_eq!(target.id, None);
        assert_eq!(target.name, None);

        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn selected_target_maps_non_success_status_to_connector_http() {
        let (base, handle) = spawn_server(vec![(500, "boom".to_string(), vec![])]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let err = client
            .selected_target()
            .await
            .expect_err("non-2xx selected target");
        match err {
            ZotError::Connector { code, status, .. } => {
                assert_eq!(code, "connector-http");
                assert_eq!(status, Some(500));
            }
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn import_posts_text_plain_body_with_session_query_and_parses_json_response() {
        let body = serde_json::json!([{ "key": "ABCD1234" }]).to_string();
        let (base, handle) = spawn_server(vec![(200, body, vec![])]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let result = client
            .import("zot-test-session", "@article{key1,\n}\n")
            .await
            .expect("import");
        assert_eq!(result.status, 200);
        assert_eq!(result.session, "zot-test-session");
        assert_eq!(result.response, serde_json::json!([{ "key": "ABCD1234" }]));

        let captured = handle.join().expect("join server");
        assert_eq!(captured[0].method, "POST");
        assert_eq!(
            captured[0].url,
            "/connector/import?session=zot-test-session"
        );
        assert_eq!(captured[0].body, "@article{key1,\n}\n");
        assert_eq!(captured[0].header("content-type"), Some("text/plain"));
        assert_eq!(
            captured[0].header("x-zotero-connector-api-version"),
            Some("3")
        );
    }

    #[tokio::test]
    async fn import_preserves_non_json_response_body_verbatim() {
        let (base, handle) = spawn_server(vec![(200, "Imported 1 item".to_string(), vec![])]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let result = client
            .import("zot-test-session", "TY  - JOUR\nER  - \n")
            .await
            .expect("import");
        assert_eq!(
            result.response,
            serde_json::Value::String("Imported 1 item".to_string())
        );

        handle.join().expect("join server");
    }

    #[tokio::test]
    async fn import_maps_non_success_status_to_connector_http() {
        let (base, handle) = spawn_server(vec![(400, "bad request".to_string(), vec![])]);
        let client = ConnectorClient::with_base_url_for_tests(&base, Duration::from_secs(1))
            .expect("client");

        let err = client
            .import("zot-test-session", "TY  - JOUR\nER  - \n")
            .await
            .expect_err("non-2xx import");
        match err {
            ZotError::Connector { code, status, .. } => {
                assert_eq!(code, "connector-http");
                assert_eq!(status, Some(400));
            }
            other => panic!("unexpected error: {other:?}"),
        }
        handle.join().expect("join server");
    }
}
