# RAG Pipeline Implementation Proposal

## Overview

This proposal outlines a comprehensive RAG (Retrieval-Augmented Generation) pipeline implementation for the Golem application using PostgreSQL with pgvector for vector search capabilities.

## System Architecture

### Core Components

1. **Document Loader Agent** (`DocumentLoaderAgent`)
   - Loads documents from S3 to database with optional prefix filtering
   - Handles S3 integration with configurable endpoints
   - Manages duplicate detection and content type inference

2. **Embedding Generator Agent** (`EmbeddingGeneratorAgent`)
   - Generates and stores embeddings for specific documents
   - Handles document chunking and embedding status tracking
   - Supports mock and real embedding models

3. **RAG Coordinator Agent** (`RagCoordinatorAgent`)
   - Orchestrates document loading and embedding generation workflow
   - Coordinates between DocumentLoaderAgent and EmbeddingGeneratorAgent
   - Provides comprehensive processing status and retry mechanisms

4. **Document Agent** (`DocumentAgent`) - **Ephemeral**
   - Retrieves document content and metadata from database
   - Provides document access for search results and applications
   - Optimized for read-heavy document retrieval operations

5. **Search Agent** (`SearchAgent`) - **Ephemeral**
   - Executes semantic search on documents using stored embeddings
   - Supports advanced filtering and similar document discovery
   - Provides text highlighting and relevance scores

### Data Flow

```
S3 Storage → DocumentLoaderAgent → Database → EmbeddingGeneratorAgent → Embeddings → SearchAgent → Search Results
                                                                 ↓
                                                          DocumentAgent → Document Content
```

### Agent Workflow

```
RagCoordinatorAgent (Orchestrator)
    ├── DocumentLoaderAgent → S3 Storage → Database
    ├── EmbeddingGeneratorAgent → Embedding Model → Database
    └── SearchAgent → Database (with pgvector) → Search Results
```

## Data Models

### Core Types (in common-lib)

```rust
use serde::{Deserialize, Serialize};
use golem_rust::Schema;
use uuid::Uuid;

// Document management
#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub metadata: DocumentMetadata,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub source: String,
    pub created_at: String, // ISO timestamp
    pub updated_at: String, // ISO timestamp
    pub tags: Vec<String>,
    pub content_type: ContentType,
    pub metadata: serde_json::Value, // Additional metadata
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Markdown,
    Pdf,
    Html,
    Json,
}

// Text chunking
#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub content: String,
    pub chunk_index: u32,
    pub start_pos: u32,
    pub end_pos: u32,
    pub token_count: Option<u32>,
}

// Vector embeddings
#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct Embedding {
    pub id: Uuid,
    pub chunk_id: Uuid,
    pub vector: Vec<f32>,
    pub model_name: String,
    pub created_at: String, // ISO timestamp
}

// Search results
#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk: DocumentChunk,
    pub document: Document,
    pub similarity_score: f32,
    pub relevance_explanation: Option<String>,
}

// Search queries
#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub query_vector: Option<Vec<f32>>,
    pub filters: SearchFilters,
    pub limit: u32,
    pub similarity_threshold: f32,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct SearchFilters {
    pub tags: Vec<String>,
    pub sources: Vec<String>,
    pub content_types: Vec<ContentType>,
    pub date_range: Option<DateRange>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct DateRange {
    pub start: String, // ISO timestamp
    pub end: String,   // ISO timestamp
}

// Configuration types
#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct ChunkConfig {
    pub chunk_size: u32,
    pub chunk_overlap: u32,
    pub respect_sentences: bool,
    pub min_chunk_size: u32,
    pub max_chunk_size: u32,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            chunk_overlap: 200,
            respect_sentences: true,
            min_chunk_size: 100,
            max_chunk_size: 2000,
        }
    }
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct IndexingRequest {
    pub document: Document,
    pub chunk_config: Option<ChunkConfig>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct IndexingResult {
    pub document_id: Uuid,
    pub chunks_created: u32,
    pub embeddings_generated: u32,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct RagResponse {
    pub query: String,
    pub context: Vec<DocumentChunk>,
    pub response: String,
    pub sources: Vec<Uuid>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct DocumentFilters {
    pub tags: Vec<String>,
    pub sources: Vec<String>,
    pub content_types: Vec<ContentType>,
    pub date_range: Option<DateRange>,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            tags: vec![],
            sources: vec![],
            content_types: vec![],
            date_range: None,
        }
    }
}
```

## S3 Document Source Integration

### S3 Client Implementation

Based on the Golem AI project pattern, I've added an S3 client to `common-lib` for document ingestion with configurable endpoints:

```rust
use common_lib::s3_client::{S3Client, S3Config, S3DocumentSource};

// S3 Configuration from environment - supports custom endpoints
let s3_config = S3Config {
    access_key_id: env::var("AWS_ACCESS_KEY_ID")?,
    secret_access_key: env::var("AWS_SECRET_ACCESS_KEY")?,
    region: env::var("AWS_REGION")?,
    bucket: env::var("AWS_S3_BUCKET")?,
    endpoint_url: env::var("S3_ENDPOINT_URL").ok(), // Optional custom endpoint
};

let s3_client = S3Client::new(
    s3_config.access_key_id,
    s3_config.secret_access_key,
    s3_config.region,
    s3_config.endpoint_url, // Custom endpoint or None for AWS default
)?;

// List documents from S3
let documents = s3_client.list_objects(&s3_config.bucket, Some("documents/"))?;

// Download document content
let content = s3_client.get_object(&s3_config.bucket, &document_key)?;
```

### Document Loader Agent

**Purpose**: Load documents from S3 to database with optional prefix filtering

```rust
#[agent_definition]
pub trait DocumentLoaderAgent {
    fn new() -> Self;
    
    /// Load documents from S3 to database
    /// 
    /// # Arguments
    /// * `s3_prefix` - Optional S3 prefix to filter documents (e.g., "documents/", "pdfs/")
    /// 
    /// # Returns
    /// List of document IDs that were successfully loaded
    fn load_documents_from_s3(&mut self, s3_prefix: Option<&str>) -> Result<Vec<Uuid>>;
    
    /// List available documents in S3 (without loading)
    fn list_s3_documents(&self, s3_prefix: Option<&str>) -> Result<Vec<S3DocumentSource>>;
}

struct DocumentLoaderAgentImpl {
    db_url: String,
    s3_client: S3Client,
    s3_bucket: String,
}

#[agent_implementation]
impl DocumentLoaderAgent for DocumentLoaderAgentImpl {
    fn new() -> Self {
        let db_url = env::var("DB_URL")
            .expect("DB_URL environment variable must be set");
        
        let s3_client = S3Client::new(
            env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID required"),
            env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY required"),
            env::var("AWS_REGION").expect("AWS_REGION required"),
            env::var("S3_ENDPOINT_URL").ok(), // Optional custom endpoint
        ).expect("Failed to create S3 client");
        
        let s3_bucket = env::var("AWS_S3_BUCKET")
            .expect("AWS_S3_BUCKET required");
        
        Self { db_url, s3_client, s3_bucket }
    }
    
    fn load_documents_from_s3(&mut self, s3_prefix: Option<&str>) -> Result<Vec<Uuid>> {
        let s3_response = self.s3_client.list_objects(&self.s3_bucket, s3_prefix)?;
        let mut loaded_document_ids = Vec::new();
        
        // Connect to database
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        for s3_doc in s3_response.objects {
            // Skip non-document files
            if !self.is_document_file(&s3_doc.key) {
                continue;
            }
            
            // Check if document already exists
            if self.document_exists(&mut connection, &s3_doc.key)? {
                continue; // Skip already loaded documents
            }
            
            // Download document content
            let content = self.s3_client.get_object(&self.s3_bucket, &s3_doc.key)?;
            let content_str = String::from_utf8(content)?;
            
            // Create document record
            let document_id = Uuid::new_v4();
            let document = Document {
                id: document_id,
                title: s3_doc.key.split('/').last().unwrap_or(&s3_doc.key).to_string(),
                content: content_str,
                metadata: DocumentMetadata {
                    source: format!("s3://{}/{}", self.s3_bucket, s3_doc.key),
                    content_type: self.infer_content_type(&s3_doc.key),
                    created_at: Timestamp::now(),
                    updated_at: Timestamp::now(),
                    tags: vec!["s3".to_string(), "auto-loaded".to_string()],
                    metadata: serde_json::json!({
                        "s3_bucket": self.s3_bucket,
                        "s3_key": s3_doc.key,
                        "size_bytes": s3_doc.size_bytes,
                        "last_modified": s3_doc.last_modified,
                    }),
                },
            };
            
            // Store document in database
            self.store_document(&mut connection, document)?;
            loaded_document_ids.push(document_id);
        }
        
        Ok(loaded_document_ids)
    }
    
    fn list_s3_documents(&self, s3_prefix: Option<&str>) -> Result<Vec<S3DocumentSource>> {
        let response = self.s3_client.list_objects(&self.s3_bucket, s3_prefix)?;
        Ok(response.objects)
    }
}

// Helper methods for DocumentLoaderAgent
impl DocumentLoaderAgentImpl {
    fn is_document_file(&self, key: &str) -> bool {
        key.ends_with(".txt") || key.ends_with(".md") || 
        key.ends_with(".pdf") || key.ends_with(".docx") ||
        key.ends_with(".rtf") || key.ends_with(".html")
    }
    
    fn infer_content_type(&self, key: &str) -> String {
        match key.split('.').last() {
            Some("txt") => "text/plain".to_string(),
            Some("md") => "text/markdown".to_string(),
            Some("pdf") => "application/pdf".to_string(),
            Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
            Some("rtf") => "application/rtf".to_string(),
            Some("html") => "text/html".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }
    
    fn document_exists(&self, connection: &mut DbConnection, s3_key: &str) -> Result<bool> {
        let query = "SELECT COUNT(*) FROM documents WHERE metadata->>'s3_key' = $1";
        let result = connection.query(query, &[&s3_key])?;
        Ok(result.len() > 0 && result[0].get::<_, i64>(0) > 0)
    }
    
    fn store_document(&self, connection: &mut DbConnection, document: Document) -> Result<()> {
        let query = r#"
            INSERT INTO documents (id, title, content, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#;
        
        connection.execute(query, &[
            &document.id.to_string(),
            &document.title,
            &document.content,
            &serde_json::to_string(&document.metadata)?,
            &document.metadata.created_at,
            &document.metadata.updated_at,
        ])?;
        
        Ok(())
    }
}
```

### Embedding Generator Agent

**Purpose**: Generate and store embeddings for a specific document

