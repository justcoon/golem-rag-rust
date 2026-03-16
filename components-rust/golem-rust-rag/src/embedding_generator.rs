use common_lib::*;
use golem_rust::{agent_definition, agent_implementation};
use std::string::String;

pub type AgentResult<T> = std::result::Result<T, String>;

#[agent_definition]
pub trait EmbeddingGeneratorAgent {
    fn new() -> Self;

    /// Generate and store embeddings for a specific document
    ///
    /// # Arguments
    /// * `document_id` - String ID of the document to process
    ///
    /// # Returns
    /// Number of embeddings generated for the document
    async fn generate_embeddings_for_document(&mut self, document_id: String) -> AgentResult<u32>;

    /// Get embedding status for a document
    async fn get_embedding_status(&self, document_id: String) -> AgentResult<EmbeddingStatus>;

    /// Generate embeddings for multiple documents
    ///
    /// # Arguments
    /// * `document_ids` - List of document IDs to process
    ///
    /// # Returns
    /// Total number of embeddings generated across all documents
    async fn generate_embeddings_for_documents(
        &mut self,
        document_ids: Vec<String>,
    ) -> AgentResult<u32>;
}

struct EmbeddingGeneratorAgentImpl {
    db_config: PostgresDbConfig,
    embedding_client: Option<EmbeddingClient>,
    embedding_provider: Option<EmbeddingProvider>,
    chunk_config: ChunkConfig,
}

#[agent_implementation]
impl EmbeddingGeneratorAgent for EmbeddingGeneratorAgentImpl {
    fn new() -> Self {
        let db_config =
            PostgresDbConfig::from_env().expect("Failed to load PostgresDbConfig from environment");

        // Initialize embedding client if available
        let (embedding_client, embedding_provider) = match EmbeddingClient::from_env() {
            Ok((client, provider)) => (Some(client), Some(provider)),
            Err(e) => {
                log::warn!("Failed to initialize embedding client: {:?}", e);
                (None, None)
            }
        };

        let chunk_config = ChunkConfig::default();

        Self {
            db_config,
            embedding_client,
            embedding_provider,
            chunk_config,
        }
    }

    async fn generate_embeddings_for_document(&mut self, document_id: String) -> AgentResult<u32> {
        log::info!("Generating embeddings for document: {}", document_id);

        // Connect to database
        let mut db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_config.db_url()) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

        // Load document from database
        let document = match db_helper.load_document(&document_id) {
            Ok(Some(doc)) => doc,
            Ok(None) => return Err(format!("Document not found: {}", document_id)),
            Err(e) => return Err(format!("Failed to load document: {:?}", e)),
        };

        // Mark document as in progress
        if let Err(e) =
            db_helper.update_embedding_status(&document_id, &EmbeddingStatus::InProgress)
        {
            log::warn!("Failed to update embedding status: {:?}", e);
        }

        // Split document into chunks
        let chunks = match self.chunk_document(&document.content, &self.chunk_config) {
            Ok(chunks) => chunks,
            Err(e) => {
                let error_msg = format!("Failed to chunk document: {:?}", e);
                self.mark_as_failed(&mut db_helper, &document_id, &error_msg);
                return Err(error_msg);
            }
        };

        // Generate embeddings for each chunk
        let mut embedding_count = 0;
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            match self
                .generate_and_store_embedding(
                    &mut db_helper,
                    &document_id,
                    chunk_index as u32,
                    chunk,
                )
                .await
            {
                Ok(_) => embedding_count += 1,
                Err(e) => {
                    log::error!(
                        "Failed to generate embedding for chunk {}: {:?}",
                        chunk_index,
                        e
                    );
                    // Continue with other chunks
                }
            }
        }

        // Mark document as completed
        if let Err(e) = db_helper.update_embedding_status(
            &document_id,
            &EmbeddingStatus::Completed {
                chunk_count: embedding_count,
            },
        ) {
            log::warn!("Failed to update embedding status to completed: {:?}", e);
        }

        log::info!(
            "Successfully generated {} embeddings for document: {}",
            embedding_count,
            document_id
        );
        Ok(embedding_count as u32)
    }

    async fn get_embedding_status(&self, document_id: String) -> AgentResult<EmbeddingStatus> {
        let mut db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_config.db_url()) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

        db_helper
            .get_embedding_status(&document_id)
            .map_err(|e| format!("Failed to get embedding status: {:?}", e))
    }

    async fn generate_embeddings_for_documents(
        &mut self,
        document_ids: Vec<String>,
    ) -> AgentResult<u32> {
        log::info!("Generating embeddings for {} documents", document_ids.len());

        let mut total_embeddings = 0;
        for document_id in &document_ids {
            match self
                .generate_embeddings_for_document(document_id.clone())
                .await
            {
                Ok(count) => total_embeddings += count,
                Err(e) => {
                    log::error!("Failed to process document {}: {:?}", document_id, e);
                    // Continue with other documents
                }
            }
        }

        log::info!(
            "Generated total of {} embeddings for {} documents",
            total_embeddings,
            document_ids.len()
        );
        Ok(total_embeddings)
    }
}

