# RAG Pipeline Implementation Proposal

## Overview

This proposal outlines a comprehensive RAG (Retrieval-Augmented Generation) pipeline implementation for the Golem application using PostgreSQL with pgvector for vector search capabilities.

## System Architecture

### Core Components

1. **Document Management Agent** (`DocumentAgent`)
   - Handles document ingestion, storage, and lifecycle management
   - Supports various document formats (text, markdown, etc.)
   - Manages document metadata and versioning

2. **Vector Search Agent** (`VectorSearchAgent`)
   - Handles embedding generation and storage
   - Manages vector similarity search using pgvector
   - Supports hybrid search (semantic + keyword)

3. **RAG Pipeline Agent** (`RagPipelineAgent`)
   - Orchestrates the complete RAG workflow
   - Combines retrieval with generation capabilities
   - Manages context assembly and prompt engineering

### Data Flow

```
Document Input → Chunking → Embedding Generation → Vector Storage → Search → Context Assembly → Generation
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

### Enhanced DocumentAgent with S3 Support

```rust
#[agent_definition]
pub trait DocumentAgent {
    fn new() -> Self;
    
    // Document lifecycle
    fn add_document(&mut self, doc: Document) -> Result<Uuid>;
    fn index_s3_documents(&mut self, s3_prefix: Option<&str>) -> Result<Vec<IndexingResult>>;
    
    // S3-specific methods
    fn list_s3_documents(&self, prefix: Option<&str>) -> Result<Vec<S3DocumentSource>>;
    fn download_s3_document(&self, bucket: &str, key: &str) -> Result<Vec<u8>>;
}

struct DocumentAgentImpl {
    db_url: String,
    s3_client: S3Client,
    s3_bucket: String,
}

#[agent_implementation]
impl DocumentAgent for DocumentAgentImpl {
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
    
    fn index_s3_documents(&mut self, s3_prefix: Option<&str>) -> Result<Vec<IndexingResult>> {
        let s3_response = self.s3_client.list_objects(&self.s3_bucket, s3_prefix)?;
        let mut results = Vec::new();
        
        for s3_doc in s3_response.objects {
            // Skip non-document files
            if !s3_doc.key.ends_with(".txt") && !s3_doc.key.ends_with(".md") && !s3_doc.key.ends_with(".pdf") {
                continue;
            }
            
            // Download document content
            let content = self.s3_client.get_object(&self.s3_bucket, &s3_doc.key)?;
            let content_str = String::from_utf8(content)?;
            
            // Create document
            let document = Document {
                id: Uuid::new_v4(),
                title: s3_doc.key.split('/').last().unwrap_or(&s3_doc.key).to_string(),
                content: content_str,
                metadata: DocumentMetadata {
                    source: format!("s3://{}/{}", self.s3_bucket, s3_doc.key),
                    content_type: self.infer_content_type(&s3_doc.key),
                    created_at: Timestamp::now(),
                    updated_at: Timestamp::now(),
                    tags: vec!["s3".to_string(), "auto-indexed".to_string()],
                    metadata: serde_json::json!({
                        "s3_bucket": self.s3_bucket,
                        "s3_key": s3_doc.key,
                        "size_bytes": s3_doc.size_bytes,
                        "last_modified": s3_doc.last_modified,
                    }),
                },
            };
            
            // Index the document
            let result = self.index_document(document, ChunkConfig {
                chunk_size: 1000,
                chunk_overlap: 200,
            })?;
            
            results.push(result);
        }
        
        Ok(results)
    }
    
    fn list_s3_documents(&self, prefix: Option<&str>) -> Result<Vec<S3DocumentSource>> {
        let response = self.s3_client.list_objects(&self.s3_bucket, prefix)?;
        Ok(response.objects)
    }
    
    fn download_s3_document(&self, bucket: &str, key: &str) -> Result<Vec<u8>> {
        self.s3_client.get_object(bucket, key)
    }
}
```

### S3 Environment Variables

```bash
# AWS S3 Configuration
AWS_ACCESS_KEY_ID=your_access_key_here
AWS_SECRET_ACCESS_KEY=your_secret_key_here
AWS_REGION=us-east-1
AWS_S3_BUCKET=your-document-bucket

# Optional: Custom S3-compatible endpoint
# S3_ENDPOINT_URL=https://your-minio-server.com
# S3_ENDPOINT_URL=https://your-cloudflare-r2-endpoint.com
# S3_ENDPOINT_URL=https://your-digitalocean-spaces.com

# Database Configuration
DB_URL=postgresql://postgres:password@localhost:5432/golem_rag