```rust
#[agent_definition]
pub trait EmbeddingGeneratorAgent {
    fn new() -> Self;
    
    /// Generate and store embeddings for a specific document
    /// 
    /// # Arguments
    /// * `document_id` - UUID of the document to process
    /// 
    /// # Returns
    /// Number of embeddings generated for the document
    fn generate_embeddings_for_document(&mut self, document_id: Uuid) -> Result<usize>;
    
    /// Get embedding status for a document
    fn get_embedding_status(&self, document_id: Uuid) -> Result<EmbeddingStatus>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EmbeddingStatus {
    NotProcessed,
    InProgress,
    Completed { chunk_count: usize },
    Failed { error: String },
}

struct EmbeddingGeneratorAgentImpl {
    db_url: String,
    embedding_model: String,
}

#[agent_implementation]
impl EmbeddingGeneratorAgent for EmbeddingGeneratorAgentImpl {
    fn new() -> Self {
        let db_url = env::var("DB_URL")
            .expect("DB_URL environment variable must be set");
        
        let embedding_model = env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "mock-embedding-v1".to_string());
        
        Self { db_url, embedding_model }
    }
    
    fn generate_embeddings_for_document(&mut self, document_id: Uuid) -> Result<usize> {
        // Connect to database
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        // Load document from database
        let document = self.load_document(&mut connection, document_id)?;
        
        // Mark document as in progress
        self.update_embedding_status(&mut connection, document_id, EmbeddingStatus::InProgress)?;
        
        // Split document into chunks
        let chunks = self.chunk_document(&document.content, ChunkConfig {
            chunk_size: 1000,
            chunk_overlap: 200,
        })?;
        
        // Generate embeddings for each chunk
        let mut embedding_count = 0;
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            let embedding = self.generate_embedding(chunk)?;
            
            // Store embedding
            self.store_embedding(&mut connection, document_id, chunk_index, chunk, embedding)?;
            embedding_count += 1;
        }
        
        // Mark document as completed
        self.update_embedding_status(&mut connection, document_id, EmbeddingStatus::Completed { chunk_count: embedding_count })?;
        
        Ok(embedding_count)
    }
    
    fn get_embedding_status(&self, document_id: Uuid) -> Result<EmbeddingStatus> {
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        let query = "SELECT embedding_status, chunk_count FROM document_embeddings WHERE document_id = $1";
        let result = connection.query(query, &[&document_id.to_string()])?;
        
        if result.is_empty() {
            Ok(EmbeddingStatus::NotProcessed)
        } else {
            let status_str = result[0].get::<_, String>(0);
            match status_str.as_str() {
                "in_progress" => Ok(EmbeddingStatus::InProgress),
                "completed" => {
                    let chunk_count = result[0].get::<_, i64>(1) as usize;
                    Ok(EmbeddingStatus::Completed { chunk_count })
                },
                "failed" => Ok(EmbeddingStatus::Failed { 
                    error: result[0].get::<_, String>(2) 
                }),
                _ => Ok(EmbeddingStatus::NotProcessed),
            }
        }
    }
}

// Helper methods for EmbeddingGeneratorAgent
impl EmbeddingGeneratorAgentImpl {
    fn load_document(&self, connection: &mut DbConnection, document_id: Uuid) -> Result<Document> {
        let query = "SELECT id, title, content, metadata, created_at, updated_at FROM documents WHERE id = $1";
        let result = connection.query(query, &[&document_id.to_string()])?;
        
        if result.is_empty() {
            return Err(anyhow::anyhow!("Document not found: {}", document_id));
        }
        
        let row = &result[0];
        let document = Document {
            id: Uuid::parse_str(&row.get::<_, String>(0))?,
            title: row.get::<_, String>(1),
            content: row.get::<_, String>(2),
            metadata: serde_json::from_str(&row.get::<_, String>(3))?,
        };
        
        Ok(document)
    }
    
    fn chunk_document(&self, content: &str, config: ChunkConfig) -> Result<Vec<String>> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = content.chars().collect();
        let chunk_size = config.chunk_size;
        let overlap = config.chunk_overlap;
        
        let mut start = 0;
        while start < chars.len() {
            let end = std::cmp::min(start + chunk_size, chars.len());
            let chunk: String = chars[start..end].iter().collect();
            chunks.push(chunk);
            
            if end >= chars.len() {
                break;
            }
            
            start = end - overlap;
        }
        
        Ok(chunks)
    }
    
    fn generate_embedding(&self, text: &str) -> Result<Vector> {
        match self.embedding_model.as_str() {
            "mock-embedding-v1" => {
                // Mock embedding for testing
                let mut embedding = vec![0.0; 1536];
                for (i, byte) in text.as_bytes().iter().enumerate() {
                    embedding[i % embedding.len()] = (*byte as f32) / 255.0;
                }
                Ok(Vector(embedding))
            },
            _ => Err(anyhow::anyhow!("Unsupported embedding model: {}", self.embedding_model))
        }
    }
    
    fn store_embedding(&self, connection: &mut DbConnection, document_id: Uuid, chunk_index: usize, chunk: &str, embedding: Vector) -> Result<()> {
        let query = r#"
            INSERT INTO document_embeddings (document_id, chunk_index, chunk_text, embedding, created_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (document_id, chunk_index) DO UPDATE SET
                chunk_text = EXCLUDED.chunk_text,
                embedding = EXCLUDED.embedding,
                created_at = EXCLUDED.created_at
        "#;
        
        connection.execute(query, &[
            &document_id.to_string(),
            &(chunk_index as i64),
            chunk,
            &embedding,
            &Timestamp::now(),
        ])?;
        
        Ok(())
    }
    
    fn update_embedding_status(&self, connection: &mut DbConnection, document_id: Uuid, status: EmbeddingStatus) -> Result<()> {
        let (status_str, chunk_count, error_msg) = match status {
            EmbeddingStatus::NotProcessed => ("not_processed", None, None),
            EmbeddingStatus::InProgress => ("in_progress", None, None),
            EmbeddingStatus::Completed { chunk_count } => ("completed", Some(chunk_count as i64), None),
            EmbeddingStatus::Failed { error } => ("failed", None, Some(error)),
        };
        
        let query = r#"
            INSERT INTO document_embeddings (document_id, embedding_status, chunk_count, error_message, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (document_id) DO UPDATE SET
                embedding_status = EXCLUDED.embedding_status,
                chunk_count = EXCLUDED.chunk_count,
                error_message = EXCLUDED.error_message,
                updated_at = EXCLUDED.updated_at
        "#;
        
        connection.execute(query, &[
            &document_id.to_string(),
            status_str,
            &chunk_count,
            &error_msg,
            &Timestamp::now(),
        ])?;
        
        Ok(())
    }
}
```

### RAG Coordinator Agent

**Purpose**: Coordinate document loading and embedding generation workflow

```rust
#[agent_definition]
pub trait RagCoordinatorAgent {
    fn new() -> Self;
    
    /// Process documents from S3 with automatic embedding generation
    /// 
    /// # Arguments
    /// * `s3_prefix` - Optional S3 prefix to filter documents (e.g., "documents/", "pdfs/")
    /// 
    /// # Returns
    /// Processing summary with document IDs and embedding counts
    fn process_s3_documents(&mut self, s3_prefix: Option<&str>) -> Result<ProcessingSummary>;
    
    /// Get processing status for a specific S3 prefix
    fn get_processing_status(&self, s3_prefix: &str) -> Result<ProcessingStatus>;
    
    /// Retry failed embeddings for documents
    fn retry_failed_embeddings(&mut self, s3_prefix: Option<&str>) -> Result<RetrySummary>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessingSummary {
    pub s3_prefix: String,
    pub documents_loaded: usize,
    pub document_ids: Vec<Uuid>,
    pub embeddings_generated: usize,
    pub embeddings_failed: usize,
    pub processing_time_ms: u64,
    pub timestamp: Timestamp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessingStatus {
    pub s3_prefix: String,
    pub total_documents: usize,
    pub loaded_documents: usize,
    pub completed_embeddings: usize,
    pub failed_embeddings: usize,
    pub in_progress_embeddings: usize,
    pub last_updated: Timestamp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrySummary {
    pub attempted_retries: usize,
    pub successful_retries: usize,
    pub still_failed: usize,
}

struct RagCoordinatorAgentImpl {
    db_url: String,
    document_loader_agent_id: String,
    embedding_generator_agent_id: String,
}

#[agent_implementation]
impl RagCoordinatorAgent for RagCoordinatorAgentImpl {
    fn new() -> Self {
        let db_url = env::var("DB_URL")
            .expect("DB_URL environment variable must be set");
        
        let document_loader_agent_id = env::var("DOCUMENT_LOADER_AGENT_ID")
            .unwrap_or_else(|_| "document-loader-agent".to_string());
        
        let embedding_generator_agent_id = env::var("EMBEDDING_GENERATOR_AGENT_ID")
            .unwrap_or_else(|_| "embedding-generator-agent".to_string());
        
        Self { 
            db_url, 
            document_loader_agent_id, 
            embedding_generator_agent_id 
        }
    }
    
    fn process_s3_documents(&mut self, s3_prefix: Option<&str>) -> Result<ProcessingSummary> {
        let start_time = std::time::Instant::now();
        let prefix = s3_prefix.unwrap_or("").to_string();
        
        // Connect to database for tracking
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        // Step 1: Load documents from S3
        println!("Loading documents from S3 with prefix: '{}'", prefix);
        let document_ids = self.invoke_document_loader(s3_prefix)?;
        let documents_loaded = document_ids.len();
        
        if documents_loaded == 0 {
            return Ok(ProcessingSummary {
                s3_prefix: prefix,
                documents_loaded: 0,
                document_ids: vec![],
                embeddings_generated: 0,
                embeddings_failed: 0,
                processing_time_ms: start_time.elapsed().as_millis() as u64,
                timestamp: Timestamp::now(),
            });
        }
        
        println!("Loaded {} documents from S3", documents_loaded);
        
        // Step 2: Generate embeddings for all loaded documents
        println!("Generating embeddings for {} documents", documents_loaded);
        let (embeddings_generated, embeddings_failed) = self.generate_embeddings_for_documents(&document_ids)?;
        
        let processing_time = start_time.elapsed().as_millis() as u64;
        
        // Step 3: Store processing summary
        self.store_processing_summary(&mut connection, &prefix, documents_loaded, embeddings_generated, embeddings_failed)?;
        
        Ok(ProcessingSummary {
            s3_prefix: prefix,
            documents_loaded,
            document_ids,
            embeddings_generated,
            embeddings_failed,
            processing_time_ms: processing_time,
            timestamp: Timestamp::now(),
        })
    }
    
    fn get_processing_status(&self, s3_prefix: &str) -> Result<ProcessingStatus> {
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        let query = r#"
            SELECT 
                COUNT(*) as total_documents,
                COUNT(CASE WHEN metadata->>'source' LIKE 's3://%' THEN 1 END) as loaded_documents,
                COUNT(CASE WHEN embedding_status = 'completed' THEN 1 END) as completed_embeddings,
                COUNT(CASE WHEN embedding_status = 'failed' THEN 1 END) as failed_embeddings,
                COUNT(CASE WHEN embedding_status = 'in_progress' THEN 1 END) as in_progress_embeddings,
                MAX(updated_at) as last_updated
            FROM documents d
            LEFT JOIN document_embeddings e ON d.id = e.document_id
            WHERE d.metadata->>'s3_key' LIKE $1 || '%'
        "#;
        
        let result = connection.query(query, &[&s3_prefix])?;
        
        if result.is_empty() {
            return Ok(ProcessingStatus {
                s3_prefix: s3_prefix.to_string(),
                total_documents: 0,
                loaded_documents: 0,
                completed_embeddings: 0,
                failed_embeddings: 0,
                in_progress_embeddings: 0,
                last_updated: Timestamp::now(),
            });
        }
        
        let row = &result[0];
        Ok(ProcessingStatus {
            s3_prefix: s3_prefix.to_string(),
            total_documents: row.get::<_, i64>(0) as usize,
            loaded_documents: row.get::<_, i64>(1) as usize,
            completed_embeddings: row.get::<_, i64>(2) as usize,
            failed_embeddings: row.get::<_, i64>(3) as usize,
            in_progress_embeddings: row.get::<_, i64>(4) as usize,
            last_updated: row.get::<_, Timestamp>(5),
        })
    }
    
    fn retry_failed_embeddings(&mut self, s3_prefix: Option<&str>) -> Result<RetrySummary> {
        let prefix = s3_prefix.unwrap_or("").to_string();
        
        // Find documents with failed embeddings
        let failed_document_ids = self.get_failed_document_ids(&prefix)?;
        let attempted_retries = failed_document_ids.len();
        
        if attempted_retries == 0 {
            return Ok(RetrySummary {
                attempted_retries: 0,
                successful_retries: 0,
                still_failed: 0,
            });
        }
        
        println!("Retrying embeddings for {} failed documents", attempted_retries);
        
        let mut successful_retries = 0;
        let mut still_failed = 0;
        
        for document_id in failed_document_ids {
            match self.invoke_embedding_generator(document_id) {
                Ok(_) => {
                    successful_retries += 1;
                    println!("Successfully retried embeddings for document {}", document_id);
                },
                Err(e) => {
                    still_failed += 1;
                    println!("Failed to retry embeddings for document {}: {}", document_id, e);
                }
            }
        }
        
        Ok(RetrySummary {
            attempted_retries,
            successful_retries,
            still_failed,
        })
    }
}

// Helper methods for RagCoordinatorAgent
impl RagCoordinatorAgentImpl {
    fn invoke_document_loader(&mut self, s3_prefix: Option<&str>) -> Result<Vec<Uuid>> {
        // In a real implementation, this would use Golem RPC to call the DocumentLoaderAgent
        // For now, we'll simulate the call
        
        // Simulate RPC call to DocumentLoaderAgent
        println!("Invoking DocumentLoaderAgent with prefix: {:?}", s3_prefix);
        
        // This would be: let document_ids = document_loader_agent.load_documents_from_s3(s3_prefix)?;
        // For simulation, we'll return mock IDs
        let mock_document_ids = vec![
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];
        
        Ok(mock_document_ids)
    }
    
    fn generate_embeddings_for_documents(&self, document_ids: &[Uuid]) -> Result<(usize, usize)> {
        let mut embeddings_generated = 0;
        let mut embeddings_failed = 0;
        
        for document_id in document_ids {
            match self.invoke_embedding_generator(*document_id) {
                Ok(count) => {
                    embeddings_generated += count;
                    println!("Generated {} embeddings for document {}", count, document_id);
                },
                Err(e) => {
                    embeddings_failed += 1;
                    println!("Failed to generate embeddings for document {}: {}", document_id, e);
                }
            }
        }
        
        Ok((embeddings_generated, embeddings_failed))
    }
    
    fn invoke_embedding_generator(&self, document_id: Uuid) -> Result<usize> {
        // In a real implementation, this would use Golem RPC to call the EmbeddingGeneratorAgent
        println!("Invoking EmbeddingGeneratorAgent for document: {}", document_id);
        
        // Simulate embedding generation
        // This would be: let count = embedding_agent.generate_embeddings_for_document(document_id)?;
        let mock_embedding_count = 5; // Simulate 5 chunks/embeddings per document
        
        Ok(mock_embedding_count)
    }
    
    fn get_failed_document_ids(&self, s3_prefix: &str) -> Result<Vec<Uuid>> {
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        let query = r#"
            SELECT d.id 
            FROM documents d
            JOIN document_embeddings e ON d.id = e.document_id
            WHERE d.metadata->>'s3_key' LIKE $1 || '%'
            AND e.embedding_status = 'failed'
        "#;
        
        let result = connection.query(query, &[&s3_prefix])?;
        
        let mut failed_ids = Vec::new();
        for row in result {
            let id_str = row.get::<_, String>(0);
            failed_ids.push(Uuid::parse_str(&id_str)?);
        }
        
        Ok(failed_ids)
    }
    
    fn store_processing_summary(&self, connection: &mut DbConnection, s3_prefix: &str, documents_loaded: usize, embeddings_generated: usize, embeddings_failed: usize) -> Result<()> {
        let query = r#"
            INSERT INTO processing_summaries (s3_prefix, documents_loaded, embeddings_generated, embeddings_failed, created_at)
            VALUES ($1, $2, $3, $4, $5)
        "#;
        
        connection.execute(query, &[
            s3_prefix,
            &(documents_loaded as i64),
            &(embeddings_generated as i64),
            &(embeddings_failed as i64),
            &Timestamp::now(),
        ])?;
        
        Ok(())
    }
}
```

