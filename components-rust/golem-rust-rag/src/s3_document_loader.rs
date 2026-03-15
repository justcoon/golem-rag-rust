extern crate common_lib;
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
    db_url: String,
    s3_client: S3Client,
    bucket: String,
}

#[agent_implementation]
impl S3DocumentLoaderAgent for S3DocumentLoaderAgentImpl {
    fn new() -> Self {
        let db_url = std::env::var("DB_URL").expect("DB_URL environment variable must be set");

        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID")
            .expect("AWS_ACCESS_KEY_ID environment variable must be set");
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .expect("AWS_SECRET_ACCESS_KEY environment variable must be set");
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let bucket =
            std::env::var("AWS_S3_BUCKET").expect("AWS_S3_BUCKET environment variable must be set");
        let endpoint_url = std::env::var("S3_ENDPOINT_URL").ok();

        let s3_client = S3Client::new(access_key_id, secret_access_key, region, endpoint_url)
            .expect("Failed to create S3 client");

        Self {
            db_url,
            s3_client,
            bucket,
        }
    }

    fn load_documents_from_namespace(&mut self, namespace: String) -> AgentResult<Vec<String>> {
        log::info!("Loading documents from namespace: {}", namespace);

        // Step 1: Map namespace to S3 prefix
        let s3_prefix = match self.namespace_to_s3_prefix(&namespace) {
            Ok(prefix) => prefix,
            Err(e) => return Err(format!("Failed to create S3 prefix: {:?}", e)),
        };

        // Step 2: List documents in S3
        let mut s3_documents = match self.list_s3_documents(&s3_prefix) {
            Ok(docs) => docs,
            Err(e) => return Err(format!("Failed to list S3 documents: {:?}", e)),
        };

        // Step 3: Process each document
        let mut loaded_document_ids = Vec::new();
        let mut db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_url) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

        for s3_doc in &mut s3_documents {
            s3_doc.namespace = namespace.clone();

            // Check if document already exists
            match db_helper.document_exists_by_s3_key(&s3_doc.key) {
                Ok(exists) => {
                    if exists {
                        log::info!("Document {} already exists, skipping", s3_doc.key);
                        continue;
                    }
                }
                Err(e) => {
                    log::error!("Error checking if document exists: {:?}", e);
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
                metadata: DocumentMetadata {
                    source: "s3".to_string(),
                    namespace: namespace.clone(),
                    created_at: s3_doc.last_modified.clone(),
                    updated_at: s3_doc.last_modified.clone(),
                    tags: vec!["s3".to_string(), "auto-loaded".to_string()],
                    content_type: self.map_content_type(&content_type),
                    size_bytes: s3_doc.size_bytes,
                    source_metadata: {
                        let mut metadata = std::collections::HashMap::new();
                        metadata.insert("s3_key".to_string(), s3_doc.key.clone());
                        metadata.insert("s3_bucket".to_string(), self.bucket.clone());
                        metadata.insert("last_modified".to_string(), s3_doc.last_modified.clone());
                        metadata
                    },
                    metadata: {
                        let mut metadata = std::collections::HashMap::new();
                        metadata.insert("namespace".to_string(), namespace.clone());
                        metadata
                    },
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
        let s3_prefix = match self.namespace_to_s3_prefix(&namespace) {
            Ok(prefix) => prefix,
            Err(e) => return Err(format!("Failed to create S3 prefix: {:?}", e)),
        };
        let mut documents = match self.list_s3_documents(&s3_prefix) {
            Ok(docs) => docs,
            Err(e) => return Err(format!("Failed to list S3 documents: {:?}", e)),
        };
        for doc in &mut documents {
            doc.namespace = namespace.clone();
        }
        Ok(documents)
    }
}

impl S3DocumentLoaderAgentImpl {
    fn namespace_to_s3_prefix(&self, namespace: &str) -> AgentResult<String> {
        // Simple convention: namespace -> documents/{namespace}/
        // e.g., "legal" -> "documents/legal/"
        // e.g., "technical/reports" -> "documents/technical/reports/"
        let s3_prefix = format!("documents/{}/", namespace.trim_start_matches('/'));
        Ok(s3_prefix)
    }

    fn list_s3_documents(&self, s3_prefix: &str) -> AgentResult<Vec<S3DocumentSource>> {
        let list_response = match self.s3_client.list_objects(&self.bucket, Some(s3_prefix)) {
            Ok(response) => response,
            Err(e) => return Err(format!("Failed to list S3 objects: {:?}", e)),
        };
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
                namespace: String::new(), // Will be set by caller
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
}