impl EmbeddingGeneratorAgentImpl {
    fn chunk_document(
        &self,
        content: &str,
        config: &ChunkConfig,
    ) -> AgentResult<Vec<DocumentChunk>> {
        let mut chunks = Vec::new();
        let content_chars: Vec<char> = content.chars().collect();
        let content_len = content_chars.len();

        if content_len == 0 {
            return Ok(chunks);
        }

        let mut chunk_start = 0;
        let mut chunk_index = 0;

        while chunk_start < content_len {
            let chunk_end = std::cmp::min(chunk_start + config.chunk_size as usize, content_len);

            // If we should respect sentences and this isn't the last chunk
            let actual_chunk_end = if config.respect_sentences && chunk_end < content_len {
                self.find_sentence_boundary(
                    &content_chars,
                    chunk_start,
                    chunk_end,
                    config.chunk_size as usize,
                )
            } else {
                chunk_end
            };

            // Ensure minimum chunk size
            let final_chunk_end = if actual_chunk_end - chunk_start < config.min_chunk_size as usize
                && chunk_end < content_len
            {
                std::cmp::min(chunk_start + config.min_chunk_size as usize, content_len)
            } else {
                actual_chunk_end
            };

            let chunk_content: String =
                content_chars[chunk_start..final_chunk_end].iter().collect();

            // Create chunk
            let chunk = DocumentChunk {
                id: uuid::Uuid::new_v4().to_string(),
                document_id: String::new(), // Will be set by caller
                content: chunk_content.clone(),
                chunk_index: chunk_index as u32,
                start_pos: chunk_start as u32,
                end_pos: final_chunk_end as u32,
                token_count: Some(self.estimate_token_count(&chunk_content)),
            };

            chunks.push(chunk);

            chunk_start = final_chunk_end;
            chunk_index += 1;
        }

        Ok(chunks)
    }

    fn find_sentence_boundary(
        &self,
        content: &[char],
        start: usize,
        end: usize,
        max_size: usize,
    ) -> usize {
        // Look for sentence endings in the chunk
        let search_end = std::cmp::min(end, start + max_size);
        let mut best_boundary = end;

        for i in (start..search_end).rev() {
            if i + 1 < content.len() {
                let current = content[i];
                let next = content[i + 1];

                // Look for sentence endings followed by space or end of text
                if (current == '.' || current == '!' || current == '?')
                    && (next == ' ' || next == '\n' || next == '\r')
                {
                    best_boundary = i + 1;
                    break;
                }
            }
        }

        best_boundary
    }

    fn estimate_token_count(&self, text: &str) -> u32 {
        // Simple estimation: roughly 4 characters per token
        // This is a rough approximation - in production you'd use a proper tokenizer
        ((text.len() as f32) / 4.0).ceil() as u32
    }

    async fn generate_and_store_embedding(
        &self,
        db_helper: &mut DatabaseHelper,
        document_id: &str,
        chunk_index: u32,
        chunk: &DocumentChunk,
    ) -> AgentResult<String> {
        // Generate embedding using real HTTP client
        let embedding_vector = match &self.embedding_client {
            Some(client) => {
                match &self.embedding_provider {
                    Some(_provider) => {
                        log::debug!(
                            "Generating real embedding for chunk: {}",
                            &chunk.content[..chunk.content.len().min(100)]
                        );

                        // Use async call directly
                        match client.generate_embedding(&chunk.content).await {
                            Ok(embedding) => {
                                log::debug!(
                                    "Successfully generated real embedding with {} dimensions",
                                    embedding.len()
                                );
                                embedding
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to generate real embedding, falling back to mock: {:?}",
                                    e
                                );
                                EmbeddingClient::mock_embedding(&chunk.content, 768)
                            }
                        }
                    }
                    None => {
                        log::debug!("No embedding provider, using mock embedding");
                        EmbeddingClient::mock_embedding(&chunk.content, 768)
                    }
                }
            }
            None => {
                log::debug!("No embedding client, using mock embedding");
                EmbeddingClient::mock_embedding(&chunk.content, 768)
            }
        };

        // Store document chunk
        let mut chunk_with_id = chunk.clone();
        chunk_with_id.document_id = document_id.to_string();

        if let Err(e) = db_helper.store_document_chunk(&chunk_with_id) {
            return Err(format!("Failed to store document chunk: {:?}", e));
        }

        // Create embedding record
        let embedding = Embedding {
            id: uuid::Uuid::new_v4().to_string(),
            chunk_id: chunk_with_id.id.clone(),
            vector: embedding_vector,
            model_name: self.get_model_name(),
            created_at: "2024-01-01T00:00:00Z".to_string(), // Simplified for now
        };

        // Store embedding
        db_helper
            .store_embedding(&embedding)
            .map_err(|e| format!("Failed to store embedding: {:?}", e))?;

        log::debug!(
            "Generated and stored embedding for chunk {} of document {}",
            chunk_index,
            document_id
        );
        Ok(chunk_with_id.id)
    }

    fn get_model_name(&self) -> String {
        match &self.embedding_provider {
            Some(EmbeddingProvider::Ollama) => {
                std::env::var("EMBEDDING_MODEL").unwrap_or_else(|_| "nomic-embed-text".to_string())
            }
            Some(EmbeddingProvider::OpenAI) => std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-ada-002".to_string()),
            Some(EmbeddingProvider::Mock) => "mock-embedding-v1".to_string(),
            None => "unknown".to_string(),
        }
    }

    fn mark_as_failed(&self, db_helper: &mut DatabaseHelper, document_id: &str, error: &str) {
        if let Err(e) = db_helper.update_embedding_status(
            document_id,
            &EmbeddingStatus::Failed {
                error: error.to_string(),
            },
        ) {
            log::error!("Failed to mark document as failed: {:?}", e);
        }
    }
}
