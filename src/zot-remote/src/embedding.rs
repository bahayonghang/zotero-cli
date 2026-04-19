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
        validate_embeddings(
            texts.len(),
            payload
                .data
                .into_iter()
                .map(|entry| entry.embedding)
                .collect(),
        )
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

fn validate_embeddings(requested: usize, embeddings: Vec<Vec<f32>>) -> ZotResult<Vec<Vec<f32>>> {
    if requested == 0 || embeddings.len() == requested {
        return Ok(embeddings);
    }

    Err(ZotError::Remote {
        code: "embedding-count-mismatch".to_string(),
        message: format!(
            "Embedding service returned {} vectors for {} inputs",
            embeddings.len(),
            requested
        ),
        hint: Some("Check embedding service health or response format".to_string()),
        status: None,
    })
}

#[cfg(test)]
mod tests {
    use super::validate_embeddings;
    use zot_core::ZotError;

    #[test]
    fn accepts_matching_embedding_counts() {
        let embeddings = vec![vec![0.1_f32, 0.2_f32], vec![0.3_f32, 0.4_f32]];
        let validated = validate_embeddings(2, embeddings.clone()).expect("matching counts");
        assert_eq!(validated, embeddings);
    }

    #[test]
    fn rejects_mismatched_embedding_counts() {
        let err = validate_embeddings(2, vec![vec![0.1_f32, 0.2_f32]])
            .expect_err("mismatched counts should fail");
        match err {
            ZotError::Remote { code, .. } => assert_eq!(code, "embedding-count-mismatch"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
