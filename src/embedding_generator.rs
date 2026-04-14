use crate::common_lib::database::DatabaseHelper;
use crate::common_lib::embedding_client::EmbeddingClient;
use crate::database_helper::DatabaseHelperRagext;
use crate::models::*;
use chrono::Utc;
use futures::future;
use golem_rust::{agent_definition, agent_implementation};
use std::string::String;

pub type AgentResult<T> = std::result::Result<T, String>;

#[agent_definition(ephemeral)]
pub trait EmbeddingGeneratorAgent {
    fn new() -> Self;

    /// Generate embeddings for multiple documents
    ///
    /// # Arguments
    /// * `document_ids` - List of document IDs to process
    ///
    /// # Returns
    /// Total number of embeddings generated across all documents
    async fn generate_embeddings_for_documents(
        &self,
        document_ids: Vec<String>,
    ) -> AgentResult<u32>;

    /// Generate embeddings for all documents that don't have embeddings yet
    ///
    /// # Returns
    /// Tuple of (document_ids_processed, total_embeddings_generated)
    async fn generate_embeddings_for_all_documents(&self) -> AgentResult<(Vec<String>, u32)>;

    /// Get all documents that don't have embeddings yet
    ///
    /// # Returns
    /// Vector of document IDs that don't have embeddings
    async fn get_documents_without_embeddings(&self) -> AgentResult<Vec<String>>;
}

#[agent_definition(ephemeral)]
pub trait DocumentEmbeddingGeneratorAgent {
    fn new() -> Self;

    /// Generate and store embeddings for a specific document
    ///
    /// # Arguments
    /// * `document_id` - String ID of the document to process
    ///
    /// # Returns
    /// Number of embeddings generated for the document
    async fn generate_embeddings_for_document(&self, document_id: String) -> AgentResult<u32>;

    /// Remove all embeddings and chunks for a specific document
    ///
    /// # Arguments
    /// * `document_id` - String ID of the document to remove embeddings for
    ///
    /// # Returns
    /// Ok(()) if successful, error message if failed
    async fn remove_embeddings_for_document(&self, document_id: String) -> AgentResult<()>;

    /// Get embedding status for a document
    async fn get_embedding_status(&self, document_id: String) -> AgentResult<EmbeddingStatus>;
}

struct EmbeddingGeneratorAgentImpl;

#[agent_implementation]
impl EmbeddingGeneratorAgent for EmbeddingGeneratorAgentImpl {
    fn new() -> Self {
        Self
    }

    async fn generate_embeddings_for_documents(
        &self,
        document_ids: Vec<String>,
    ) -> AgentResult<u32> {
        log::info!(
            "Generating embeddings for {} documents in parallel",
            document_ids.len()
        );

        // Process all documents in parallel using join_all
        let futures = document_ids.into_iter().map(|document_id| async move {
            // Create a separate document embedding generator for each document
            // let doc_generator = DocumentEmbeddingGeneratorAgentClient::get();
            let doc_generator = DocumentEmbeddingGeneratorAgentClient::new_phantom();
            match doc_generator
                .generate_embeddings_for_document(document_id.clone())
                .await
            {
                Ok(count) => {
                    log::debug!(
                        "Successfully generated {} embeddings for document: {}",
                        count,
                        document_id
                    );
                    Some(count)
                }
                Err(e) => {
                    log::error!("Failed to process document {}: {:?}", document_id, e);
                    None
                }
            }
        });

        let results = future::join_all(futures).await;
        let total_documents = results.len();

        let mut total_embeddings = 0;
        let mut processed_count = 0;

        for count in results.into_iter().flatten() {
            total_embeddings += count;
            processed_count += 1;
        }

        log::info!(
            "Generated total of {} embeddings for {}/{} documents",
            total_embeddings,
            processed_count,
            total_documents
        );
        Ok(total_embeddings)
    }