### Agent Usage Examples

```rust
// Simple coordination - one call to handle everything
let coordinator = RagCoordinatorAgent::new();
let summary = coordinator.process_s3_documents(Some("documents/"))?;

println!("Processing Summary:");
println!("  S3 Prefix: {}", summary.s3_prefix);
println!("  Documents Loaded: {}", summary.documents_loaded);
println!("  Embeddings Generated: {}", summary.embeddings_generated);
println!("  Embeddings Failed: {}", summary.embeddings_failed);
println!("  Processing Time: {}ms", summary.processing_time_ms);

// Check processing status
let status = coordinator.get_processing_status("documents/");
println!("Status: {:?}", status);

// Retry failed embeddings if any
if summary.embeddings_failed > 0 {
    let retry_summary = coordinator.retry_failed_embeddings(Some("documents/"));
    println!("Retry Summary: {:?}", retry_summary);
}
```

### Individual Agent Usage (if needed)

```rust
// Manual coordination - use individual agents
let loader = DocumentLoaderAgent::new();
let document_ids = loader.load_documents_from_s3(Some("pdfs/"))?;

let generator = EmbeddingGeneratorAgent::new();
for document_id in document_ids {
    match generator.generate_embeddings_for_document(document_id) {
        Ok(count) => println!("Generated {} embeddings", count),
        Err(e) => println!("Failed to process {}: {}", document_id, e),
    }
}
```

### Environment Variables

```bash
# Database Configuration
DB_URL=postgresql://postgres:password@localhost:5432/golem_rag

# AWS S3 Configuration
AWS_ACCESS_KEY_ID=your_access_key_here
AWS_SECRET_ACCESS_KEY=your_secret_key_here
AWS_REGION=us-east-1  # Default: us-east-1
AWS_S3_BUCKET=your-document-bucket

# Optional: Custom S3-compatible endpoint
# S3_ENDPOINT_URL=https://your-minio-server.com

# Embedding Configuration
EMBEDDING_MODEL=mock-embedding-v1

# Agent Configuration
DOCUMENT_LOADER_AGENT_ID=document-loader-agent
EMBEDDING_GENERATOR_AGENT_ID=embedding-generator-agent
RAG_COORDINATOR_AGENT_ID=rag-coordinator-agent
```

### Benefits of Coordinator Agent

1. **Single Point of Entry**: One call handles the entire workflow
2. **Automatic Orchestration**: Coordinates between loader and embedding agents
3. **Status Tracking**: Provides comprehensive processing status
4. **Error Recovery**: Built-in retry mechanisms for failed embeddings
5. **Performance Monitoring**: Tracks processing time and success rates
6. **Simplified API**: Reduces complexity for end users

### Complete RAG Agent Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────────┐
│ RagCoordinator  │───▶│ DocumentLoader   │───▶│ S3 Storage          │
│ Agent           │    │ Agent            │    │ (documents/)       │
└─────────────────┘    └──────────────────┘    └─────────────────────┘
         │
         ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────────┐
│ Processing      │    │ EmbeddingGen     │───▶│ Embedding Model     │
│ Summary         │◀───│ Agent            │    │ (mock/embeddings)   │
└─────────────────┘    └──────────────────┘    └─────────────────────┘
         │
         ▼
┌─────────────────┐
│ PostgreSQL      │
│ Database        │
│ (documents +    │
│  embeddings)    │
└─────────────────┘
```

### Processing Workflow

```rust
// Step 1: Process documents with coordination (recommended)
let coordinator = RagCoordinatorAgent::new();
let result = coordinator.process_s3_documents(Some("documents/"))?;

// Step 2: Monitor progress
let status = coordinator.get_processing_status("documents/");
while status.in_progress_embeddings > 0 {
    println!("Still processing {} embeddings...", status.in_progress_embeddings);
    std::thread::sleep(std::time::Duration::from_secs(5));
    status = coordinator.get_processing_status("documents/");
}

// Step 3: Handle failures if needed
if status.failed_embeddings > 0 {
    let retry_result = coordinator.retry_failed_embeddings(Some("documents/"));
    println!("Retried {} failed embeddings", retry_result.successful_retries);
}
```

### Document Agent

**Purpose**: Retrieve document content and metadata from database (Ephemeral)

```rust
#[agent_definition]
pub trait DocumentAgent {
    fn new() -> Self;
    
    /// Get document content by ID
    fn get_document(&self, document_id: Uuid) -> Result<Option<Document>>;
    
    /// Get document metadata without content
    fn get_document_metadata(&self, document_id: Uuid) -> Result<Option<DocumentMetadata>>;
    
    /// List documents with optional filters
    fn list_documents(&self, filters: Option<DocumentFilters>, limit: Option<usize>) -> Result<Vec<Document>>;
    
    /// Get document chunks for a specific document
    fn get_document_chunks(&self, document_id: Uuid) -> Result<Vec<DocumentChunk>>;
    
    /// Check if document exists
    fn document_exists(&self, document_id: Uuid) -> Result<bool>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentFilters {
    pub content_types: Vec<String>,        // e.g., ["text/plain", "application/pdf"]
    pub tags: Vec<String>,                 // e.g., ["s3", "auto-loaded"]
    pub date_range: Option<DateRange>,      // Filter by creation/update date
    pub s3_prefix: Option<String>,          // Filter by S3 prefix
    pub min_size_bytes: Option<u64>,       // Minimum document size
    pub max_size_bytes: Option<u64>,       // Maximum document size
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub chunk_id: Uuid,
    pub document_id: Uuid,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub start_pos: usize,
    pub end_pos: usize,
    pub token_count: usize,
}

struct DocumentAgentImpl {
    db_url: String,
}

#[agent_implementation]
impl DocumentAgent for DocumentAgentImpl {
    fn new() -> Self {
        let db_url = env::var("DB_URL")
            .expect("DB_URL environment variable must be set");
        
        Self { db_url }
    }
    
    fn get_document(&self, document_id: Uuid) -> Result<Option<Document>> {
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        let query = "SELECT id, title, content, metadata, created_at, updated_at FROM documents WHERE id = $1";
        let result = connection.query(query, &[&document_id.to_string()])?;
        
        if result.is_empty() {
            Ok(None)
        } else {
            let row = &result[0];
            let document = Document {
                id: Uuid::parse_str(&row.get::<_, String>(0))?,
                title: row.get::<_, String>(1),
                content: row.get::<_, String>(2),
                metadata: serde_json::from_str(&row.get::<_, String>(3))?,
            };
            Ok(Some(document))
        }
    }
    
    fn get_document_metadata(&self, document_id: Uuid) -> Result<Option<DocumentMetadata>> {
        if let Some(document) = self.get_document(document_id)? {
            Ok(Some(document.metadata))
        } else {
            Ok(None)
        }
    }
    
    fn list_documents(&self, filters: Option<DocumentFilters>, limit: Option<usize>) -> Result<Vec<Document>> {
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        let limit = limit.unwrap_or(50);
        let (sql_query, params) = self.build_document_list_query(filters, limit)?;
        
        let result = connection.query(&sql_query, &params)?;
        
        let mut documents = Vec::new();
        for row in result {
            let document = Document {
                id: Uuid::parse_str(&row.get::<_, String>(0))?,
                title: row.get::<_, String>(1),
                content: String::new(), // Don't include content in list
                metadata: serde_json::from_str(&row.get::<_, String>(2))?,
            };
            documents.push(document);
        }
        
        Ok(documents)
    }
    
    fn get_document_chunks(&self, document_id: Uuid) -> Result<Vec<DocumentChunk>> {
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        let query = r#"
            SELECT chunk_id, chunk_index, chunk_text, start_pos, end_pos, token_count
            FROM document_embeddings 
            WHERE document_id = $1 AND embedding_status = 'completed'
            ORDER BY chunk_index
        "#;
        
        let result = connection.query(query, &[&document_id.to_string()])?;
        
        let mut chunks = Vec::new();
        for row in result {
            let chunk = DocumentChunk {
                chunk_id: Uuid::parse_str(&row.get::<_, String>(0))?,
                document_id,
                chunk_index: row.get::<_, i64>(1) as usize,
                chunk_text: row.get::<_, String>(2),
                start_pos: row.get::<_, i64>(3) as usize,
                end_pos: row.get::<_, i64>(4) as usize,
                token_count: row.get::<_, i64>(5) as usize,
            };
            chunks.push(chunk);
        }
        
        Ok(chunks)
    }
    
