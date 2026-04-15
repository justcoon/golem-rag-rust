use crate::embedding_generator::EmbeddingGeneratorAgentClient;
use crate::models::LoadDocumentsRequest;
use crate::s3_document_loader::S3DocumentLoaderAgentClient;
use futures::future;
use golem_rust::{Schema, agent_definition, agent_implementation, endpoint};
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

#[agent_definition(mount = "/s3/sync", ephemeral)]
pub trait S3DocumentSyncAgent {
    fn new() -> Self;

    /// Sync all S3 buckets - load documents and generate embeddings for changes
    ///
    /// # Returns
    /// SyncResult with statistics about the sync operation
    #[endpoint(post = "")]
    async fn sync_all(&self) -> AgentResult<SyncResult>;
}

struct S3DocumentSyncAgentImpl;

async fn sync_bucket(bucket: String, s3_loader: &S3DocumentLoaderAgentClient) -> BucketSyncResult {
    log::info!("Processing bucket: {}", bucket);

    let mut bucket_result = BucketSyncResult {
        bucket_name: bucket.clone(),
        documents_loaded: 0,
        embeddings_generated: 0,
        errors: Vec::new(),
        success: true,
    };

    // Load documents from bucket (this handles change detection)
    match s3_loader
        .load_documents(bucket.clone(), LoadDocumentsRequest { prefix: None })
        .await
    {
        Ok(document_ids) => {
            let document_ids: Vec<String> = document_ids;
            bucket_result.documents_loaded = document_ids.len();
            log::info!(
                "Loaded {} documents from bucket {}",
                document_ids.len(),
                bucket
            );

            // Generate embeddings for the loaded documents if any
            if !document_ids.is_empty() {
                let embedding_generator = EmbeddingGeneratorAgentClient::new_phantom();
                match embedding_generator
                    .generate_embeddings_for_documents(document_ids)
                    .await
                {
                    Ok(embedding_count) => {
                        bucket_result.embeddings_generated = embedding_count;
                        log::info!(
                            "Generated {} embeddings for bucket {}",
                            embedding_count,
                            bucket
                        );
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to generate embeddings for bucket {}: {:?}",
                            bucket, e
                        );
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

    bucket_result
}

#[agent_implementation]
impl S3DocumentSyncAgent for S3DocumentSyncAgentImpl {
    fn new() -> Self {
        Self
    }

    async fn sync_all(&self) -> AgentResult<SyncResult> {
        log::info!("Starting S3 document sync for all buckets");

        // Create S3 document loader client
        let s3_loader = S3DocumentLoaderAgentClient::new_phantom();

        // Get list of all buckets
        let buckets: Vec<String> = s3_loader
            .list_buckets()
            .await
            .map_err(|e| format!("Failed to list S3 buckets: {:?}", e))?;

        log::info!("Found {} buckets to sync", buckets.len());

        // Process all buckets in parallel
        let bucket_futures: Vec<_> = buckets
            .into_iter()
            .map(|bucket| sync_bucket(bucket, &s3_loader))
            .collect();

        let bucket_results = future::join_all(bucket_futures).await;

        // Aggregate results
        let mut total_buckets_processed = 0;
        let mut total_documents_loaded = 0;
        let mut total_embeddings_generated = 0;

        for bucket_result in &bucket_results {
            total_buckets_processed += 1;
            total_documents_loaded += bucket_result.documents_loaded;
            total_embeddings_generated += bucket_result.embeddings_generated;
        }

        let sync_result = SyncResult {
            bucket_results,
            total_buckets_processed,
            total_documents_loaded,
            total_embeddings_generated,
            sync_timestamp: chrono::Utc::now().to_rfc3339(),
        };

        log::info!(
            "S3 sync completed - Buckets: {}, Documents: {}, Embeddings: {}, Errors: {}",
            sync_result.total_buckets_processed,
            sync_result.total_documents_loaded,
            sync_result.total_embeddings_generated,
            sync_result
                .bucket_results
                .iter()
                .map(|r| r.errors.len())
                .sum::<usize>()
        );

        Ok(sync_result)
    }
}