    async fn generate_embeddings_for_all_documents(&self) -> AgentResult<(Vec<String>, u32)> {
        log::info!("Finding all documents without embeddings");

        // Get documents without embeddings
        let document_ids = self.get_documents_without_embeddings().await?;

        if document_ids.is_empty() {
            return Ok((vec![], 0));
        }

        // Generate embeddings for all found documents
        let total_embeddings = self
            .generate_embeddings_for_documents(document_ids.clone())
            .await?;

        log::info!(
            "Successfully processed {} documents with {} total embeddings",
            document_ids.len(),
            total_embeddings
        );

        Ok((document_ids, total_embeddings))
    }

    async fn get_documents_without_embeddings(&self) -> AgentResult<Vec<String>> {
        log::info!("Finding all documents without embeddings");
        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        // Query for documents that don't have embeddings or have failed embeddings
        let query = r#"
            SELECT DISTINCT d.id 
            FROM documents d 
            LEFT JOIN document_embeddings de ON d.id = de.document_id 
            WHERE de.document_id IS NULL 
               OR de.embedding_status LIKE 'failed%'
               OR de.embedding_status = 'not_processed'
        "#;

        let result = db_helper
            .connection
            .query(query, vec![])
            .map_err(|e| format!("Failed to query documents without embeddings: {:?}", e))?;

        use crate::common_lib::database::decode::{DbResultDecoder, Single};
        let document_ids: Vec<String> = Single::<String>::decode_result(result)
            .map_err(|e| format!("Failed to decode document IDs: {:?}", e))?
            .into_iter()
            .map(|s| s.0)
            .collect();

        log::info!("Found {} documents without embeddings", document_ids.len());
        Ok(document_ids)
    }
}

struct DocumentEmbeddingGeneratorAgentImpl {
    embedding_client: EmbeddingClient,
    chunk_config: ChunkConfig,
}

#[agent_implementation]
impl DocumentEmbeddingGeneratorAgent for DocumentEmbeddingGeneratorAgentImpl {
    fn new() -> Self {
        let embedding_client = EmbeddingClient::from_env()
            .expect("Failed to create embedding client from environment");

        let chunk_config = ChunkConfig::default();

        Self {
            embedding_client,
            chunk_config,
        }
    }

    async fn generate_embeddings_for_document(&self, document_id: String) -> AgentResult<u32> {
        log::info!("Generating embeddings for document: {}", document_id);

        let db_helper = self.create_db_helper()?;

        // Check if embeddings already exist and return early if completed
        if let Ok(EmbeddingStatus::Completed { chunk_count }) =
            db_helper.get_embedding_status(&document_id)
        {
            log::info!(
                "Embeddings already exist for document: {} ({} chunks), skipping",
                document_id,
                chunk_count
            );
            return Ok(chunk_count as u32);
        }

        // Check if embeddings are already in progress
        if let Ok(EmbeddingStatus::InProgress) = db_helper.get_embedding_status(&document_id) {
            log::info!(
                "Embeddings are already in progress for document: {}, skipping",
                document_id
            );
            return Err("Embeddings are already in progress".to_string());
        }

        // Load document
        let document = self.load_document(&db_helper, &document_id)?;

        // Mark as in progress
        self.mark_status(&db_helper, &document_id, &EmbeddingStatus::InProgress)?;

        // Clean up any existing partial data
        self.cleanup_existing_chunks(&db_helper, &document_id)?;

        // Process document
        let embedding_count = self
            .process_document(&db_helper, &document_id, &document.content)
            .await?;

        // Mark as completed
        self.mark_status(
            &db_helper,
            &document_id,
            &EmbeddingStatus::Completed {
                chunk_count: embedding_count as usize,
            },
        )?;

        log::info!(
            "Successfully generated {} embeddings for document: {}",
            embedding_count,
            document_id
        );
        Ok(embedding_count)
    }

    async fn get_embedding_status(&self, document_id: String) -> AgentResult<EmbeddingStatus> {
        let db_helper = self.create_db_helper()?;
        db_helper
            .get_embedding_status(&document_id)
            .map_err(|e| format!("Failed to get embedding status: {:?}", e))
    }