    fn document_exists(&self, document_id: Uuid) -> Result<bool> {
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        let query = "SELECT COUNT(*) FROM documents WHERE id = $1";
        let result = connection.query(query, &[&document_id.to_string()])?;
        
        Ok(result.len() > 0 && result[0].get::<_, i64>(0) > 0)
    }
}

// Helper methods for DocumentAgent
impl DocumentAgentImpl {
    fn build_document_list_query(&self, filters: Option<DocumentFilters>, limit: usize) -> Result<(String, Vec<String>)> {
        let mut query_conditions = vec!["1=1".to_string()];
        let mut params = vec![];
        let mut param_index = 1;
        
        if let Some(filters) = filters {
            // Add content type filters
            if !filters.content_types.is_empty() {
                let placeholders: Vec<String> = filters.content_types.iter()
                    .map(|_| format!("${}", param_index + 1))
                    .collect();
                query_conditions.push(format!("metadata->>'content_type' IN ({})", placeholders.join(", ")));
                for content_type in &filters.content_types {
                    params.push(content_type.clone());
                    param_index += 1;
                }
            }
            
            // Add tag filters
            if !filters.tags.is_empty() {
                for tag in &filters.tags {
                    query_conditions.push(format!("metadata->'tags' ? ${}", param_index));
                    params.push(tag.clone());
                    param_index += 1;
                }
            }
            
            // Add date range filter
            if let Some(date_range) = &filters.date_range {
                query_conditions.push(format!("created_at >= ${}", param_index));
                params.push(date_range.start.to_string());
                param_index += 1;
                
                query_conditions.push(format!("created_at <= ${}", param_index));
                params.push(date_range.end.to_string());
                param_index += 1;
            }
            
            // Add S3 prefix filter
            if let Some(s3_prefix) = &filters.s3_prefix {
                query_conditions.push(format!("metadata->>'s3_key' LIKE ${}", param_index));
                params.push(format!("{}%", s3_prefix));
                param_index += 1;
            }
            
            // Add size filters
            if let Some(min_size) = filters.min_size_bytes {
                query_conditions.push(format!("(metadata->>'size_bytes')::bigint >= ${}", param_index));
                params.push(min_size.to_string());
                param_index += 1;
            }
            
            if let Some(max_size) = filters.max_size_bytes {
                query_conditions.push(format!("(metadata->>'size_bytes')::bigint <= ${}", param_index));
                params.push(max_size.to_string());
                param_index += 1;
            }
        }
        
        let where_clause = if query_conditions.len() > 1 {
            format!("WHERE {}", query_conditions[1..].join(" AND "))
        } else {
            String::new()
        };
        
        let sql_query = format!(r#"
            SELECT id, title, metadata, created_at, updated_at
            FROM documents
            {}
            ORDER BY created_at DESC
            LIMIT {}
        "#, where_clause, limit);
        
        Ok((sql_query, params))
    }
}

impl Default for DocumentFilters {
    fn default() -> Self {
        Self {
            content_types: vec![],
            tags: vec![],
            date_range: None,
            s3_prefix: None,
            min_size_bytes: None,
            max_size_bytes: None,
        }
    }
}
```

### Search Agent

**Purpose**: Execute semantic search on documents using stored embeddings (Ephemeral)

```rust
#[agent_definition]
pub trait SearchAgent {
    fn new() -> Self;
    
    /// Search for documents using semantic similarity
    /// 
    /// # Arguments
    /// * `query` - Search query text
    /// * `limit` - Maximum number of results to return (default: 10)
    /// * `threshold` - Similarity threshold (0.0 to 1.0, default: 0.7)
    /// 
    /// # Returns
    /// List of search results with relevance scores
    fn search(&self, query: &str, limit: Option<usize>, threshold: Option<f32>) -> Result<Vec<SearchResult>>;
    
    /// Search documents with metadata filters
    fn search_with_filters(&self, query: &str, filters: SearchFilters, limit: Option<usize>, threshold: Option<f32>) -> Result<Vec<SearchResult>>;
    
    /// Hybrid search combining semantic similarity and keyword matching
    /// 
    /// # Arguments
    /// * `query` - Search query text
    /// * `keyword_weight` - Weight for keyword search (0.0 to 1.0, default: 0.3)
    /// * `semantic_weight` - Weight for semantic search (0.0 to 1.0, default: 0.7)
    /// * `filters` - Optional metadata filters
    /// * `limit` - Maximum number of results to return (default: 10)
    /// 
    /// # Returns
    /// List of hybrid search results with combined relevance scores
    fn hybrid_search(&self, query: &str, keyword_weight: Option<f32>, semantic_weight: Option<f32>, filters: Option<SearchFilters>, limit: Option<usize>) -> Result<Vec<HybridSearchResult>>;
    
