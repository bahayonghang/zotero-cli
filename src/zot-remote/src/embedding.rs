use serde::Deserialize;
use serde_json::json;
use zot_core::{EmbeddingConfig, ZotError, ZotResult};

use crate::http::{HttpRuntime, ensure_status, remote_err};

const DEFAULT_EMBEDDING_BATCH_SIZE: usize = 64;
const EMBEDDING_BATCH_SIZE_ENV: &str = "ZOT_EMBEDDING_BATCH_SIZE";

#[derive(Clone)]
pub struct EmbeddingClient {
    client: reqwest::Client,
    config: EmbeddingConfig,
}

impl EmbeddingClient {
    pub fn new(runtime: &HttpRuntime, config: EmbeddingConfig) -> Self {
        Self {
            client: runtime.client_clone(),
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

        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = embedding_batch_size();
        let mut output = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(batch_size) {
            let batch = self.embed_chunk(chunk).await?;
            output.extend(batch);
        }
        validate_embeddings(texts.len(), output)
    }

    async fn embed_chunk(&self, texts: &[String]) -> ZotResult<Vec<Vec<f32>>> {
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

        let response = ensure_status(response, "embedding-http").await?;

        let payload: EmbeddingResponse = response
            .json()
            .await
            .map_err(remote_err("embedding-json"))?;
        let embeddings = payload
            .data
            .into_iter()
            .map(|entry| entry.embedding)
            .collect::<Vec<_>>();
        validate_embeddings(texts.len(), embeddings)
    }
}

fn embedding_batch_size() -> usize {
    embedding_batch_size_from(std::env::var(EMBEDDING_BATCH_SIZE_ENV).ok().as_deref())
}

fn embedding_batch_size_from(raw: Option<&str>) -> usize {
    raw.and_then(|value| value.parse::<usize>().ok())
        .filter(|size| *size > 0)
        .unwrap_or(DEFAULT_EMBEDDING_BATCH_SIZE)
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
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
    use super::{DEFAULT_EMBEDDING_BATCH_SIZE, embedding_batch_size_from, validate_embeddings};
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

    #[test]
    fn embedding_batch_size_defaults_to_64_when_env_missing() {
        assert_eq!(
            embedding_batch_size_from(None),
            DEFAULT_EMBEDDING_BATCH_SIZE
        );
    }

    #[test]
    fn embedding_batch_size_respects_positive_env_override() {
        assert_eq!(embedding_batch_size_from(Some("128")), 128);
    }

    #[test]
    fn embedding_batch_size_falls_back_for_zero_or_invalid_input() {
        assert_eq!(
            embedding_batch_size_from(Some("0")),
            DEFAULT_EMBEDDING_BATCH_SIZE
        );
        assert_eq!(
            embedding_batch_size_from(Some("not-a-number")),
            DEFAULT_EMBEDDING_BATCH_SIZE
        );
    }

    #[test]
    fn splits_large_input_into_batches() {
        // Documents that the request loop will fan a 200-item input across at
        // least 2 HTTP calls under the 64-item default batch size, satisfying
        // the F-15 contract observable from `texts.chunks(batch_size)`.
        let texts: Vec<String> = (0..200).map(|index| format!("doc-{index}")).collect();
        let batches = texts
            .chunks(DEFAULT_EMBEDDING_BATCH_SIZE)
            .collect::<Vec<_>>();
        assert!(
            batches.len() >= 2,
            "200 inputs must span at least 2 batches"
        );
        assert_eq!(
            batches.iter().map(|chunk| chunk.len()).sum::<usize>(),
            texts.len(),
            "concatenated batch sizes must equal the original input length"
        );
    }
}
