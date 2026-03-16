use anyhow::Result;
use golem_rust::Schema;
use golem_wasi_http::{Client, Method};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: String,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: u32,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub data: Vec<EmbeddingResponse>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct OllamaEmbeddingRequest {
    pub model: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct OllamaEmbeddingResponse {
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct OpenAIEmbeddingError {
    pub error: OpenAIEmbeddingErrorDetail,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct OpenAIEmbeddingErrorDetail {
    pub message: String,
    pub type_: String,
    pub code: Option<String>,
}

pub struct EmbeddingClient {
    pub model: String,
    pub provider: EmbeddingProvider,
    api_base: String,
    api_key: String,
    client: Client,
}

impl EmbeddingClient {
    pub fn new(
        api_base: String,
        api_key: String,
        model: String,
        provider: EmbeddingProvider,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {:?}", e))?;

        Ok(Self {
            api_base,
            api_key,
            model,
            provider,
            client,
        })
    }

    // Real embedding generation with HTTP calls using golem-wasi-http
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        log::info!("Generating embedding for text: {}", text);

        match self.provider {
            EmbeddingProvider::Ollama => self.generate_ollama_embedding(text).await,
            EmbeddingProvider::OpenAI => self.generate_openai_embedding(text).await,
            EmbeddingProvider::Mock => Ok(Self::mock_embedding(text, 768)),
        }
    }

    // Real Ollama embedding generation using golem-wasi-http (following S3 pattern)
    async fn generate_ollama_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request = OllamaEmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };

        let url = format!("{}/api/embeddings", self.api_base);
        let body = serde_json::to_string(&request)?;

        log::debug!("Making Ollama request to: {}", url);

        let response = self
            .client
            .request(Method::POST, &url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .map_err(|e| anyhow::anyhow!("Failed to send Ollama request: {:?}", e))?;

        let status = response.status();
        if status.is_success() {
            let response_body = response
                .text()
                .map_err(|e| anyhow::anyhow!("Failed to read Ollama response: {:?}", e))?;

            let embedding_response: OllamaEmbeddingResponse = serde_json::from_str(&response_body)
                .map_err(|e| anyhow::anyhow!("Failed to parse Ollama response: {}", e))?;

            log::debug!(
                "Generated Ollama embedding with {} dimensions",
                embedding_response.embedding.len()
            );
            Ok(embedding_response.embedding)
        } else {
            let error_body = response
                .text()
                .map_err(|e| anyhow::anyhow!("Failed to read error response: {:?}", e))?;

            Err(anyhow::anyhow!(
                "Ollama API error: {} - {}",
                status,
                error_body
            ))
        }
    }

    // Real OpenAI embedding generation using golem-wasi-http (following S3 pattern)
    async fn generate_openai_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request = EmbeddingRequest {
            model: self.model.clone(),
            input: text.to_string(),
        };

        let url = format!("{}/embeddings", self.api_base);
        let body = serde_json::to_string(&request)?;

        log::debug!("Making OpenAI request to: {}", url);

        let response = self
            .client
            .request(Method::POST, &url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .body(body)
            .send()
            .map_err(|e| anyhow::anyhow!("Failed to send OpenAI request: {:?}", e))?;

        let status = response.status();
        if status.is_success() {
            let response_body = response
                .text()
                .map_err(|e| anyhow::anyhow!("Failed to read OpenAI response: {:?}", e))?;

            let embedding_data: EmbeddingData = serde_json::from_str(&response_body)
                .map_err(|e| anyhow::anyhow!("Failed to parse OpenAI response: {}", e))?;

            if embedding_data.data.is_empty() {
                return Err(anyhow::anyhow!("No embeddings returned from API"));
            }

            log::debug!(
                "Generated OpenAI embedding with {} dimensions",
                embedding_data.data[0].embedding.len()
            );
            Ok(embedding_data.data[0].embedding.clone())
        } else {
            let error_body = response
                .text()
                .map_err(|e| anyhow::anyhow!("Failed to read error response: {:?}", e))?;

            // Try to parse OpenAI error format
            if let Ok(error_response) = serde_json::from_str::<OpenAIEmbeddingError>(&error_body) {
                Err(anyhow::anyhow!(
                    "OpenAI API error: {} - {}",
                    status,
                    error_response.error.message
                ))
            } else {
                Err(anyhow::anyhow!(
                    "OpenAI API error: {} - {}",
                    status,
                    error_body
                ))
            }
        }
    }

    // Fallback mock embedding for testing
    pub fn mock_embedding(text: &str, dimension: usize) -> Vec<f32> {
        let mut embedding = Vec::with_capacity(dimension);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        text.hash(&mut hasher);
        let seed = hasher.finish();

        for i in 0..dimension {
            let value = ((seed >> (i % 64)) & 0xFF) as f32;
            embedding.push((value - 128.0) / 128.0);
        }

        // Normalize the embedding
        let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= norm);
        }

        embedding
    }
}

#[derive(Clone, Copy, Debug, Schema, Serialize, Deserialize)]
pub enum EmbeddingProvider {
    Ollama,
    OpenAI,
    Mock,
}

impl EmbeddingClient {
    pub fn from_env() -> Result<Self> {
        let api_base = std::env::var("EMBEDDING_API_BASE")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let api_key = std::env::var("EMBEDDING_API_KEY").unwrap_or_else(|_| "ollama".to_string());
        let model =
            std::env::var("EMBEDDING_MODEL").unwrap_or_else(|_| "nomic-embed-text".to_string());

        let provider_str =
            std::env::var("EMBEDDING_PROVIDER").unwrap_or_else(|_| "ollama".to_string());
        let provider = match provider_str.to_lowercase().as_str() {
            "ollama" => EmbeddingProvider::Ollama,
            "openai" => EmbeddingProvider::OpenAI,
            "mock" => EmbeddingProvider::Mock,
            _ => {
                log::warn!(
                    "Unknown EMBEDDING_PROVIDER '{}', defaulting to Ollama",
                    provider_str
                );
                EmbeddingProvider::Ollama
            }
        };

        Self::new(api_base, api_key, model, provider)
    }

    pub async fn generate_embedding_with_fallback(&self, text: &str) -> Result<Vec<f32>> {
        match self.provider {
            EmbeddingProvider::Ollama | EmbeddingProvider::OpenAI => {
                match self.generate_embedding(text).await {
                    Ok(embedding) => Ok(embedding),
                    Err(e) => {
                        log::warn!("Embedding generation failed, using mock: {:?}", e);
                        Ok(Self::mock_embedding(text, 768)) // Common embedding dimension
                    }
                }
            }
            EmbeddingProvider::Mock => Ok(Self::mock_embedding(text, 768)),
        }
    }
}
