use crate::s3_document_loader::S3DocumentLoaderAgentClient;
use crate::embedding_generator::EmbeddingGeneratorAgentClient;
use golem_rust::{agent_definition, agent_implementation, Schema};
use serde::{Deserialize, Serialize};
use std::string::String;

pub type AgentResult<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct SyncResult {
    pub bucket_results: Vec<BucketSyncResult>,
    pub total_buckets_processed: usize,
    pub total_documents_loaded: usize,
    pub total_embeddings_generated: u32,
    pub sync_timestamp: String,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct BucketSyncResult {
    pub bucket_name: String,
    pub documents_loaded: usize,
    pub embeddings_generated: u32,
    pub errors: Vec<String>,
    pub success: bool,
}

#[agent_definition(ephemeral)]
pub trait S3DocumentSyncAgent {
    fn new() -> Self;

    /// Sync all S3 buckets - load documents and generate embeddings for changes
    ///
    /// # Returns
    /// SyncResult with statistics about the sync operation
    async fn sync_all(&self) -> AgentResult<SyncResult>;
}

struct S3DocumentSyncAgentImpl;

#[agent_implementation]
impl S3DocumentSyncAgent for S3DocumentSyncAgentImpl {
    fn new() -> Self {
        Self
    }

    async fn sync_all(&self) -> AgentResult<SyncResult> {
        log::info!("Starting S3 document sync for all buckets");
        
        let mut sync_result = SyncResult {
            bucket_results: Vec::new(),
            total_buckets_processed: 0,
            total_documents_loaded: 0,
            total_embeddings_generated: 0,
            sync_timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // Create S3 document loader client
        let s3_loader = S3DocumentLoaderAgentClient::get();
        
        // Get list of all buckets
        let buckets: Vec<String> = match s3_loader.list_buckets().await {
            Ok(buckets) => buckets,
            Err(e) => {
                return Err(format!("Failed to list S3 buckets: {:?}", e));
            }
        };

        log::info!("Found {} buckets to sync", buckets.len());
        
        // Process each bucket
        for bucket in buckets {
            log::info!("Processing bucket: {}", bucket);
            
            let mut bucket_result = BucketSyncResult {
                bucket_name: bucket.clone(),
                documents_loaded: 0,
                embeddings_generated: 0,
                errors: Vec::new(),
                success: true,
            };
            
            // Load documents from bucket (this handles change detection)
            match s3_loader.load_documents(bucket.clone(), None).await {
                Ok(document_ids) => {
                    let document_ids: Vec<String> = document_ids;
                    bucket_result.documents_loaded = document_ids.len();
                    sync_result.total_documents_loaded += document_ids.len();
                    log::info!("Loaded {} documents from bucket {}", document_ids.len(), bucket);
                    
                    // Generate embeddings for the loaded documents if any
                    if !document_ids.is_empty() {
                        let embedding_generator = EmbeddingGeneratorAgentClient::get();
                        match embedding_generator.generate_embeddings_for_documents(document_ids).await {
                            Ok(embedding_count) => {
                                bucket_result.embeddings_generated = embedding_count;
                                sync_result.total_embeddings_generated += embedding_count;
                                log::info!("Generated {} embeddings for bucket {}", embedding_count, bucket);
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to generate embeddings for bucket {}: {:?}", bucket, e);
                                log::error!("{}", error_msg);
                                bucket_result.errors.push(error_msg);
                                bucket_result.success = false;
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("Failed to load documents from bucket {}: {:?}", bucket, e);
                    log::error!("{}", error_msg);
                    bucket_result.errors.push(error_msg);
                    bucket_result.success = false;
                }
            }
            
            sync_result.bucket_results.push(bucket_result);
            sync_result.total_buckets_processed += 1;
        }

        log::info!(
            "S3 sync completed - Buckets: {}, Documents: {}, Embeddings: {}, Errors: {}",
            sync_result.total_buckets_processed,
            sync_result.total_documents_loaded,
            sync_result.total_embeddings_generated,
            sync_result.bucket_results.iter().map(|r| r.errors.len()).sum::<usize>()
        );

        Ok(sync_result)
    }
}
