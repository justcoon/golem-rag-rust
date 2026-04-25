use crate::embedding_generator::EmbeddingGeneratorAgentClient;
use crate::models::ErrorResponse;
use crate::s3_document_loader::S3DocumentLoaderAgentClient;
use futures::future;
use golem_rust::{Schema, agent_definition, agent_implementation, description, endpoint, prompt};
use serde::{Deserialize, Serialize};
use std::string::String;

pub type AgentResult<T> = std::result::Result<T, ErrorResponse>;

const MAX_SYNC_HISTORY_SIZE: usize = 500;

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct SyncState {
    pub sync_history: Vec<SyncResult>,
    pub sync_schedule: Option<SyncSchedule>,
}

impl SyncState {
    pub fn add_sync_result(&mut self, sync_result: SyncResult) {
        self.sync_history.push(sync_result);
        if self.sync_history.len() > MAX_SYNC_HISTORY_SIZE {
            self.sync_history.remove(0); // Remove oldest entry
        }
    }

    pub fn set_schedule(&mut self, interval_minutes: u64, is_repetitive: bool) {
        let now = chrono::Utc::now().to_rfc3339();
        let next_execution = if is_repetitive {
            Some(
                chrono::Utc::now()
                    .checked_add_signed(chrono::Duration::minutes(interval_minutes as i64))
                    .unwrap_or_else(|| chrono::Utc::now())
                    .to_rfc3339(),
            )
        } else {
            Some(now.clone())
        };

        self.sync_schedule = Some(SyncSchedule {
            interval_minutes,
            is_repetitive,
            last_execution: None,
            next_execution,
        });
    }

    pub fn update_next_execution(&mut self) {
        if let Some(schedule) = &mut self.sync_schedule {
            if schedule.is_repetitive {
                schedule.last_execution = Some(chrono::Utc::now().to_rfc3339());
                schedule.next_execution = Some(
                    chrono::Utc::now()
                        .checked_add_signed(chrono::Duration::minutes(
                            schedule.interval_minutes as i64,
                        ))
                        .unwrap_or_else(|| chrono::Utc::now())
                        .to_rfc3339(),
                );
            } else {
                schedule.last_execution = schedule.next_execution.clone();
                schedule.next_execution = None;
            }
        }
    }

    pub fn delete_schedule(&mut self) {
        self.sync_schedule = None;
    }
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct SyncSchedule {
    pub interval_minutes: u64,
    pub is_repetitive: bool,
    pub last_execution: Option<String>,
    pub next_execution: Option<String>,
}

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

#[agent_definition(mount = "/s3/sync")]
pub trait S3DocumentSyncAgent {
    fn new() -> Self;

    /// Sync all S3 buckets - load documents and generate embeddings for changes
    ///
    /// # Returns
    /// SyncResult with statistics about the sync operation
    #[prompt("Sync all S3 buckets")]
    #[description(
        "Synchronizes all accessible S3 buckets by loading new/modified documents and generating embeddings. Processes all buckets in parallel for efficiency."
    )]
    #[endpoint(post = "/execute")]
    async fn sync_all(&mut self) -> AgentResult<SyncResult>;

    /// Configure sync schedule
    ///
    /// # Arguments
    /// * `interval_minutes` - Interval in minutes between sync executions
    /// * `is_repetitive` - Whether the schedule should repeat or be one-time
    #[prompt("Configure sync schedule")]
    #[description(
        "Sets up a schedule for automatic sync execution with specified interval and repetition."
    )]
    #[endpoint(post = "/schedule")]
    async fn set_sync_schedule(
        &mut self,
        interval_minutes: u64,
        is_repetitive: bool,
    ) -> AgentResult<String>;

    /// Get current sync schedule
    ///
    /// # Returns
    /// Current sync schedule configuration
    #[prompt("Get sync schedule")]
    #[description("Returns the current sync schedule configuration.")]
    #[endpoint(get = "/schedule")]
    async fn get_sync_schedule(&self) -> AgentResult<Option<SyncSchedule>>;

    /// Execute scheduled sync if due
    ///
    /// # Returns
    /// SyncResult if execution occurred, None if not due
    #[prompt("Execute scheduled sync")]
    #[description("Executes sync if scheduled time has arrived.")]
    async fn execute_scheduled_sync(&mut self) -> AgentResult<bool>;

    /// Delete sync schedule
    ///
    /// # Returns
    /// Confirmation message
    #[prompt("Delete sync schedule")]
    #[description("Removes the current sync schedule configuration.")]
    #[endpoint(delete = "/schedule")]
    async fn delete_sync_schedule(&mut self) -> AgentResult<String>;

    /// Get the sync history
    ///
    /// # Returns
    /// The current sync state containing all historical sync results
    #[prompt("Get sync history")]
    #[description("Returns the complete sync history with all previous sync results.")]
    #[endpoint(get = "/history")]
    async fn get_sync_history(&self) -> AgentResult<Vec<SyncResult>>;
}

