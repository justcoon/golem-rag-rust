use crate::database_helper::DatabaseHelperRagext;
use crate::models::*;
use chrono::DateTime;
use crate::common_lib::database::DatabaseHelper;
use crate::common_lib::s3_client::S3Client;
use crate::common_lib::s3_client::S3DocumentSource;
use crate::encode_params;

use golem_rust::{agent_definition, agent_implementation};
use std::string::String;
use uuid::Uuid;

pub type AgentResult<T> = std::result::Result<T, String>;

#[agent_definition]
pub trait S3DocumentLoaderAgent {
    fn new() -> Self;

    /// Load documents from S3 using bucket and optional prefix
    ///
    /// # Arguments
    /// * `bucket` - S3 bucket name
    /// * `prefix` - Optional S3 key prefix to filter documents
    ///
    /// # Returns
    /// List of document IDs that were successfully loaded
    fn load_documents(&self, bucket: String, prefix: Option<String>) -> AgentResult<Vec<String>>;

    /// List available S3 documents for a bucket with optional prefix
    fn list_documents(
        &self,
        bucket: String,
        prefix: Option<String>,
    ) -> AgentResult<Vec<S3DocumentSource>>;

    /// List all available S3 buckets
    fn list_buckets(&self) -> AgentResult<Vec<String>>;
}

struct S3DocumentLoaderAgentImpl {
    s3_client: S3Client,
}

#[agent_implementation]
impl S3DocumentLoaderAgent for S3DocumentLoaderAgentImpl {
    fn new() -> Self {
        let s3_client =
            S3Client::from_env().expect("Failed to initialize S3 client from environment");

        Self { s3_client }
    }

    fn load_documents(&self, bucket: String, prefix: Option<String>) -> AgentResult<Vec<String>> {
        log::info!(
            "Loading documents from bucket: {}, prefix: {:?}",
            bucket,
            prefix
        );

        // Step 1: List documents in S3 using prefix directly
        let s3_documents = self
            .list_s3_documents(&bucket, prefix.as_deref())
            .map_err(|e| format!("Failed to list S3 documents: {:?}", e))?;

        // Step 2: Process each document
        let mut loaded_document_ids = Vec::new();
        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        for s3_doc in &s3_documents {
            // Check if document already exists and if it needs update
            match self.get_document_info_by_s3_key(
                &bucket,
                &s3_doc.key,
                &s3_doc.namespace,
                &db_helper,
            ) {
                Ok(Some((id, db_last_modified))) => {
                    let needs_update = match db_last_modified {
                        Some(ref db_timestamp) => {
                            match (
                                DateTime::parse_from_rfc3339(&s3_doc.last_modified),
                                DateTime::parse_from_rfc3339(db_timestamp),
                            ) {
                                (Ok(s3_dt), Ok(db_dt)) => s3_dt > db_dt,
                                _ => {
                                    log::warn!("Failed to parse timestamps for comparison, falling back to string comparison. S3: {}, DB: {}", s3_doc.last_modified, db_timestamp);
                                    s3_doc.last_modified > *db_timestamp
                                }
                            }
                        }
                        None => {
                            log::info!(
                                "Document {} in bucket {} has no stored timestamp, treating as new",
                                s3_doc.key,
                                bucket
                            );
                            true
                        }
                    };

                    if needs_update {
                        log::info!(
                            "Document {} in bucket {} has changed (S3: {}, DB: {}). Updating...",
                            s3_doc.key,
                            bucket,
                            s3_doc.last_modified,
                            db_last_modified.unwrap_or("None".to_string())
                        );
                        if let Err(e) = db_helper.delete_document(&id) {
                            log::error!("Failed to delete old document version {}: {:?}", id, e);
                            continue;
                        }
                    } else {
                        log::info!(
                            "Document {} in bucket {} is up to date, skipping",
                            s3_doc.key,
                            bucket
                        );
                        continue;
                    }
                }
                Ok(None) => {
                    log::info!(
                        "Document {} in bucket {} is new, loading...",
                        s3_doc.key,
                        bucket
                    );
                }
                Err(e) => {
                    log::error!(
                        "Error checking document status for {} in bucket {}: {:?}",
                        s3_doc.key,
                        bucket,
                        e
                    );
                    continue;
                }
            }

            // Download document content
            let content = match self.s3_client.get_object(&bucket, &s3_doc.key) {
                Ok(content) => content,
                Err(e) => {
                    log::error!(
                        "Failed to download document {} from bucket {}: {:?}",
                        s3_doc.key,
                        bucket,
                        e
                    );
                    continue;
                }
            };

            // Infer content type
            let content_type = if !s3_doc.content_type.is_empty() {
                s3_doc.content_type.clone()
            } else {
                self.infer_content_type(&s3_doc.key)
                    .unwrap_or_else(|| "application/octet-stream".to_string())
            };

            // Create document
            let document = Document {
                id: Uuid::new_v4().to_string(),
                title: self.extract_title_from_key(&s3_doc.key),
                content: match String::from_utf8(content) {
                    Ok(content) => content,
                    Err(e) => {
                        log::error!("Failed to convert document content to string: {:?}", e);
                        continue;
                    }
                },
                source: "s3".to_string(),
                namespace: s3_doc.namespace.clone(),
                tags: vec!["s3".to_string(), "auto-loaded".to_string()],
                size_bytes: s3_doc.size_bytes,
                created_at: s3_doc.last_modified.clone(),
                updated_at: s3_doc.last_modified.clone(),
                metadata: DocumentMetadata {
                    content_type: self.map_content_type(&content_type),
                    source_metadata: {
                        let mut metadata = std::collections::HashMap::new();
                        metadata.insert("s3_key".to_string(), s3_doc.key.clone());
                        metadata.insert("s3_bucket".to_string(), bucket.clone());
                        metadata.insert("last_modified".to_string(), s3_doc.last_modified.clone());
                        metadata
                    },
                    metadata: std::collections::HashMap::new(),
                },
            };

            // Store document in database
            match db_helper.store_document(&document) {
                Ok(document_id) => {
                    let doc_id = document_id.clone();
                    loaded_document_ids.push(document_id);
                    log::info!(
                        "Loaded document: {} (ID: {}) from bucket: {}, namespace: {}",
                        s3_doc.key,
                        doc_id,
                        bucket,
                        s3_doc.namespace
                    );
                }
                Err(e) => {
                    log::error!("Failed to store document {}: {:?}", s3_doc.key, e);
                }
            }
        }

        log::info!(
            "Successfully loaded {} documents from bucket: {}, prefix: {:?}",
            loaded_document_ids.len(),
            bucket,
            prefix
        );
        Ok(loaded_document_ids)
    }