    /// Get similar documents to a specific document
    fn find_similar_documents(&self, document_id: Uuid, limit: Option<usize>) -> Result<Vec<SearchResult>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub document_id: Uuid,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub title: String,
    pub similarity_score: f32,
    pub metadata: DocumentMetadata,
    pub highlight: Option<String>, // Highlighted matching text
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub document_id: Uuid,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub title: String,
    pub metadata: DocumentMetadata,
    pub highlight: Option<String>, // Highlighted matching text
    pub semantic_score: f32,      // Semantic similarity score (0.0 to 1.0)
    pub keyword_score: f32,       // Keyword matching score (0.0 to 1.0)
    pub combined_score: f32,      // Weighted combined relevance score (0.0 to 1.0)
    pub match_type: MatchType,    // Type of match found
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MatchType {
    SemanticOnly,     // Only semantic match
    KeywordOnly,      // Only keyword match
    BothMatch,        // Both semantic and keyword match
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchFilters {
    pub content_types: Vec<String>,        // e.g., ["text/plain", "application/pdf"]
    pub tags: Vec<String>,                 // e.g., ["s3", "auto-loaded"]
    pub date_range: Option<DateRange>,      // Filter by creation/update date
    pub s3_prefix: Option<String>,          // Filter by S3 prefix
    pub min_size_bytes: Option<u64>,       // Minimum document size
    pub max_size_bytes: Option<u64>,       // Maximum document size
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DateRange {
    pub start: Timestamp,
    pub end: Timestamp,
}

struct SearchAgentImpl {
    db_url: String,
    embedding_model: String,
}

#[agent_implementation]
impl SearchAgent for SearchAgentImpl {
    fn new() -> Self {
        let db_url = env::var("DB_URL")
            .expect("DB_URL environment variable must be set");
        
        let embedding_model = env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "mock-embedding-v1".to_string());
        
        Self { db_url, embedding_model }
    }
    
    fn search(&self, query: &str, limit: Option<usize>, threshold: Option<f32>) -> Result<Vec<SearchResult>> {
        let filters = SearchFilters::default();
        self.search_with_filters(query, filters, limit, threshold)
    }
    
    fn search_with_filters(&self, query: &str, filters: SearchFilters, limit: Option<usize>, threshold: Option<f32>) -> Result<Vec<SearchResult>> {
        let limit = limit.unwrap_or(10);
        let threshold = threshold.unwrap_or(0.7);
        
        // Connect to database
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        // Step 1: Generate embedding for the query
        let query_embedding = self.generate_embedding(query)?;
        
        // Step 2: Build SQL query with filters
        let (sql_query, params) = self.build_search_query(&filters, limit, threshold)?;
        
        // Step 3: Execute similarity search using pgvector
        let results = connection.query(&sql_query, &params)?;
        
        // Step 4: Process results and create SearchResult objects
        let mut search_results = Vec::new();
        for row in results {
            let result = self.create_search_result_from_row(row, &query_embedding)?;
            search_results.push(result);
        }
        
        // Step 5: Highlight matching text in results
        for result in &mut search_results {
            result.highlight = self.highlight_text(query, &result.chunk_text);
        }
        
        Ok(search_results)
    }
    
    fn find_similar_documents(&self, document_id: Uuid, limit: Option<usize>) -> Result<Vec<SearchResult>> {
        let limit = limit.unwrap_or(5);
        
        // Get the document's embedding
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        let query = "SELECT embedding FROM document_embeddings WHERE document_id = $1 AND chunk_index = 0 LIMIT 1";
        let result = connection.query(query, &[&document_id.to_string()])?;
        
        if result.is_empty() {
            return Ok(vec![]);
        }
        
        let reference_embedding: Vector = result[0].get::<_, Vector>(0);
        
        // Find similar documents using the reference embedding
        let similarity_query = format!(r#"
            SELECT 
                d.id, d.title, d.content, d.metadata, d.created_at, d.updated_at,
                e.chunk_index, e.chunk_text, e.embedding,
                1 - (e.embedding <=> $1) as similarity
            FROM documents d
            JOIN document_embeddings e ON d.id = e.document_id
            WHERE d.id != $2
            AND e.embedding_status = 'completed'
            HAVING similarity > 0.5
            ORDER BY similarity DESC
            LIMIT $3
        "#);
        
        let similarity_results = connection.query(&similarity_query, &[&reference_embedding, &document_id.to_string(), &(limit as i64)])?;
        
        let mut similar_results = Vec::new();
        for row in similarity_results {
            let similarity_score = row.get::<_, f32>("similarity");
            let result = self.create_search_result_from_row(row, &reference_embedding)?;
            similar_results.push(result);
        }
        
        Ok(similar_results)
    }
    
    fn hybrid_search(&self, query: &str, keyword_weight: Option<f32>, semantic_weight: Option<f32>, filters: Option<SearchFilters>, limit: Option<usize>) -> Result<Vec<HybridSearchResult>> {
        let keyword_weight = keyword_weight.unwrap_or(0.3);
        let semantic_weight = semantic_weight.unwrap_or(0.7);
        let limit = limit.unwrap_or(10);
        
        // Validate weights sum to 1.0
        if (keyword_weight + semantic_weight - 1.0).abs() > 0.001 {
            return Err(anyhow::anyhow!("Keyword weight and semantic weight must sum to 1.0"));
        }
        
        // Step 1: Get semantic search results
        let semantic_results = match &filters {
            Some(f) => self.search_with_filters(query, f.clone(), Some(limit * 2), Some(0.3))?,
            None => self.search(query, Some(limit * 2), Some(0.3))?,
        };
        
        // Step 2: Get keyword search results
        let keyword_results = self.keyword_search(query, filters.as_ref(), limit * 2)?;
        
        // Step 3: Combine and score results
        let mut hybrid_results = Vec::new();
        let mut seen_documents = std::collections::HashSet::new();
        
        // Process semantic results
        for semantic_result in semantic_results {
            let doc_key = (semantic_result.document_id, semantic_result.chunk_index);
            seen_documents.insert(doc_key.clone());
            
            // Find matching keyword result
            let keyword_score = keyword_results
                .iter()
                .find(|k| k.document_id == semantic_result.document_id && k.chunk_index == semantic_result.chunk_index)
                .map(|k| k.keyword_score)
                .unwrap_or(0.0);
            
            let combined_score = (semantic_result.similarity_score * semantic_weight) + (keyword_score * keyword_weight);
            let match_type = if keyword_score > 0.0 {
                MatchType::BothMatch
            } else {
                MatchType::SemanticOnly
            };
            
            hybrid_results.push(HybridSearchResult {
                document_id: semantic_result.document_id,
                chunk_index: semantic_result.chunk_index,
                chunk_text: semantic_result.chunk_text.clone(),
                title: semantic_result.title.clone(),
                metadata: semantic_result.metadata.clone(),
                highlight: semantic_result.highlight.clone(),
                semantic_score: semantic_result.similarity_score,
                keyword_score,
                combined_score,
                match_type,
            });
        }
        
        // Process keyword-only results
        for keyword_result in keyword_results {
            let doc_key = (keyword_result.document_id, keyword_result.chunk_index);
            if !seen_documents.contains(&doc_key) {
                seen_documents.insert(doc_key.clone());
                
                let combined_score = keyword_result.keyword_score * keyword_weight;
                
                hybrid_results.push(HybridSearchResult {
                    document_id: keyword_result.document_id,
                    chunk_index: keyword_result.chunk_index,
                    chunk_text: keyword_result.chunk_text.clone(),
                    title: keyword_result.title.clone(),
                    metadata: keyword_result.metadata.clone(),
                    highlight: keyword_result.highlight.clone(),
                    semantic_score: 0.0,
                    keyword_score: keyword_result.keyword_score,
                    combined_score,
                    match_type: MatchType::KeywordOnly,
                });
            }
        }
        
        // Step 4: Sort by combined score and limit results
        hybrid_results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap_or(std::cmp::Ordering::Equal));
        hybrid_results.truncate(limit);
        
        Ok(hybrid_results)
    }
}

// Helper methods for SearchAgent
impl SearchAgentImpl {
    fn generate_embedding(&self, text: &str) -> Result<Vector> {
        match self.embedding_model.as_str() {
            "mock-embedding-v1" => {
                // Mock embedding for testing
                let mut embedding = vec![0.0; 1536];
                for (i, byte) in text.as_bytes().iter().enumerate() {
                    embedding[i % embedding.len()] = (*byte as f32) / 255.0;
                }
                Ok(Vector(embedding))
            },
            _ => Err(anyhow::anyhow!("Unsupported embedding model: {}", self.embedding_model))
        }
    }
    
    fn keyword_search(&self, query: &str, filters: Option<&SearchFilters>, limit: usize) -> Result<Vec<HybridSearchResult>> {
        let mut connection = DbConnection::open(&self.db_url)
            .map_err(|e| anyhow::anyhow!("Failed to connect to database: {:?}", e))?;
        
        // Build keyword search query using PostgreSQL full-text search
        let (sql_query, params) = self.build_keyword_search_query(query, filters, limit)?;
        
        let result = connection.query(&sql_query, &params)?;
        
        let mut keyword_results = Vec::new();
        for row in result {
            let keyword_score = row.get::<_, f32>("keyword_score");
            let document_id = Uuid::parse_str(&row.get::<_, String>("document_id"))?;
            let chunk_index = row.get::<_, i64>("chunk_index") as usize;
            
            keyword_results.push(HybridSearchResult {
                document_id,
                chunk_index,
                chunk_text: row.get::<_, String>("chunk_text"),
                title: row.get::<_, String>("title"),
                metadata: serde_json::from_str(&row.get::<_, String>("metadata"))?,
                highlight: self.highlight_text(query, &row.get::<_, String>("chunk_text")),
                semantic_score: 0.0,
                keyword_score,
                combined_score: keyword_score,
                match_type: MatchType::KeywordOnly,
            });
        }
        
        Ok(keyword_results)
    }
    
    fn build_keyword_search_query(&self, query: &str, filters: Option<&SearchFilters>, limit: usize) -> Result<(String, Vec<String>)> {
        let mut query_conditions = vec!["e.embedding_status = 'completed'".to_string()];
        let mut params = vec![];
        let mut param_index = 1;
        
        // Add full-text search condition
        query_conditions.push(format!("to_tsvector('english', e.chunk_text) @@ plainto_tsquery('english', ${})", param_index));
        params.push(query.to_string());
        param_index += 1;
        
        // Add keyword score calculation
        query_conditions.push("ts_rank(to_tsvector('english', e.chunk_text), plainto_tsquery('english', $1)) > 0.1".to_string());
        
        // Add filters if provided
        if let Some(filters) = filters {
            if !filters.content_types.is_empty() {
                let placeholders: Vec<String> = filters.content_types.iter()
                    .map(|_| format!("${}", param_index + 1))
                    .collect();
                query_conditions.push(format!("d.metadata->>'content_type' IN ({})", placeholders.join(", ")));
                for content_type in &filters.content_types {
                    params.push(content_type.clone());
                    param_index += 1;
                }
            }
            
            if !filters.tags.is_empty() {
                for tag in &filters.tags {
                    query_conditions.push(format!("d.metadata->'tags' ? ${}", param_index));
                    params.push(tag.clone());
                    param_index += 1;
                }
            }
            
            if let Some(s3_prefix) = &filters.s3_prefix {
                query_conditions.push(format!("d.metadata->>'s3_key' LIKE ${}", param_index));
                params.push(format!("{}%", s3_prefix));
                param_index += 1;
            }
        }
        
        let where_clause = if query_conditions.len() > 2 {
            format!("WHERE {}", query_conditions[1..].join(" AND "))
        } else {
            String::new()
        };
        
        let sql_query = format!(r#"
            SELECT 
                d.id as document_id,
                d.title,
                d.metadata,
                e.chunk_index,
                e.chunk_text,
                ts_rank(to_tsvector('english', e.chunk_text), plainto_tsquery('english', $1)) as keyword_score
            FROM documents d
            JOIN document_embeddings e ON d.id = e.document_id
            {}
            ORDER BY keyword_score DESC
            LIMIT {}
        "#, where_clause, limit);
        
        Ok((sql_query, params))
    }
    
    fn build_search_query(&self, filters: &SearchFilters, limit: usize, threshold: f32) -> Result<(String, Vec<String>)> {
        let mut query_conditions = vec!["e.embedding_status = 'completed'".to_string()];
        let mut params = vec![];
        let mut param_index = 1;
        
        // Add content type filters
        if !filters.content_types.is_empty() {
            let placeholders: Vec<String> = filters.content_types.iter()
                .map(|_| format!("${}", param_index + 1))
                .collect();
            query_conditions.push(format!("d.metadata->>'content_type' IN ({})", placeholders.join(", ")));
            for content_type in &filters.content_types {
                params.push(content_type.clone());
                param_index += 1;
            }
        }
        
        // Add tag filters
        if !filters.tags.is_empty() {
            for tag in &filters.tags {
                query_conditions.push(format!("d.metadata->'tags' ? ${}", param_index));
                params.push(tag.clone());
                param_index += 1;
            }
        }
        
        // Add date range filter
        if let Some(date_range) = &filters.date_range {
            query_conditions.push(format!("d.created_at >= ${}", param_index));
            params.push(date_range.start.to_string());
            param_index += 1;
            
            query_conditions.push(format!("d.created_at <= ${}", param_index));
            params.push(date_range.end.to_string());
            param_index += 1;
        }
        
        // Add S3 prefix filter
        if let Some(s3_prefix) = &filters.s3_prefix {
            query_conditions.push(format!("d.metadata->>'s3_key' LIKE ${}", param_index));
            params.push(format!("{}%", s3_prefix));
            param_index += 1;
        }
        
        // Add size filters
        if let Some(min_size) = filters.min_size_bytes {
            query_conditions.push(format!("(d.metadata->>'size_bytes')::bigint >= ${}", param_index));
            params.push(min_size.to_string());
            param_index += 1;
        }
        
        if let Some(max_size) = filters.max_size_bytes {
            query_conditions.push(format!("(d.metadata->>'size_bytes')::bigint <= ${}", param_index));
            params.push(max_size.to_string());
            param_index += 1;
        }
        
        // Build the final query
        let where_clause = if query_conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", query_conditions.join(" AND "))
        };
        
        let sql_query = format!(r#"
            SELECT 
                d.id, d.title, d.content, d.metadata, d.created_at, d.updated_at,
                e.chunk_index, e.chunk_text, e.embedding,
                1 - (e.embedding <=> ${}) as similarity
            FROM documents d
            JOIN document_embeddings e ON d.id = e.document_id
            {}
            HAVING similarity > {}
            ORDER BY similarity DESC
            LIMIT {}
        "#, param_index + 1, where_clause, threshold, limit);
        
        // Add the query embedding parameter placeholder
        params.push("query_embedding".to_string()); // This will be replaced with actual embedding
        
        Ok((sql_query, params))
    }
    
    fn create_search_result_from_row(&self, row: golem_rust::bindings::golem::rdbms::types::Row, query_embedding: &Vector) -> Result<SearchResult> {
        let document_id = Uuid::parse_str(&row.get::<_, String>("id"))?;
        let chunk_index = row.get::<_, i64>("chunk_index") as usize;
        let chunk_text = row.get::<_, String>("chunk_text");
        let title = row.get::<_, String>("title");
        let similarity_score = row.get::<_, f32>("similarity");
        let metadata: DocumentMetadata = serde_json::from_str(&row.get::<_, String>("metadata"))?;
        
        Ok(SearchResult {
            document_id,
            chunk_index,
            chunk_text,
            title,
            similarity_score,
            metadata,
            highlight: None, // Will be set later
        })
    }
    
    fn highlight_text(&self, query: &str, text: &str) -> Option<String> {
        // Simple highlighting implementation
        let query_lower = query.to_lowercase();
        let text_lower = text.to_lowercase();
        
        if let Some(start) = text_lower.find(&query_lower) {
            let end = std::cmp::min(start + query.len(), text.len());
            let before = if start > 50 { &text[start-50..start] } else { &text[..start] };
            let match_text = &text[start..end];
            let after = if text.len() - end > 50 { &text[end..end+50] } else { &text[end..] };
            
            Some(format!("...{}**{}**{}...", before, match_text, after))
        } else {
            None
        }
    }
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            content_types: vec![],
            tags: vec![],
            date_range: None,
            s3_prefix: None,
            min_size_bytes: None,
            max_size_bytes: None,
        }
    }
}
```

### Search Agent Usage Examples

```rust
// Basic semantic search
let search_agent = SearchAgent::new();
let results = search_agent.search("machine learning algorithms", Some(5), Some(0.8))?;

println!("Found {} results:", results.len());
for (i, result) in results.iter().enumerate() {
    println!("{}. {} (score: {:.2})", i + 1, result.title, result.similarity_score);
    if let Some(highlight) = &result.highlight {
        println!("   Highlight: {}", highlight);
    }
    println!("   Document ID: {}", result.document_id);
    println!("   Chunk: {}", &result.chunk_text[..100.min(result.chunk_text.len())]);
    println!();
}

// Advanced search with filters
let mut filters = SearchFilters::default();
filters.content_types = vec!["text/plain".to_string(), "text/markdown".to_string()];
filters.tags = vec!["s3".to_string(), "auto-loaded".to_string()];
filters.s3_prefix = Some("documents/".to_string());

let filtered_results = search_agent.search_with_filters(
    "database optimization", 
    filters, 
    Some(10), 
    Some(0.7)
)?;

// Hybrid search combining semantic and keyword matching
let hybrid_results = search_agent.hybrid_search(
    "database performance optimization",
    Some(0.3),    // keyword weight
    Some(0.7),    // semantic weight
    None,         // no filters
    Some(15)      // limit
)?;

println!("Hybrid search results:");
for (i, result) in hybrid_results.iter().enumerate() {
    println!("{}. {} (combined: {:.2})", i + 1, result.title, result.combined_score);
    println!("   Semantic: {:.2} | Keyword: {:.2} | Match: {:?}", 
             result.semantic_score, result.keyword_score, result.match_type);
    if let Some(highlight) = &result.highlight {
        println!("   Highlight: {}", highlight);
    }
    println!();
}

// Hybrid search with custom weighting (more emphasis on keywords)
let keyword_heavy_results = search_agent.hybrid_search(
    "exact phrase matching",
    Some(0.6),    // higher keyword weight
    Some(0.4),    // lower semantic weight
    Some(filters), // with filters
    Some(10)
)?;

// Find similar documents
let similar_docs = search_agent.find_similar_documents(
    some_document_id, 
    Some(3)
)?;
```

### Document Agent Usage Examples

```rust
// Get full document content
let document_agent = DocumentAgent::new();
if let Some(document) = document_agent.get_document(some_document_id)? {
    println!("Document: {}", document.title);
    println!("Content: {}", document.content);
    println!("Source: {}", document.metadata.source);
}

// List documents with filters
let mut filters = DocumentFilters::default();
filters.content_types = vec!["text/plain".to_string()];
filters.tags = vec!["s3".to_string()];

let documents = document_agent.list_documents(Some(filters), Some(20))?;
for doc in documents {
    println!("Title: {} | Created: {}", doc.title, doc.metadata.created_at);
}

// Get document chunks
let chunks = document_agent.get_document_chunks(some_document_id)?;
for (i, chunk) in chunks.iter().enumerate() {
    println!("Chunk {}: {} chars", i, chunk.chunk_text.len());
}

// Check if document exists
if document_agent.document_exists(some_document_id)? {
    println!("Document exists");
} else {
    println!("Document not found");
}
```

### Complete RAG System Workflow

```rust
// Step 1: Process documents
let coordinator = RagCoordinatorAgent::new();
let processing_summary = coordinator.process_s3_documents(Some("documents/"))?;

// Step 2: Search for relevant information
let search_agent = SearchAgent::new();
let search_results = search_agent.search("your query here", Some(5), Some(0.8))?;

// Step 3: Get full document content using DocumentAgent
let document_agent = DocumentAgent::new();
for result in search_results {
    println!("Found relevant content in: {}", result.title);
    println!("Relevance score: {:.2}", result.similarity_score);
    println!("Content snippet: {}", result.chunk_text);
    
    // Get full document if needed
    if let Some(full_doc) = document_agent.get_document(result.document_id)? {
        // Use full document content for context
        println!("Full document length: {} chars", full_doc.content.len());
    }
}
```

### Environment Variables

```bash
# Database Configuration
DB_URL=postgresql://postgres:password@localhost:5432/golem_rag

# AWS S3 Configuration
AWS_ACCESS_KEY_ID=your_access_key_here
AWS_SECRET_ACCESS_KEY=your_secret_key_here
AWS_REGION=us-east-1  # Default: us-east-1
AWS_S3_BUCKET=your-document-bucket

# Optional: Custom S3-compatible endpoint
# S3_ENDPOINT_URL=https://your-minio-server.com

# Embedding Configuration
EMBEDDING_MODEL=mock-embedding-v1

# Agent Configuration
DOCUMENT_LOADER_AGENT_ID=document-loader-agent
EMBEDDING_GENERATOR_AGENT_ID=embedding-generator-agent
RAG_COORDINATOR_AGENT_ID=rag-coordinator-agent
DOCUMENT_AGENT_ID=document-agent
SEARCH_AGENT_ID=search-agent
```

### Benefits of Search Agent

1. **Semantic Search**: Uses vector similarity for meaningful results
2. **Hybrid Search**: Combines semantic and keyword search for better relevance
3. **Flexible Filtering**: Filter by content type, tags, dates, S3 prefix, size
4. **Configurable Weighting**: Adjust semantic vs keyword importance
5. **Similar Documents**: Find documents similar to a given document
6. **Text Highlighting**: Highlights matching portions in results
7. **Match Type Classification**: Identifies semantic-only, keyword-only, or both matches
8. **Performance Optimized**: Uses pgvector and PostgreSQL full-text search

### Benefits of Document Agent

1. **Efficient Document Retrieval**: Optimized for read-heavy operations
2. **Flexible Document Listing**: Filter and paginate document collections
3. **Chunk Access**: Retrieve document chunks for detailed analysis
4. **Metadata-Only Access**: Get document metadata without content
5. **Existence Checking**: Fast document existence verification

### Search Capabilities

- **Vector Similarity**: Uses cosine similarity with pgvector
- **Full-Text Search**: PostgreSQL tsvector and tsrank for keyword matching
- **Hybrid Relevance**: Weighted combination of semantic and keyword scores
- **Metadata Filtering**: Filter by document metadata
- **Text Highlighting**: Highlights matching text in results
- **Similar Document Discovery**: Find similar documents to a reference
- **Match Type Analysis**: Distinguish between semantic and keyword matches

### Document Capabilities

- **Full Document Retrieval**: Get complete document content and metadata
- **Document Listing**: Browse documents with filtering options
- **Chunk Access**: Retrieve individual document chunks
- **Metadata Queries**: Access document metadata without content
- **Existence Verification**: Check if documents exist

### Database Access Strategy

#[agent_definition]
pub trait DocumentAgent {
    fn new() -> Self;
    
    fn add_document(&mut self, doc: Document) -> Result<Uuid>;
    // ... other methods
}

struct DocumentAgentImpl {
    db_url: String,
}

#[agent_implementation]
impl DocumentAgent for DocumentAgentImpl {
    fn new() -> Self {
        let db_url = env::var("DB_URL")
            .expect("DB_URL environment variable must be set");
        
        Self { db_url }
    }
    
    fn add_document(&mut self, doc: Document) -> Result<Uuid> {
        let mut connection = DbConnection::open(&self.db_url)?;
        
        connection.execute(
            "INSERT INTO documents (id, title, content, source, created_at, tags) 
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[&doc.id, &doc.title, &doc.content, &doc.metadata.source, 
               &doc.metadata.created_at, &doc.metadata.tags]
        )?;
        Ok(doc.id)
    }
}
```

### Required Environment Variables

```bash
# Database Configuration - Complete connection string
DB_URL=postgresql://postgres:password@localhost:5432/golem_rag

# Examples for different environments:
# Local: DB_URL=postgres://user:pass@localhost:5432/rag_db
# Remote: DB_URL=postgresql://user:password@db.example.com:5432/production
# Cloud: DB_URL=postgres://user:pass@aws-host.rds.amazonaws.com:5432/cloud_db

# Embedding Configuration
EMBEDDING_MODEL=mock-embedding-v1
# EMBEDDING_API_KEY=your_api_key_here

# Agent Configuration
DOCUMENT_AGENT_ID=document-agent
SEARCH_AGENT_ID=search-agent
RAG_PIPELINE_AGENT_ID=rag-pipeline-agent
```

### Advantages of Direct Connection

1. **Simplicity**: No configuration structures, just URL string
2. **Direct API**: Uses Golem's native `DbConnection::open()`
3. **Minimal Dependencies**: No need for URL parsing or config structs
4. **Standard**: Follows standard database connection patterns
5. **Flexible**: Works with any PostgreSQL-compatible connection string

### Database Schema with pgvector Support

```sql
-- Enable pgvector extension (handled by Golem RDBMS)
CREATE EXTENSION IF NOT EXISTS vector;

-- Documents table
CREATE TABLE documents (
    id UUID PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT NOT NULL,
    content_type TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    tags TEXT[],
    metadata JSONB
);

-- Document chunks table
CREATE TABLE chunks (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    start_pos INTEGER NOT NULL,
    end_pos INTEGER NOT NULL,
    token_count INTEGER,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Embeddings table with vector support
CREATE TABLE embeddings (
    id UUID PRIMARY KEY,
    chunk_id UUID NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    embedding vector(1536) NOT NULL,
    model_name TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Vector index for similarity search
CREATE INDEX idx_embeddings_vector ON embeddings 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
```

### Vector Operations with Golem RDBMS

```rust
use golem_rust::bindings::golem::rdbms::types::Vector;

impl RagAgentImpl {
    fn store_embedding(&mut self, chunk_id: Uuid, embedding: Vec<f32>) -> Result<Uuid> {
        let mut connection = DbConnection::open(&self.db_url)?;
        let vector = Vector::from(embedding); // Convert to Golem vector type
        let embedding_id = Uuid::new_v4();
        
        connection.execute(
            "INSERT INTO embeddings (id, chunk_id, embedding, model_name, created_at) 
             VALUES ($1, $2, $3, $4, $5)",
            &[&embedding_id, &chunk_id, &vector, &self.model_name, &Timestamp::now()]
        )?;
        
        Ok(embedding_id)
    }
    
    fn similarity_search(&self, query_vector: Vec<f32>, limit: u32, threshold: f32) -> Result<Vec<SearchResult>> {
        let mut connection = DbConnection::open(&self.db_url)?;
        let query_vector = Vector::from(query_vector);
        
        let rows = connection.query(
            r#"
            SELECT 
                c.id as chunk_id,
                c.document_id,
                c.content as chunk_content,
                c.chunk_index,
                d.title,
                d.content as document_content,
                d.source,
                d.created_at,
                d.tags,
                1 - (e.embedding <=> $1) as similarity
            FROM embeddings e
            JOIN chunks c ON e.chunk_id = c.id
            JOIN documents d ON c.document_id = d.id
            WHERE 1 - (e.embedding <=> $1) >= $2
            ORDER BY similarity DESC
            LIMIT $3
            "#,
            &[&query_vector, &threshold, &limit]
        )?;
        
        // Process results into SearchResult objects
        self.process_search_results(rows)
    }
}
```

### Configuration and Setup

Since we're using direct `DB_URL` connections, no configuration structures are needed. All configuration is done through environment variables:

```bash
# Database Configuration
DB_URL=postgresql://postgres:password@localhost:5432/golem_rag

# Other Configuration
EMBEDDING_MODEL=mock-embedding-v1
DOCUMENT_AGENT_ID=document-agent
SEARCH_AGENT_ID=search-agent
```

### Migration Management

```rust
impl RagAgentImpl {
    fn initialize_database(&mut self) -> Result<()> {
        let mut connection = DbConnection::open(&self.db_url)?;
        
        // Run migrations
        connection.execute("CREATE EXTENSION IF NOT EXISTS vector", &[])?;
        
        // Create tables if they don't exist
        connection.execute(include_str!("migrations/001_create_documents.sql"), &[])?;
        connection.execute(include_str!("migrations/002_create_chunks.sql"), &[])?;
        connection.execute(include_str!("migrations/003_create_embeddings.sql"), &[])?;
        
        // Create indexes
        connection.execute(include_str!("migrations/004_create_indexes.sql"), &[])?;
        
        Ok(())
    }
}
```

## Current Agent Implementation

The proposal has been updated with four specialized agents that work together:

### ✅ Implemented Agents

1. **DocumentLoaderAgent** - Loads documents from S3 to database
2. **EmbeddingGeneratorAgent** - Generates embeddings for specific documents  
3. **RagCoordinatorAgent** - Orchestrates the complete workflow
4. **DocumentAgent** - Retrieves document content and metadata (Ephemeral)
5. **SearchAgent** - Executes semantic search on stored embeddings (Ephemeral)

### 📋 Agent Summary

| Agent | Purpose | Key Function | Input | Output | Type |
|-------|---------|--------------|-------|--------|------|
| DocumentLoaderAgent | S3 document ingestion | `load_documents_from_s3()` | S3 prefix | Document IDs | Persistent |
| EmbeddingGeneratorAgent | Vector generation | `generate_embeddings_for_document()` | Document ID | Embedding count | Persistent |
| RagCoordinatorAgent | Workflow orchestration | `process_s3_documents()` | S3 prefix | Processing summary | Persistent |
| DocumentAgent | Document retrieval | `get_document()` | Document ID | Document content | Ephemeral |
| SearchAgent | Semantic search | `search()` | Query text | Search results | Ephemeral |

### 🔄 Complete Workflow

```rust
// Simple coordination (recommended)
let coordinator = RagCoordinatorAgent::new();
let summary = coordinator.process_s3_documents(Some("documents/"))?;

// Search for documents
let search_agent = SearchAgent::new();
let results = search_agent.search("your query", Some(5), Some(0.8))?;

// Get document content using DocumentAgent
let document_agent = DocumentAgent::new();
for result in results {
    if let Some(document) = document_agent.get_document(result.document_id)? {
        println!("Full document: {}", document.title);
    }
}
```

### 📖 Detailed Implementation

See the following sections for complete agent implementations:
- [Document Loader Agent](#document-loader-agent)
- [Embedding Generator Agent](#embedding-generator-agent)  
- [RAG Coordinator Agent](#rag-coordinator-agent)
- [Document Agent](#document-agent)
- [Search Agent](#search-agent)

## Deployment Strategy

### Golem-Specific Considerations

1. **State Management**: Leverage Golem's durable execution
2. **Agent Communication**: Use Golem's RPC for inter-agent calls
3. **Resource Limits**: Optimize for WASM constraints
4. **Scalability**: Horizontal scaling with multiple agent instances

### Environment Configuration

```rust
pub struct RagConfig {
    pub database_url: String,
    pub embedding_model: String,
    pub api_key: Option<String>,
    pub chunk_config: ChunkConfig,
    pub search_config: SearchConfig,
}
```

## Embedding Strategy for Golem WASM

### Available Embedding Libraries

#### 1. **WASM-Native Options** (Recommended for Golem)

**ruvector-onnx-embeddings-wasm** `v0.1.2`
- ✅ **WASM-compatible**: Designed specifically for WebAssembly
- ✅ **SIMD optimized**: High performance with SIMD instructions
- ✅ **ONNX support**: Use standard ONNX models
- ✅ **Edge-ready**: Runs in browsers, Cloudflare Workers, Deno
- ✅ **No external dependencies**: Self-contained embedding generation

```rust
use ruvector_onnx_embeddings_wasm::EmbeddingModel;

#[agent_definition]
pub trait VectorSearchAgent {
    fn new() -> Self;
    fn generate_embedding(&self, text: String) -> Result<Vec<f32>>;
}

struct VectorSearchAgentImpl {
    embedding_model: EmbeddingModel,
}

#[agent_implementation]
impl VectorSearchAgent for VectorSearchAgentImpl {
    fn new() -> Self {
        let model = EmbeddingModel::new("all-MiniLM-L6-v2").unwrap();
        Self { embedding_model: model }
    }
    
    fn generate_embedding(&self, text: String) -> Result<Vec<f32>> {
        let embedding = self.embedding_model.embed(&text)?;
        Ok(embedding.to_vec())
    }
}
```

#### 2. **Candle-Based Options** (May Need WASM Testing)

**candle_embed** `v0.1.4`
- ✅ **Fast and configurable**: High performance
- ✅ **Hugging Face models**: Access to any HF model
- ⚠️ **CUDA dependencies**: May need CPU-only features
- ⚠️ **WASM compatibility**: Needs testing for Golem

**embed_anything** `v0.6.7`
- ✅ **Lightning fast**: Optimized for speed
- ✅ **Multiple modalities**: Text, image, audio
- ✅ **ONNX support**: Standard model format
- ⚠️ **Heavy dependencies**: May not be WASM-friendly

#### 3. **HTTP API Options** (Fallback Strategy)

**External APIs via wstd::http**
- ✅ **Always works**: HTTP requests work in Golem
- ✅ **High quality**: OpenAI, Cohere, etc.
- ❌ **External dependency**: Requires API keys and network
- ❌ **Latency**: Network overhead

```rust
use wstd::http;

fn generate_embedding_openai(&self, text: String) -> Result<Vec<f32>> {
    let api_key = env::var("OPENAI_API_KEY")?;
    let request = http::Request::post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(serde_json::json!({
            "input": text,
            "model": "text-embedding-ada-002"
        }).to_string())?;
    
    let response = http::send(request)?;
    let embedding_response: OpenAIEmbeddingResponse = serde_json::from_str(&response.body())?;
    Ok(embedding_response.data[0].embedding.clone())
}
```

### Recommended Embedding Strategy

#### Phase 1: Mock Embeddings (Development)
```rust
fn mock_embedding_generation(&self, text: &str) -> Result<Vec<f32>> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut embedding = vec![0.0f32; 384]; // Standard small embedding size
    
    for (i, word) in words.iter().enumerate() {
        let hash = self.simple_hash(word) as f32;
        let idx = (hash * 384.0) as usize % 384;
        embedding[idx] = hash / 1000.0;
    }
    
    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in embedding.iter_mut() {
            *v /= norm;
        }
    }
    
    Ok(embedding)
}
```

#### Phase 2: WASM-Native Embeddings (Production)
```rust
// Add to Cargo.toml
// ruvector-onnx-embeddings-wasm = "0.1.2"

use ruvector_onnx_embeddings_wasm::EmbeddingModel;

impl VectorSearchAgentImpl {
    fn new() -> Self {
        let model_name = env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string());
        
        let embedding_model = EmbeddingModel::new(&model_name)
            .expect("Failed to load embedding model");
        
        Self { 
            db_config: PostgresConfig::from_env().unwrap(),
            embedding_model,
        }
    }
}
```

#### Phase 3: Hybrid Approach (Advanced)
```rust
enum EmbeddingBackend {
    WASMNative(EmbeddingModel),
    ExternalAPI(String), // API URL
    Mock,
}

impl VectorSearchAgentImpl {
    fn new() -> Self {
        let backend = match env::var("EMBEDDING_BACKEND").as_deref() {
            Ok("wasm") => {
                let model = EmbeddingModel::new(&env::var("EMBEDDING_MODEL").unwrap()).unwrap();
                EmbeddingBackend::WASMNative(model)
            }
            Ok("api") => {
                let api_url = env::var("EMBEDDING_API_URL").unwrap();
                EmbeddingBackend::ExternalAPI(api_url)
            }
            _ => EmbeddingBackend::Mock,
        };
        
        Self { backend }
    }
    
    fn generate_embedding(&self, text: String) -> Result<Vec<f32>> {
        match &self.backend {
            EmbeddingBackend::WASMNative(model) => {
                Ok(model.embed(&text)?.to_vec())
            }
        
        // Search for relevant documents
        let search_query = SearchQuery {
            query: query.clone(),
            query_vector: None,
            filters: SearchFilters::default(),
            limit: context_limit,
            similarity_threshold: 0.7,
        };
        
        let search_results = search_agent.similarity_search(search_query)?;
        
        // Assemble context
        let context = self.assemble_context(search_results)?;
        
        // Generate response (mock for now)
        let response = self.generate_response(&query, &context)?;
        
        Ok(RagResponse {
            query,
            context,
            response,
            sources: search_results.into_iter().map(|r| r.document.id).collect(),
        })
    }
}
```

## Embedding Strategy for Golem WASM

### Available Embedding Libraries

#### 1. **WASM-Native Options** (Recommended for Golem)

**ruvector-onnx-embeddings-wasm** `v0.1.2`
- ✅ **WASM-compatible**: Designed specifically for WebAssembly
- ✅ **SIMD optimized**: High performance with SIMD instructions
- ✅ **ONNX support**: Use standard ONNX models
- ✅ **Edge-ready**: Runs in browsers, Cloudflare Workers, Deno
- ✅ **No external dependencies**: Self-contained embedding generation

```rust
use ruvector_onnx_embeddings_wasm::EmbeddingModel;

#[agent_definition]
pub trait VectorSearchAgent {
    fn new() -> Self;
    fn generate_embedding(&self, text: String) -> Result<Vec<f32>>;
}

struct VectorSearchAgentImpl {
    embedding_model: EmbeddingModel,
}

#[agent_implementation]
impl VectorSearchAgent for VectorSearchAgentImpl {
    fn new() -> Self {
        let model = EmbeddingModel::new("all-MiniLM-L6-v2").unwrap();
        Self { embedding_model: model }
    }
    
    fn generate_embedding(&self, text: String) -> Result<Vec<f32>> {
        let embedding = self.embedding_model.embed(&text)?;
        Ok(embedding.to_vec())
    }
}
```

#### 2. **Candle-Based Options** (May Need WASM Testing)

**candle_embed** `v0.1.4`
- ✅ **Fast and configurable**: High performance
- ✅ **Hugging Face models**: Access to any HF model
- ⚠️ **CUDA dependencies**: May need CPU-only features
- ⚠️ **WASM compatibility**: Needs testing for Golem

**embed_anything** `v0.6.7`
- ✅ **Lightning fast**: Optimized for speed
- ✅ **Multiple modalities**: Text, image, audio
- ✅ **ONNX support**: Standard model format
- ⚠️ **Heavy dependencies**: May not be WASM-friendly

#### 3. **HTTP API Options** (Fallback Strategy)

**External APIs via wstd::http**
- ✅ **Always works**: HTTP requests work in Golem
- ✅ **High quality**: OpenAI, Cohere, etc.
- ❌ **External dependency**: Requires API keys and network
- ❌ **Latency**: Network overhead

```rust
use wstd::http;

fn generate_embedding_openai(&self, text: String) -> Result<Vec<f32>> {
    let api_key = env::var("OPENAI_API_KEY")?;
    let request = http::Request::post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .body(serde_json::json!({
            "input": text,
            "model": "text-embedding-ada-002"
        }).to_string())?;
    
    let response = http::send(request)?;
    let embedding_response: OpenAIEmbeddingResponse = serde_json::from_str(&response.body())?;
    Ok(embedding_response.data[0].embedding.clone())
}
```

### Recommended Embedding Strategy

#### Phase 1: Mock Embeddings (Development)
```rust
fn mock_embedding_generation(&self, text: &str) -> Result<Vec<f32>> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut embedding = vec![0.0f32; 384]; // Standard small embedding size
    
    for (i, word) in words.iter().enumerate() {
        let hash = self.simple_hash(word) as f32;
        let idx = (hash * 384.0) as usize % 384;
        embedding[idx] = hash / 1000.0;
    }
    
    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in embedding.iter_mut() {
            *v /= norm;
        }
    }
    
    Ok(embedding)
}
```

#### Phase 2: WASM-Native Embeddings (Production)
```rust
// Add to Cargo.toml
// ruvector-onnx-embeddings-wasm = "0.1.2"

use ruvector_onnx_embeddings_wasm::EmbeddingModel;

impl VectorSearchAgentImpl {
    fn new() -> Self {
        let model_name = env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string());
        
        let embedding_model = EmbeddingModel::new(&model_name)
            .expect("Failed to load embedding model");
        
        Self { 
            db_config: PostgresConfig::from_env().unwrap(),
            embedding_model,
        }
    }
}
```

#### Phase 3: Hybrid Approach (Advanced)
```rust
enum EmbeddingBackend {
    WASMNative(EmbeddingModel),
    ExternalAPI(String), // API URL
    Mock,
}

impl VectorSearchAgentImpl {
    fn new() -> Self {
        let backend = match env::var("EMBEDDING_BACKEND").as_deref() {
            Ok("wasm") => {
                let model = EmbeddingModel::new(&env::var("EMBEDDING_MODEL").unwrap()).unwrap();
                EmbeddingBackend::WASMNative(model)
            }
            Ok("api") => {
                let api_url = env::var("EMBEDDING_API_URL").unwrap();
                EmbeddingBackend::ExternalAPI(api_url)
            }
            _ => EmbeddingBackend::Mock,
        };
        
        Self { backend }
    }
    
    fn generate_embedding(&self, text: String) -> Result<Vec<f32>> {
        match &self.backend {
            EmbeddingBackend::WASMNative(model) => {
                Ok(model.embed(&text)?.to_vec())
            }
            EmbeddingBackend::ExternalAPI(api_url) => {
                self.call_external_api(api_url, text)
            }
            EmbeddingBackend::Mock => {
                self.mock_embedding_generation(&text)
            }
        }
    }
}
```

### Updated Dependencies

```toml
[workspace.dependencies]
# ... existing dependencies ...

# WASM-native embedding library
ruvector-onnx-embeddings-wasm = "0.1.2"

# Alternative: Candle-based (if WASM compatible)
# candle_embed = "0.1.4"
# candle-core = { version = "0.4", default-features = false }
# candle-nn = { version = "0.4", default-features = false }

# HTTP client for external APIs
wstd = { version = "=0.5.4", features = ["default", "json", "http"] }
```

### Environment Variables for Embeddings

```bash
# Embedding Backend Selection
EMBEDDING_BACKEND=wasm  # Options: wasm, api, mock
EMBEDDING_MODEL=all-MiniLM-L6-v2

# External API Configuration (if needed)
EMBEDDING_API_URL=https://api.openai.com/v1/embeddings
EMBEDDING_API_KEY=your_api_key_here
EMBEDDING_TIMEOUT=30

# Model Configuration
EMBEDDING_BATCH_SIZE=32
EMBEDDING_DIMENSIONS=384  # Model-specific dimensions
```

### Benefits of WASM-Native Embeddings

1. **No External Dependencies**: Works offline
2. **Low Latency**: No network overhead
3. **Privacy**: Data never leaves the system
4. **Cost Effective**: No API costs
5. **Golem-Native**: Designed for WASM environments
6. **Portable**: Works across different WASM runtimes

### Recommended Model Choices

#### For Development (Mock)
- Dimensions: 384
- Speed: Instant
- Quality: Deterministic hash-based

#### For Production (WASM-Native)
- **all-MiniLM-L6-v2**: 384 dimensions, fast, good quality
- **sentence-t5-base**: 768 dimensions, higher quality
- **all-mpnet-base-v2**: 768 dimensions, excellent quality

#### For High Quality (External API)
- **text-embedding-ada-002**: 1536 dimensions, OpenAI
- **embed-english-v3.0**: 1024 dimensions, Cohere

## Search Algorithms

### 1. Semantic Search
- Cosine similarity on vector embeddings
- Configurable similarity thresholds
- Result diversification

### 2. Hybrid Search
- Combine semantic + keyword search
- Weighted scoring algorithm
- Re-ranking based on multiple signals

### 3. Context-Aware Search
- Conversation history integration
- Query expansion and rewriting
- Personalized result ranking

## Text Chunking Strategy

### Configurable Chunking Parameters

```rust
pub struct ChunkConfig {
    pub chunk_size: u32,        // Default: 1000 tokens
    pub chunk_overlap: u32,     // Default: 200 tokens
    pub respect_sentences: bool, // Default: true
    pub min_chunk_size: u32,    // Default: 100 tokens
    pub max_chunk_size: u32,    // Default: 2000 tokens
}
```

### Chunking Algorithms

1. **Fixed-size chunking** with overlap
2. **Semantic chunking** based on sentence boundaries
3. **Recursive character splitting** for code/markdown
4. **Document structure-aware** chunking

## Performance Considerations

### Database Optimization

1. **Vector Indexing**: IVFFlat or HNSW indexes
2. **Batch Operations**: Bulk embedding generation and storage
3. **Connection Pooling**: Efficient database connections
4. **Caching**: Redis for frequent queries and embeddings

### Search Performance

1. **Approximate Nearest Neighbor**: Fast similarity search
2. **Result Caching**: Cache common query results
3. **Pre-filtering**: Metadata-based filtering before vector search
4. **Parallel Processing**: Concurrent embedding generation

## Security and Privacy

### Data Protection

1. **Encryption**: Database encryption at rest
2. **Access Control**: Document-level permissions
3. **Data Isolation**: Multi-tenant data separation
4. **Audit Logging**: Search and access logging

### API Security

1. **Rate Limiting**: Prevent abuse of embedding APIs
2. **Input Validation**: Sanitize all inputs
3. **Error Handling**: Secure error responses
4. **Monitoring**: Performance and security metrics

## Deployment Strategy

### Golem-Specific Considerations

1. **State Management**: Leverage Golem's durable execution
2. **Agent Communication**: Use Golem's RPC for inter-agent calls
3. **Resource Limits**: Optimize for WASM constraints
4. **Scalability**: Horizontal scaling with multiple agent instances

### Environment Configuration

```rust
pub struct RagConfig {
    pub database_url: String,
    pub embedding_model: String,
    pub api_key: Option<String>,
    pub chunk_config: ChunkConfig,
    pub search_config: SearchConfig,
}
```

## Testing Strategy

### Unit Tests
- Embedding generation accuracy
- Chunking algorithm correctness
- Search result relevance
- Database operations

### Integration Tests
- End-to-end RAG pipeline
- Database connectivity and performance
- Agent communication
- Error handling scenarios

### Performance Tests
- Search latency benchmarks
- Memory usage profiling
- Concurrent request handling
- Large document processing

## Updated Development Phases

### Phase 1: Core Infrastructure with Golem RDBMS
- [ ] Set up Golem RDBMS dependencies and configuration
- [ ] Create database schema with pgvector support
- [ ] Implement basic agent definitions with Golem RDBMS
- [ ] Mock embedding generation (hash-based)
- [ ] Simple document indexing and search
- [ ] Basic CRUD operations for documents

### Phase 2: Enhanced Search Capabilities
- [ ] Vector similarity search with pgvector
- [ ] Hybrid search (semantic + keyword)
- [ ] Advanced filtering options
- [ ] Performance optimization with vector indexes
- [ ] Agent-to-agent RPC communication

### Phase 3: Advanced Features
- [ ] External embedding API integration (OpenAI/Cohere)
- [ ] Conversational search with context
- [ ] Multi-modal document support
- [ ] Analytics and search metrics
- [ ] Caching strategies

### Phase 4: Production Readiness
- [ ] Security hardening and access control
- [ ] Performance tuning and monitoring
- [ ] Error handling and recovery
- [ ] Documentation and usage examples
- [ ] Deployment automation

## Key Implementation Details

### Agent Communication Pattern
```rust
// Agents communicate via Golem's RPC system
let doc_agent = DocumentAgentClient::get("document-db-agent");
let search_agent = VectorSearchAgentClient::get("vector-search-agent");

// Cross-agent calls for complete workflows
let doc_id = doc_agent.add_document(document)?;
let chunks = doc_agent.create_chunks(doc_id, 1000, 200)?;
for chunk in chunks {
    let embedding = search_agent.generate_embedding(chunk.content)?;
    search_agent.store_embedding(chunk.id, embedding)?;
}
```

### Error Handling Strategy
```rust
#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub enum RagError {
    DatabaseError(String),
    EmbeddingError(String),
    DocumentNotFound(Uuid),
    InvalidQuery(String),
    ConfigurationError(String),
}

// Result types for proper error propagation
pub type RagResult<T> = Result<T, RagError>;
```

### Configuration Management

Since we're using direct database connections with `DB_URL`, the configuration is simplified to just environment variables:

```rust
// No complex configuration structs needed
// Just use environment variables directly:

pub struct RagConfig {
    pub embedding_model: String,
    pub agent_ids: AgentIds,
}

pub struct AgentIds {
    pub document_agent_id: String,
    pub search_agent_id: String,
    pub rag_pipeline_agent_id: String,
}

impl RagConfig {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(RagConfig {
            embedding_model: env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "mock-embedding-v1".to_string()),
            agent_ids: AgentIds {
                document_agent_id: env::var("DOCUMENT_AGENT_ID")
                    .unwrap_or_else(|_| "document-agent".to_string()),
                search_agent_id: env::var("SEARCH_AGENT_ID")
                    .unwrap_or_else(|_| "search-agent".to_string()),
                rag_pipeline_agent_id: env::var("RAG_PIPELINE_AGENT_ID")
                    .unwrap_or_else(|_| "rag-pipeline-agent".to_string()),
            },
        })
    }
}
```

### Environment Variables Only

```bash
# Database (single URL)
DB_URL=postgresql://postgres:password@localhost:5432/golem_rag

# Embedding
EMBEDDING_MODEL=mock-embedding-v1
EMBEDDING_API_KEY=your_api_key_here  # Optional

# Agent IDs
DOCUMENT_AGENT_ID=document-agent
SEARCH_AGENT_ID=search-agent
RAG_PIPELINE_AGENT_ID=rag-pipeline-agent

# Search Configuration
DEFAULT_CHUNK_SIZE=1000
DEFAULT_CHUNK_OVERLAP=200
DEFAULT_SIMILARITY_THRESHOLD=0.7
DEFAULT_SEARCH_LIMIT=10
```

## Testing Strategy

### Unit Tests
- Database operations with test fixtures
- Embedding generation accuracy tests
- Chunking algorithm edge cases
- Search result ranking validation

### Integration Tests
- End-to-end RAG pipeline workflows
- Multi-agent communication scenarios
- Database connection and transaction handling
- Error propagation and recovery

### Performance Tests
- Vector search latency benchmarks
- Large document ingestion throughput
- Concurrent search request handling
- Memory usage profiling under load

## Success Metrics

### Technical Metrics
- Search latency < 500ms for typical queries
- Document indexing throughput > 50 docs/minute
- Vector search accuracy > 85% relevance
- Database query optimization > 90% cache hit rate

### Operational Metrics
- Agent uptime and durability
- Cross-agent communication success rate
- Error recovery and retry success
- Resource utilization efficiency

## Updated Architecture Benefits

1. **Golem-Native**: Leverages Golem RDBMS for optimal WASM compatibility
2. **Durable**: All operations benefit from Golem's durability guarantees
3. **Scalable**: Agent-based architecture allows horizontal scaling
4. **Maintainable**: Clear separation of concerns between agents
5. **Testable**: Each agent can be tested independently
6. **Extensible**: Easy to add new features and capabilities

---

## Next Steps

The updated proposal now properly uses Golem RDBMS instead of external PostgreSQL libraries. The implementation will proceed with:

1. **Dependencies**: Updated to use `golem-rdbms` and remove `tokio-postgres`
2. **Database Schema**: Compatible with Golem's PostgreSQL implementation
3. **Agent Design**: Full implementation examples with Golem RDBMS integration
4. **Communication**: Agent-to-agent RPC patterns for complete workflows
5. **Configuration**: Proper setup for Golem environment

This approach ensures the RAG system is fully compatible with Golem's WASM constraints while providing enterprise-grade vector search capabilities.

## Success Metrics

### Technical Metrics
- Search latency < 500ms for typical queries
- Indexing throughput > 100 documents/minute
- Vector search accuracy > 85% relevance
- System uptime > 99.9%

### Business Metrics
- User satisfaction with search results
- Document processing efficiency
- Query success rate
- System adoption and usage patterns

## Risks and Mitigations

### Technical Risks
- **WASM Limitations**: Complex ML models may not compile to WASM
  - *Mitigation*: External API integration with fallbacks
- **Database Performance**: Vector search at scale
  - *Mitigation*: Proper indexing and query optimization
- **Memory Constraints**: Large documents and embeddings
  - *Mitigation*: Streaming processing and chunking

### Operational Risks
- **API Dependencies**: External embedding services
  - *Mitigation*: Multiple providers and local fallbacks
- **Data Migration**: Schema changes and data loss
  - *Mitigation*: Incremental migrations and backups
- **Cost Management**: Embedding API costs
  - *Mitigation*: Caching and efficient usage patterns

---

## Next Steps

Upon approval of this proposal, the implementation will proceed with:

1. Setting up the database schema and migrations
2. Implementing the core agent definitions
3. Creating the document indexing pipeline
4. Adding vector search capabilities
5. Testing and optimization
6. Documentation and examples

The implementation will follow Golem best practices and leverage the platform's durability and agent communication features.
