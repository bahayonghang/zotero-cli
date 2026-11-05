use reqwest::Url;
use serde::Deserialize;
use serde_json::json;
use zot_core::{ZotError, ZotResult};

use crate::http::{HttpRuntime, ensure_status, remote_err, send_with_retry};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BetterBibTexSearchItem {
    #[serde(default)]
    pub citekey: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "itemKey", default)]
    pub item_key: String,
    #[serde(rename = "libraryID")]
    pub library_id: Option<i64>,
}

#[derive(Clone)]
pub struct BetterBibTexClient {
    client: reqwest::Client,
    /// `None` means the configured base is not an HTTP loopback origin.
    /// Probe stays false and search fails before any request.
    base_url: Option<String>,
}

impl BetterBibTexClient {
    pub fn new(runtime: &HttpRuntime) -> Self {
        let port = std::env::var("ZOT_BBT_PORT").unwrap_or_else(|_| "23119".to_string());
        let base_url = std::env::var("ZOT_BBT_URL")
            .unwrap_or_else(|_| format!("http://127.0.0.1:{port}/better-bibtex"));
        Self::from_raw(runtime, &base_url)
    }

    fn from_raw(runtime: &HttpRuntime, raw: &str) -> Self {
        Self {
            client: runtime.download_client().clone(),
            base_url: loopback_base(raw),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_base_url_for_tests(runtime: &HttpRuntime, base_url: &str) -> Self {
        Self::from_raw(runtime, base_url)
    }

    pub async fn probe(&self) -> bool {
        let Some(base_url) = &self.base_url else {
            return false;
        };
        let url = format!("{base_url}/cayw?probe=true");
        match send_with_retry(self.client.get(url), "bbt-probe").await {
            Ok(response) if response.status().is_redirection() => false,
            Ok(response) if response.status().is_success() => response
                .text()
                .await
                .map(|body| body == "ready")
                .unwrap_or(false),
            _ => false,
        }
    }

    pub async fn search(&self, query: &str) -> ZotResult<Vec<BetterBibTexSearchItem>> {
        let Some(base_url) = &self.base_url else {
            return Err(bbt_error(
                "bbt-url",
                "Better BibTeX URL is not an HTTP loopback address",
                Some("Use the local Better BibTeX endpoint. An external ZOT_BBT_URL is ignored."),
            ));
        };
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "item.search",
            "params": [query],
            "id": 1,
        });
        let response = self
            .client
            .post(format!("{base_url}/json-rpc"))
            .json(&payload)
            .send()
            .await
            .map_err(remote_err("bbt-search"))?;
        if response.status().is_redirection() {
            return Err(bbt_error(
                "bbt-redirect",
                "Better BibTeX refused to follow a redirect",
                Some("The plugin endpoint must stay on loopback."),
            ));
        }
        let response = ensure_status(response, "bbt-search-http").await?;
        let payload: JsonRpcResponse<Vec<BetterBibTexSearchItem>> = response
            .json()
            .await
            .map_err(remote_err("bbt-search-json"))?;
        if let Some(error) = payload.error {
            return Err(ZotError::Remote {
                code: "bbt-search-rpc".to_string(),
                message: error.message,
                hint: Some("Ensure Zotero is running with Better BibTeX installed".to_string()),
                status: None,
            });
        }
        Ok(payload.result.unwrap_or_default())
    }
}

fn bbt_error(code: &str, message: &str, hint: Option<&str>) -> ZotError {
    ZotError::Remote {
        code: code.to_string(),
        message: message.to_string(),
        hint: hint.map(str::to_string),
        status: None,
    }
}

/// Accept only `http` loopback with no user info. Trailing slashes are removed.
fn loopback_base(raw: &str) -> Option<String> {
    let url = Url::parse(raw.trim()).ok()?;
    if url.scheme() != "http" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    if !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1")) {
        return None;
    }
    Some(url.as_str().trim_end_matches('/').to_string())
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tiny_http::{Header, Response as FakeResponse, Server};

    struct Hit {
        method: String,
        url: String,
        body: String,
    }

    fn serve(
        reply: impl Fn(&Hit) -> (u16, String, Vec<(&'static str, &'static str)>) + Send + Sync + 'static,
    ) -> (String, Arc<Mutex<Vec<Hit>>>) {
        let http = Server::http("127.0.0.1:0").expect("server");
        let base = format!("http://{}/better-bibtex", http.server_addr());
        let hits = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&hits);
        std::thread::spawn(move || {
            while let Ok(Some(mut request)) = http.recv_timeout(Duration::from_secs(3)) {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let hit = Hit {
                    method: request.method().to_string(),
                    url: request.url().to_string(),
                    body,
                };
                let (status, response_body, headers) = reply(&hit);
                recorded.lock().expect("hits").push(hit);
                let mut response =
                    FakeResponse::from_string(response_body).with_status_code(status);
                for (name, value) in headers {
                    response.add_header(Header::from_bytes(name, value).expect("header"));
                }
                let _ = request.respond(response);
            }
        });
        (base, hits)
    }

    fn runtime() -> HttpRuntime {
        HttpRuntime::new().expect("runtime")
    }

    #[tokio::test]
    async fn item_search_is_the_only_rpc_and_external_urls_are_not_called() {
        let (base, hits) = serve(|hit| {
            assert_eq!(hit.method, "POST");
            let value: serde_json::Value = serde_json::from_str(&hit.body).expect("json");
            assert_eq!(value["method"], "item.search");
            (
                200,
                r#"{"jsonrpc":"2.0","result":[{"citekey":"Smith2024","itemKey":"ITEM0001","libraryID":1}],"id":1}"#
                    .to_string(),
                Vec::new(),
            )
        });
        let client = BetterBibTexClient::with_base_url_for_tests(&runtime(), &base);
        let found = client.search("Smith2024").await.expect("search");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].citekey, "Smith2024");
        assert_eq!(found[0].item_key, "ITEM0001");
        assert_eq!(found[0].library_id, Some(1));
        {
            let recorded = hits.lock().expect("hits");
            assert_eq!(recorded.len(), 1);
            assert!(recorded.iter().all(|hit| hit.method == "POST"));
            assert!(
                recorded
                    .iter()
                    .all(|hit| hit.body.contains("\"method\":\"item.search\""))
            );
        }

        let external = BetterBibTexClient::with_base_url_for_tests(
            &runtime(),
            "http://example.com/better-bibtex",
        );
        assert!(!external.probe().await);
        let error = external.search("Smith2024").await.expect_err("rejected");
        assert_eq!(error.payload().code, "bbt-url");
        assert_eq!(hits.lock().expect("hits").len(), 1);
    }

    #[tokio::test]
    async fn redirects_are_not_followed() {
        let (base, hits) = serve(|_| {
            (
                302,
                String::new(),
                vec![("Location", "http://example.com/better-bibtex/json-rpc")],
            )
        });
        let client = BetterBibTexClient::with_base_url_for_tests(&runtime(), &format!("{base}/"));
        assert!(!client.probe().await);
        let error = client.search("Smith2024").await.expect_err("redirect");
        assert_eq!(error.payload().code, "bbt-redirect");
        let recorded = hits.lock().expect("hits");
        assert_eq!(recorded.len(), 2);
        assert!(
            recorded
                .iter()
                .all(|hit| hit.url.starts_with("/better-bibtex"))
        );
        assert!(recorded.iter().all(|hit| !hit.url.contains("example.com")));
    }
}