    fn list_documents(
        &self,
        bucket: String,
        prefix: Option<String>,
    ) -> AgentResult<Vec<S3DocumentSource>> {
        self.list_s3_documents(&bucket, prefix.as_deref())
    }

    fn list_buckets(&self) -> AgentResult<Vec<String>> {
        let buckets_response = self
            .s3_client
            .list_buckets()
            .map_err(|e| format!("Failed to list S3 buckets: {:?}", e))?;

        let bucket_names: Vec<String> = buckets_response
            .buckets
            .into_iter()
            .map(|bucket| bucket.name)
            .collect();

        Ok(bucket_names)
    }
}

impl S3DocumentLoaderAgentImpl {
    fn list_s3_documents(
        &self,
        bucket: &str,
        prefix: Option<&str>,
    ) -> AgentResult<Vec<S3DocumentSource>> {
        let list_response = self
            .s3_client
            .list_objects(bucket, prefix, true) // Use recursive listing to get all files in subfolders
            .map_err(|e| format!("Failed to list S3 objects: {:?}", e))?;

        // Filter out directories and empty objects, bucket is already set by S3 client
        let documents: Vec<S3DocumentSource> = list_response
            .objects
            .into_iter()
            .filter(|obj| obj.size_bytes > 0) // Skip directories and empty objects
            .collect();

        Ok(documents)
    }

    fn infer_content_type(&self, key: &str) -> Option<String> {
        let key_lower = key.to_lowercase();

        if key_lower.ends_with(".txt") {
            Some("text/plain".to_string())
        } else if key_lower.ends_with(".md") {
            Some("text/markdown".to_string())
        } else if key_lower.ends_with(".pdf") {
            Some("application/pdf".to_string())
        } else if key_lower.ends_with(".html") || key_lower.ends_with(".htm") {
            Some("text/html".to_string())
        } else if key_lower.ends_with(".json") {
            Some("application/json".to_string())
        } else {
            None
        }
    }

    fn map_content_type(&self, content_type: &str) -> ContentType {
        match content_type {
            "text/plain" => ContentType::Text,
            "text/markdown" => ContentType::Markdown,
            "application/pdf" => ContentType::Pdf,
            "text/html" => ContentType::Html,
            "application/json" => ContentType::Json,
            _ => ContentType::Text,
        }
    }

    fn extract_title_from_key(&self, key: &str) -> String {
        // Extract filename from S3 key and remove extension
        let filename = key.split('/').next_back().unwrap_or(key);
        let title = filename.split('.').next().unwrap_or(filename);

        // Convert to title case
        title
            .replace('_', " ")
            .replace("-", " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn get_document_info_by_s3_key(
        &self,
        bucket: &str,
        s3_key: &str,
        namespace: &str,
        db_helper: &DatabaseHelper,
    ) -> anyhow::Result<Option<(String, Option<String>)>> {
        let query = "SELECT id, metadata->'source_metadata'->>'last_modified' FROM documents WHERE metadata->'source_metadata'->>'s3_bucket' = $1 AND metadata->'source_metadata'->>'s3_key' = $2 AND source = 's3' AND namespace = $3";
        let result = db_helper
            .connection
            .query(query, encode_params![bucket, s3_key, namespace])?;

        use crate::common_lib::database::decode::DbResultDecoder;
        let results: Vec<(String, Option<String>)> =
            <(String, Option<String>)>::decode_result(result)
                .map_err(|e| anyhow::anyhow!("Failed to decode document info: {:?}", e))?;

        Ok(results.into_iter().next())
    }
}