struct S3DocumentSyncAgentImpl {
    state: SyncState,
}

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
    match s3_loader.load_documents(bucket.clone(), None).await {
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
                            "Failed to generate embeddings for bucket {}: {}",
                            bucket, e.message
                        );
                        log::error!("{}", error_msg);
                        bucket_result.errors.push(error_msg);
                        bucket_result.success = false;
                    }
                }
            }
        }
        Err(e) => {
            let error_msg = format!(
                "Failed to load documents from bucket {}: {}",
                bucket, e.message
            );
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
        Self {
            state: SyncState {
                sync_history: Vec::new(),
                sync_schedule: None,
            },
        }
    }

    async fn sync_all(&mut self) -> AgentResult<SyncResult> {
        log::info!("Starting S3 document sync for all buckets");

        // Create S3 document loader client
        let s3_loader = S3DocumentLoaderAgentClient::new_phantom();

        // Get list of all buckets
        let buckets: Vec<String> = s3_loader
            .list_buckets()
            .await
            .map_err(|e| format!("Failed to list S3 buckets: {}", e.message))?;

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

        // Append the sync result to history, maintaining max size limit
        self.state.add_sync_result(sync_result.clone());

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

    async fn set_sync_schedule(
        &mut self,
        interval_minutes: u64,
        is_repetitive: bool,
    ) -> AgentResult<String> {
        self.state.set_schedule(interval_minutes, is_repetitive);

        let schedule_type = if is_repetitive {
            "repetitive"
        } else {
            "one-time"
        };
        let message = format!(
            "Sync schedule configured: every {} minutes ({})",
            interval_minutes, schedule_type
        );

        log::info!("{}", message);

        // Trigger the scheduled execution
        let schedule_time = get_next_execution_time(interval_minutes);
        S3DocumentSyncAgentClient::get().schedule_execute_scheduled_sync(schedule_time);

        Ok(message)
    }

    async fn get_sync_schedule(&self) -> AgentResult<Option<SyncSchedule>> {
        Ok(self.state.sync_schedule.clone())
    }

    async fn execute_scheduled_sync(&mut self) -> AgentResult<bool> {
        log::info!("Executing scheduled sync");
        let _ = self.sync_all().await?;

        // Update execution times
        self.state.update_next_execution();

        // If schedule is still active and repetitive, reschedule
        if let Some(updated_schedule) = &self.state.sync_schedule
            && updated_schedule.is_repetitive
        {
            log::info!("Rescheduling next sync execution");
            let schedule_time = get_next_execution_time(updated_schedule.interval_minutes);
            S3DocumentSyncAgentClient::get().schedule_execute_scheduled_sync(schedule_time);
            Ok(true)
        } else {
            self.state.delete_schedule();
            Ok(false)
        }
    }

    async fn delete_sync_schedule(&mut self) -> AgentResult<String> {
        self.state.delete_schedule();
        log::info!("Sync schedule deleted");
        Ok("Sync schedule deleted successfully".to_string())
    }

    async fn get_sync_history(&self) -> AgentResult<Vec<SyncResult>> {
        Ok(self.state.sync_history.clone())
    }
}

fn get_next_execution_time(
    interval_minutes: u64,
) -> golem_rust::wasip2::clocks::wall_clock::Datetime {
    use golem_rust::wasip2::clocks::wall_clock::Datetime;
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let interval_secs = interval_minutes * 60;

    Datetime {
        seconds: now_secs + interval_secs,
        nanoseconds: 0,
    }
}
