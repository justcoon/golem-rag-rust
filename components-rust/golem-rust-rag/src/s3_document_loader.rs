extern crate common_lib;
use chrono::DateTime;
use common_lib::*;
use golem_rust::{agent_definition, agent_implementation};
use std::string::String;
use uuid::Uuid;

pub type AgentResult<T> = std::result::Result<T, String>;

#[agent_definition]
pub trait S3DocumentLoaderAgent {
    fn new() -> Self;

    /// Load documents from S3 using namespace mapping
    ///
    /// # Arguments
    /// * `namespace` - Logical namespace (e.g., "legal", "technical/reports")
    ///
    /// # Returns
    /// List of document IDs that were successfully loaded
    fn load_documents_from_namespace(&mut self, namespace: String) -> AgentResult<Vec<String>>;

    /// List available S3 documents for a namespace
    fn list_namespace_documents(&self, namespace: String) -> AgentResult<Vec<S3DocumentSource>>;
}

struct S3DocumentLoaderAgentImpl {
    s3_client: S3Client,
    bucket: String,
}

#[agent_implementation]
impl S3DocumentLoaderAgent for S3DocumentLoaderAgentImpl {
    fn new() -> Self {
        let s3_client =
            S3Client::from_env().expect("Failed to initialize S3 client from environment");
        let bucket =
            std::env::var("AWS_S3_BUCKET").expect("AWS_S3_BUCKET environment variable must be set");

        Self { s3_client, bucket }
    }

    fn load_documents_from_namespace(&mut self, namespace: String) -> AgentResult<Vec<String>> {
        log::info!("Loading documents from namespace: {}", namespace);

        // Step 1: List documents in S3 using namespace directly
        let s3_documents = self
            .list_s3_documents(&namespace)
            .map_err(|e| format!("Failed to list S3 documents: {:?}", e))?;

        // Step 2: Process each document
        let mut loaded_document_ids = Vec::new();
        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        for s3_doc in &s3_documents {
            // Check if document already exists and if it needs update
            match self.get_document_info_by_s3_key(&s3_doc.key, &namespace, &db_helper) {
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
                                "Document {} has no stored timestamp, treating as new",
                                s3_doc.key
                            );
                            true
                        }
                    };

                    if needs_update {
                        log::info!(
                            "Document {} has changed (S3: {}, DB: {}). Updating...",
                            s3_doc.key,
                            s3_doc.last_modified,
                            db_last_modified.unwrap_or("None".to_string())
                        );
                        if let Err(e) = db_helper.delete_document(&id) {
                            log::error!("Failed to delete old document version {}: {:?}", id, e);
                            continue;
                        }
                    } else {
                        log::info!("Document {} is up to date, skipping", s3_doc.key);
                        continue;
                    }
                }
                Ok(None) => {
                    log::info!("Document {} is new, loading...", s3_doc.key);
                }
                Err(e) => {
                    log::error!("Error checking document status for {}: {:?}", s3_doc.key, e);
                    continue;
                }
            }

            // Download document content
            let content = match self.s3_client.get_object(&self.bucket, &s3_doc.key) {
                Ok(content) => content,
                Err(e) => {
                    log::error!("Failed to download document {}: {:?}", s3_doc.key, e);
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
                namespace: namespace.clone(),
                tags: vec!["s3".to_string(), "auto-loaded".to_string()],
                size_bytes: s3_doc.size_bytes,
                created_at: s3_doc.last_modified.clone(),
                updated_at: s3_doc.last_modified.clone(),
                metadata: DocumentMetadata {
                    content_type: self.map_content_type(&content_type),
                    source_metadata: {
                        let mut metadata = std::collections::HashMap::new();
                        metadata.insert("s3_key".to_string(), s3_doc.key.clone());
                        metadata.insert("s3_bucket".to_string(), self.bucket.clone());
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
                        "Loaded document: {} (ID: {}) from namespace: {}",
                        s3_doc.key,
                        doc_id,
                        namespace
                    );
                }
                Err(e) => {
                    log::error!("Failed to store document {}: {:?}", s3_doc.key, e);
                }
            }
        }

        log::info!(
            "Successfully loaded {} documents from namespace: {}",
            loaded_document_ids.len(),
            namespace
        );
        Ok(loaded_document_ids)
    }

    fn list_namespace_documents(&self, namespace: String) -> AgentResult<Vec<S3DocumentSource>> {
        self.list_s3_documents(&namespace)
    }
}

impl S3DocumentLoaderAgentImpl {
    fn namespace_to_s3_prefix(&self, namespace: &str) -> AgentResult<String> {
        // Simple convention: namespace -> {namespace}/
        // e.g., "samp" -> "samp/"
        let trimmed_namespace = namespace.trim_start_matches('/');
        let s3_prefix = if trimmed_namespace.is_empty() {
            "".to_string()
        } else {
            format!("{}/", trimmed_namespace)
        };
        Ok(s3_prefix)
    }

    fn list_s3_documents(&self, namespace: &str) -> AgentResult<Vec<S3DocumentSource>> {
        let s3_prefix = self
            .namespace_to_s3_prefix(namespace)
            .map_err(|e| format!("Failed to create S3 prefix: {:?}", e))?;
        
        let list_response = self
            .s3_client
            .list_objects(&self.bucket, Some(&s3_prefix))
            .map_err(|e| format!("Failed to list S3 objects: {:?}", e))?;
        let mut documents = Vec::new();

        for obj in list_response.objects {
            // Skip directories and empty objects
            if obj.size_bytes == 0 {
                continue;
            }

            let document = S3DocumentSource {
                key: obj.key.clone(),
                size_bytes: obj.size_bytes,
                last_modified: obj.last_modified,
                content_type: obj.content_type,
                bucket: self.bucket.clone(),
                namespace: namespace.to_string(),
            };

            documents.push(document);
        }

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
        s3_key: &str,
        namespace: &str,
        db_helper: &DatabaseHelper,
    ) -> anyhow::Result<Option<(String, Option<String>)>> {
        let query = "SELECT id, metadata->'source_metadata'->>'last_modified' FROM documents WHERE metadata->'source_metadata'->>'s3_key' = $1 AND source = 's3' AND namespace = $2";
        let result = db_helper.connection.query(
            query,
            vec![
                PostgresDbValue::Text(s3_key.to_string()),
                PostgresDbValue::Text(namespace.to_string()),
            ],
        )?;

        if result.rows.is_empty() {
            return Ok(None);
        }

        let id = extract_db_field!(result.rows[0], 0, PostgresDbValue::Text(id) => id.clone());
        // JSONB ->> operator can return NULL, so we handle it with Option
        let last_modified = match &result.rows[0].values[1] {
            PostgresDbValue::Text(lm) if !lm.is_empty() => Some(lm.clone()),
            _ => None,
        };

        Ok(Some((id, last_modified)))
    }
}
