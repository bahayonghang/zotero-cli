use serde::Deserialize;
use serde_json::json;
use zot_core::{EmbeddingConfig, ZotError, ZotResult};

#[derive(Clone)]
pub struct EmbeddingClient {
    client: reqwest::Client,
    config: EmbeddingConfig,
}

impl EmbeddingClient {
    pub fn new(config: EmbeddingConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    pub fn configured(&self) -> bool {
        self.config.is_configured()
    }

    pub async fn embed(&self, texts: &[String]) -> ZotResult<Vec<Vec<f32>>> {
        if !self.configured() {
            return Err(ZotError::InvalidInput {
                code: "embedding-not-configured".to_string(),
                message: "Embedding endpoint is not configured".to_string(),
                hint: Some("Set ZOT_EMBEDDING_URL and ZOT_EMBEDDING_KEY".to_string()),
            });
        }

        let response = self
            .client
            .post(&self.config.url)
            .bearer_auth(&self.config.api_key)
            .json(&json!({
                "model": self.config.model,
                "input": texts,
            }))
            .send()
            .await
            .map_err(remote_err("embedding-request"))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ZotError::Remote {
                code: "embedding-http".to_string(),
                message: format!("Embedding API request failed with status {status}: {body}"),
                hint: None,
                status: Some(status),
            });
        }

        let payload: EmbeddingResponse = response
            .json()
            .await
            .map_err(remote_err("embedding-json"))?;
        Ok(payload
            .data
            .into_iter()
            .map(|entry| entry.embedding)
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
}

fn remote_err(code: &'static str) -> impl Fn(reqwest::Error) -> ZotError {
    move |err| ZotError::Remote {
        code: code.to_string(),
        message: err.to_string(),
        hint: None,
        status: err.status().map(|status| status.as_u16()),
    }
}
