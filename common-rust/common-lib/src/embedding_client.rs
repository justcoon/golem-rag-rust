use serde::{Deserialize, Serialize};
use golem_rust::Schema;
use anyhow::Result;

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

pub struct EmbeddingClient {
    api_base: String,
    api_key: String,
    model: String,
}

impl EmbeddingClient {
    pub fn new(api_base: String, api_key: String, model: String) -> Result<Self> {
        Ok(Self {
            api_base,
            api_key,
            model,
        })
    }

    // Simplified embedding generation - for now returns mock embeddings
    // In a real implementation, this would make HTTP calls to the embedding service
    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        log::info!("Generating embedding for text: {}", text);
        
        // For now, return a mock embedding with proper dimensions
        // This can be replaced with actual HTTP calls later
        Ok(Self::mock_embedding(text, 768))
    }

    // Mock embedding for testing
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

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub enum EmbeddingProvider {
    Ollama,
    Mock,
}

impl EmbeddingClient {
    pub fn from_env() -> Result<(Self, EmbeddingProvider)> {
        let api_base = std::env::var("EMBEDDING_API_BASE")
            .unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key = std::env::var("EMBEDDING_API_KEY")
            .unwrap_or_else(|_| "ollama".to_string());
        let model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "nomic-embed-text".to_string());

        if api_base.contains("localhost") || api_base.contains("127.0.0.1") {
            let client = Self::new(api_base, api_key, model)?;
            Ok((client, EmbeddingProvider::Ollama))
        } else {
            let client = Self::new(api_base, api_key, model)?;
            Ok((client, EmbeddingProvider::Ollama))
        }
    }

    pub async fn generate_embedding_with_fallback(&self, text: &str, provider: &EmbeddingProvider) -> Result<Vec<f32>> {
        match provider {
            EmbeddingProvider::Ollama => {
                match self.generate_embedding(text).await {
                    Ok(embedding) => Ok(embedding),
                    Err(e) => {
                        log::warn!("Ollama embedding failed, using mock: {:?}", e);
                        Ok(Self::mock_embedding(text, 768)) // Common embedding dimension
                    }
                }
            }
            EmbeddingProvider::Mock => Ok(Self::mock_embedding(text, 768)),
        }
    }
}