# Other Configuration
EMBEDDING_MODEL=mock-embedding-v1
DOCUMENT_AGENT_ID=document-agent
SEARCH_AGENT_ID=search-agent
```

### Benefits of Configurable S3 Endpoints

1. **Multi-Cloud Support**: Works with AWS S3, MinIO, DigitalOcean Spaces, Cloudflare R2, etc.
2. **Private Cloud**: Use on-premises S3-compatible storage
3. **Edge Storage**: Deploy to edge locations with S3-compatible APIs
4. **Cost Optimization**: Choose the most cost-effective S3-compatible provider
5. **Migration Friendly**: Easy to switch between S3 providers without code changes

### Supported S3-Compatible Providers

- **AWS S3**: Default (no endpoint URL needed)
- **MinIO**: `S3_ENDPOINT_URL=https://your-minio-server.com`
- **DigitalOcean Spaces**: `S3_ENDPOINT_URL=https://your-region.digitaloceanspaces.com`
- **Cloudflare R2**: `S3_ENDPOINT_URL=https://your-account.r2.cloudflarestorage.com`
- **Wasabi**: `S3_ENDPOINT_URL=https://s3.wasabisys.com`
- **Backblaze B2**: `S3_ENDPOINT_URL=https://s3.us-west-002.backblazeb2.com`

## Database Access Strategy

### Golem RDBMS Integration (Primary Approach)

The RAG system will use Golem's built-in RDBMS support through Golem-generated database code from WIT definitions, with direct database connection using a URL:

```rust
use golem_rust::bindings::golem::rdbms::postgres::DbConnection;
use golem_rust::bindings::golem::rdbms::types::{Uuid, Timestamp, Vector};
use std::env;

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

## Agent Design

### 1. DocumentAgent

```rust
#[agent_definition]
pub trait DocumentAgent {
    fn new() -> Self;
    
    // Document lifecycle
    fn add_document(&mut self, doc: Document) -> Result<Uuid>;
    fn update_document(&mut self, doc: Document) -> Result<bool>;
    fn delete_document(&mut self, doc_id: Uuid) -> Result<bool>;
    fn get_document(&self, doc_id: Uuid) -> Result<Option<Document>>;
    fn list_documents(&self, filters: Option<DocumentFilters>) -> Result<Vec<Document>>;
    
    // Chunk management
    fn create_chunks(&mut self, doc_id: Uuid, chunk_size: u32, overlap: u32) -> Result<Vec<DocumentChunk>>;
    fn get_document_chunks(&self, doc_id: Uuid) -> Result<Vec<DocumentChunk>>;
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
            "INSERT INTO documents (id, title, content, source, content_type, created_at, updated_at, tags, metadata) 
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            &[&doc.id, &doc.title, &doc.content, &doc.metadata.source, &doc.metadata.content_type,
               &doc.metadata.created_at, &doc.metadata.updated_at, &doc.metadata.tags, &doc.metadata.metadata]
        )?;
        
        Ok(doc.id)
    }
    
    fn create_chunks(&mut self, doc_id: Uuid, chunk_size: u32, overlap: u32) -> Result<Vec<DocumentChunk>> {
        let mut connection = DbConnection::open(&self.db_url)?;
        
        // Get document content
        let row = connection.query_one(
            "SELECT content FROM documents WHERE id = $1",
            &[&doc_id]
        )?;
        let content: String = row.get(0);
        
        // Create chunks
        let chunks = self.chunk_text(&content, chunk_size, overlap);
        let mut created_chunks = Vec::new();
        
        for (index, chunk_content) in chunks.iter().enumerate() {
            let chunk_id = Uuid::new_v4();
            let chunk = DocumentChunk {
                id: chunk_id,
                document_id: doc_id,
                content: chunk_content.clone(),
                chunk_index: index as u32,
                start_pos: 0, // Calculate actual positions
                end_pos: chunk_content.len() as u32,
            };
            
            connection.execute(
                "INSERT INTO chunks (id, document_id, content, chunk_index, start_pos, end_pos, token_count, created_at) 
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[&chunk.id, &chunk.document_id, &chunk.content, &chunk.chunk_index,
                   &chunk.start_pos, &chunk.end_pos, &chunk.token_count, &Timestamp::now()]
            )?;
            
            created_chunks.push(chunk);
        }
        
        Ok(created_chunks)
    }
}
```

### 2. VectorSearchAgent

```rust
#[agent_definition]
pub trait VectorSearchAgent {
    fn new() -> Self;
    
    // Embedding operations
    fn generate_embedding(&self, text: String) -> Result<Vec<f32>>;
    fn store_embedding(&mut self, chunk_id: Uuid, embedding: Vec<f32>) -> Result<Uuid>;
    fn batch_generate_embeddings(&mut self, chunks: Vec<DocumentChunk>) -> Result<Vec<Embedding>>;
    