    async fn remove_embeddings_for_document(&self, document_id: String) -> AgentResult<()> {
        log::info!("Removing embeddings for document: {}", document_id);

        let db_helper = self.create_db_helper()?;

        // Verify document exists
        self.load_document(&db_helper, &document_id)?;

        // Remove embeddings and chunks
        self.cleanup_existing_chunks(&db_helper, &document_id)?;

        // Reset status
        self.mark_status(&db_helper, &document_id, &EmbeddingStatus::NotProcessed)?;

        log::info!(
            "Successfully removed embeddings for document: {}",
            document_id
        );
        Ok(())
    }
}

impl DocumentEmbeddingGeneratorAgentImpl {
    fn chunk_document(
        &self,
        document_id: &str,
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
                document_id: document_id.to_string(),
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
        db_helper: &DatabaseHelper,
        document_id: &str,
        chunk_index: u32,
        chunk: &DocumentChunk,
    ) -> AgentResult<String> {
        // Generate embedding using fallback logic
        log::debug!(
            "Generating embedding for chunk: {}",
            &chunk.content[..chunk.content.len().min(100)]
        );
        let embedding_vector = self
            .embedding_client
            .generate_embedding_with_fallback(&chunk.content)
            .await
            .map_err(|e| format!("Failed to generate embedding: {:?}", e))?;

        // Store document chunk
        db_helper
            .store_document_chunk(chunk)
            .map_err(|e| format!("Failed to store document chunk: {:?}", e))?;

        // Create embedding record with proper mapping to database schema
        let embedding = Embedding {
            id: uuid::Uuid::new_v4().to_string(),
            chunk_id: document_id.to_string(), // Use document_id instead of chunk_id for now
            vector: embedding_vector,
            model_name: self.get_model_name(),
            created_at: Utc::now().to_rfc3339(), // Use actual current time
        };

        // Store embedding
        db_helper
            .store_embedding(&embedding, document_id, chunk_index as i32, &chunk.content)
            .map_err(|e| format!("Failed to store embedding: {:?}", e))?;

        log::debug!(
            "Generated and stored embedding for chunk {} of document {}",
            chunk_index,
            document_id
        );
        Ok(chunk.id.clone())
    }

    fn get_model_name(&self) -> String {
        self.embedding_client.model.clone()
    }

    fn create_db_helper(&self) -> AgentResult<DatabaseHelper> {
        DatabaseHelper::from_env().map_err(|e| format!("Failed to create database helper: {:?}", e))
    }

    fn load_document(
        &self,
        db_helper: &DatabaseHelper,
        document_id: &str,
    ) -> AgentResult<Document> {
        db_helper
            .load_document(document_id)
            .map_err(|e| format!("Failed to load document: {:?}", e))?
            .ok_or_else(|| format!("Document not found: {}", document_id))
    }

    fn mark_status(
        &self,
        db_helper: &DatabaseHelper,
        document_id: &str,
        status: &EmbeddingStatus,
    ) -> AgentResult<()> {
        db_helper
            .update_embedding_status(document_id, status)
            .map_err(|e| format!("Failed to update embedding status: {:?}", e))
    }

    async fn process_document(
        &self,
        db_helper: &DatabaseHelper,
        document_id: &str,
        content: &str,
    ) -> AgentResult<u32> {
        // Split document into chunks
        let chunks = self.chunk_document(document_id, content, &self.chunk_config)?;

        // Generate embeddings for each chunk
        let mut embedding_count = 0;
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            match self
                .generate_and_store_embedding(db_helper, document_id, chunk_index as u32, chunk)
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

        Ok(embedding_count)
    }

    fn cleanup_existing_chunks(
        &self,
        db_helper: &DatabaseHelper,
        document_id: &str,
    ) -> AgentResult<()> {
        log::debug!("Cleaning up existing chunks for document: {}", document_id);

        let tables = ["document_chunks", "document_embeddings"];
        db_helper
            .delete_from_tables(document_id, &tables)
            .map_err(|e| format!("Failed to cleanup existing chunks: {:?}", e))?;

        log::debug!(
            "Successfully cleaned up existing chunks and embeddings for document: {}",
            document_id
        );
        Ok(())
    }
}
