use anyhow::Result;
use golem_ai_embed::EmbeddingProvider;
use golem_ai_embed::config::SecretSource;
use golem_ai_embed::model::{Config, ContentPart};
use golem_ai_embed_openai::{DurableOpenAI, OpenAiEmbedConfig};
use golem_rust::ConfigSchema;
use golem_rust::agentic::Secret;

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
    config: OpenAiEmbedConfig,
}

impl EmbeddingClient {
    pub fn new(config: EmbeddingConfig) -> Result<Self> {
        let openai_config = OpenAiEmbedConfig {
            api_key: SecretSource::from_handle(config.api_key),
            base_url: Some(config.api_base),
        };

        Ok(Self {
            model: config.model,
            config: openai_config,
        })
    }

    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let inputs = vec![ContentPart::Text(text.to_string())];
        let config = Config {
            model: Some(self.model.clone()),
            task_type: None,
            dimensions: Some(DEFAULT_EMBEDDING_DIMENSION as u32),
            truncation: None,
            output_format: None,
            output_dtype: None,
            user: None,
            provider_options: vec![],
        };
        let response = DurableOpenAI::generate(self.config.clone(), inputs, config)
            .map_err(|e| anyhow::anyhow!("Embedding generation failed: {}", e))?;

        if response.embeddings.is_empty() {
            return Err(anyhow::anyhow!("No embeddings returned"));
        }

        match &response.embeddings[0].vector {
            golem_ai_embed::model::VectorData::Float(vec) => Ok(vec.clone()),
            _ => Err(anyhow::anyhow!("Unexpected vector type")),
        }
    }
}