    // Search operations
    fn similarity_search(&self, query: SearchQuery) -> Result<Vec<SearchResult>>;
    fn hybrid_search(&self, query: SearchQuery, keyword_weight: f32) -> Result<Vec<SearchResult>>;
    fn find_similar_documents(&self, doc_id: Uuid, limit: u32) -> Result<Vec<Document>>;
}

struct VectorSearchAgentImpl {
    db_url: String,
    embedding_model: String,
}

#[agent_implementation]
impl VectorSearchAgent for VectorSearchAgentImpl {
    fn new() -> Self {
        let db_url = env::var("DB_URL")
            .expect("DB_URL environment variable must be set");
        let embedding_model = env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "mock-embedding-v1".to_string());
        
        Self { db_url, embedding_model }
    }
    
    fn store_embedding(&mut self, chunk_id: Uuid, embedding: Vec<f32>) -> Result<Uuid> {
        let mut connection = DbConnection::open(&self.db_url)?;
        let vector = Vector::from(embedding);
        let embedding_id = Uuid::new_v4();
        
        connection.execute(
            "INSERT INTO embeddings (id, chunk_id, embedding, model_name, created_at) 
             VALUES ($1, $2, $3, $4, $5)",
            &[&embedding_id, &chunk_id, &vector, &self.embedding_model, &Timestamp::now()]
        )?;
        
        Ok(embedding_id)
    }
    
    fn similarity_search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        let mut connection = DbConnection::open(&self.db_url)?;
        let query_vector = Vector::from(query.query_vector.unwrap_or_else(|| {
            self.generate_embedding(query.query.clone()).unwrap()
        }));
        
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
            &[&query_vector, &query.similarity_threshold, &query.limit]
        )?;
        
        self.process_search_results(rows)
    }
    
    fn hybrid_search(&self, query: SearchQuery, keyword_weight: f32) -> Result<Vec<SearchResult>> {
        // Get semantic results
        let semantic_results = self.similarity_search(query.clone())?;
        
        // Get keyword results
        let keyword_results = self.keyword_search(&query.query, query.limit)?;
        
        // Combine and re-rank
        self.combine_search_results(semantic_results, keyword_results, keyword_weight)
    }
}
```

### 3. RagPipelineAgent

```rust
#[agent_definition]
pub trait RagPipelineAgent {
    fn new() -> Self;
    
    // Complete RAG workflow
    fn index_document(&mut self, doc: Document, chunk_config: ChunkConfig) -> Result<IndexingResult>;
    fn search_and_generate(&self, query: String, context_limit: u32) -> Result<RagResponse>;
    fn get_relevant_context(&self, query: String, max_docs: u32) -> Result<Vec<DocumentChunk>>;
    
    // Advanced features
    fn conversational_search(&self, query: String, conversation_history: Vec<String>) -> Result<RagResponse>;
    fn multi_query_search(&self, queries: Vec<String>) -> Result<Vec<SearchResult>>;
}

struct RagPipelineAgentImpl {
    document_agent_id: String,
    search_agent_id: String,
}

#[agent_implementation]
impl RagPipelineAgent for RagPipelineAgentImpl {
    fn new() -> Self {
        let document_agent_id = env::var("DOCUMENT_AGENT_ID")
            .unwrap_or_else(|_| "document-agent".to_string());
        let search_agent_id = env::var("SEARCH_AGENT_ID")
            .unwrap_or_else(|_| "search-agent".to_string());
        
        Self { document_agent_id, search_agent_id }
    }
    
    fn index_document(&mut self, doc: Document, chunk_config: ChunkConfig) -> Result<IndexingResult> {
        // Use Golem RPC to call other agents
        let doc_agent = DocumentAgentClient::get(&self.document_agent_id);
        let search_agent = VectorSearchAgentClient::get(&self.search_agent_id);
        
        // Add document
        let doc_id = doc_agent.add_document(doc.clone())?;
        
        // Create chunks
        let chunks = doc_agent.create_chunks(doc_id, chunk_config.chunk_size, chunk_config.chunk_overlap)?;
        
        // Generate and store embeddings
        let mut embeddings_created = 0;
        for chunk in &chunks {
            match search_agent.generate_embedding(chunk.content.clone()) {
                Ok(embedding) => {
                    search_agent.store_embedding(chunk.id, embedding)?;
                    embeddings_created += 1;
                }
                Err(e) => log::warn!("Failed to generate embedding for chunk {}: {}", chunk.id, e),
            }
        }
        
        Ok(IndexingResult {
            document_id: doc_id,
            chunks_created: chunks.len() as u32,
            embeddings_generated: embeddings_created,
        })
    }
    
    fn search_and_generate(&self, query: String, context_limit: u32) -> Result<RagResponse> {
        let search_agent = VectorSearchAgentClient::get(&self.search_agent_id);
        
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
