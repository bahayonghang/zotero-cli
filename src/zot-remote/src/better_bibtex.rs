use serde::Deserialize;
use serde_json::json;
use zot_core::{ZotError, ZotResult};

use crate::http::{HttpRuntime, ensure_status, remote_err};

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
    base_url: String,
}

impl BetterBibTexClient {
    pub fn new(runtime: &HttpRuntime) -> Self {
        let port = std::env::var("ZOT_BBT_PORT").unwrap_or_else(|_| "23119".to_string());
        let base_url = std::env::var("ZOT_BBT_URL")
            .unwrap_or_else(|_| format!("http://127.0.0.1:{port}/better-bibtex"));
        Self {
            client: runtime.client_clone(),
            base_url,
        }
    }

    pub async fn probe(&self) -> bool {
        let url = format!("{}/cayw?probe=true", self.base_url);
        match self.client.get(url).send().await {
            Ok(response) => response
                .text()
                .await
                .map(|body| body == "ready")
                .unwrap_or(false),
            Err(_) => false,
        }
    }

    pub async fn search(&self, query: &str) -> ZotResult<Vec<BetterBibTexSearchItem>> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "item.search",
            "params": [query],
            "id": 1,
        });
        let response = self
            .client
            .post(format!("{}/json-rpc", self.base_url))
            .json(&payload)
            .send()
            .await
            .map_err(remote_err("bbt-search"))?;
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

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}
