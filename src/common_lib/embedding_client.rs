use anyhow::Result;
use golem_rust::ConfigSchema;
use golem_rust::agentic::Secret;
use golem_rust::http::{self, Request};
use serde::{Deserialize, Serialize};

/// Default embedding dimension for mock embeddings
pub const DEFAULT_EMBEDDING_DIMENSION: usize = 768;

#[derive(ConfigSchema)]
pub struct EmbeddingConfig {
    #[config_schema(secret)]
    pub api_key: Secret<String>,
    pub api_base: String,
    pub model: String,
}

pub struct EmbeddingClient {
    model: String,
    api_key: String,
    api_base: String,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    input: &'a str,
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<u32>,
}

#[derive(Deserialize)]
struct EmbeddingResponseData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingResponseData>,
}

impl EmbeddingClient {
    pub fn new(config: EmbeddingConfig) -> Result<Self> {
        let api_key = config.api_key.get().map_err(|e| anyhow::anyhow!("{e:?}"))?;
        Ok(Self {
            model: config.model,
            api_key,
            api_base: config.api_base,
        })
    }

    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/embeddings", self.api_base.trim_end_matches('/'));
        let req_body = serde_json::to_vec(&EmbeddingRequest {
            input: text,
            model: &self.model,
            dimensions: Some(DEFAULT_EMBEDDING_DIMENSION as u32),
        })?;

        let mut req_builder = Request::post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");

        if !self.api_key.trim().is_empty() {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let request = req_builder.body(req_body)?;
        let response = http::send(request)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send embedding HTTP request: {e}"))?;

        let status = response.status();
        if !status.is_success() {
            let err_text = http::response_text(response).await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Embedding API returned HTTP {status}: {err_text}"
            ));
        }

        let res_data: EmbeddingResponse = http::response_json(response)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to parse embedding response JSON: {e}"))?;

        res_data
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| anyhow::anyhow!("No embeddings returned from embedding API"))
    }
}
