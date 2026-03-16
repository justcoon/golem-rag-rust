use golem_rust::Schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub source: String,
    pub namespace: String,
    pub tags: Vec<String>,
    pub size_bytes: u64,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: DocumentMetadata,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub content_type: ContentType,
    pub source_metadata: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Markdown,
    Pdf,
    Html,
    Json,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub enum EmbeddingStatus {
    NotProcessed,
    InProgress,
    Completed { chunk_count: usize },
    Failed { error: String },
}

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
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub content: String,
    pub chunk_index: u32,
    pub start_pos: u32,
    pub end_pos: u32,
    pub token_count: Option<u32>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct Embedding {
    pub id: String,
    pub chunk_id: String,
    pub vector: Vec<f32>,
    pub model_name: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk: DocumentChunk,
    pub similarity_score: f32,
    pub relevance_explanation: Option<String>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub query_vector: Option<Vec<f32>>,
    pub filters: SearchFilters,
    pub limit: u32,
    pub similarity_threshold: f32,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize, Default)]
pub struct SearchFilters {
    pub tags: Vec<String>,
    pub sources: Vec<String>,
    pub content_types: Vec<ContentType>,
    pub date_range: Option<DateRange>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct IndexingRequest {
    pub document: Document,
    pub chunk_config: Option<ChunkConfig>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct IndexingResult {
    pub document_id: String,
    pub chunks_created: u32,
    pub embeddings_generated: u32,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct RagResponse {
    pub query: String,
    pub context: Vec<DocumentChunk>,
    pub response: String,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct DocumentFilters {
    pub tags: Vec<String>,
    pub sources: Vec<String>,
    pub content_types: Vec<ContentType>,
    pub date_range: Option<DateRange>,
}

// Use existing SearchResult and SearchFilters from above
pub type HybridSearchResult = SearchResult;

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub enum MatchType {
    SemanticOnly, // Only semantic match
    KeywordOnly,  // Only keyword match
    BothMatch,    // Both semantic and keyword match
}
