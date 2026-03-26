use golem_rust::Schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use crate::database::{PostgresDbColumn, PostgresDbRow, PostgresDbValue, decode::DbValueDecoder, decode::DbRowDecoder};


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

crate::db_value_decoder_json!(DocumentMetadata);

crate::db_row_decoder!(Document {
    id,
    title,
    content,
    source,
    namespace,
    tags,
    size_bytes,
    created_at,
    updated_at,
    metadata,
});


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

impl std::str::FromStr for EmbeddingStatus {
    type Err = String;

    fn from_str(status_str: &str) -> Result<Self, Self::Err> {
        if status_str.starts_with("completed:") {
            let chunk_count = status_str
                .split(':')
                .nth(1)
                .and_then(|s: &str| s.parse::<usize>().ok())
                .unwrap_or(0);
            Ok(EmbeddingStatus::Completed { chunk_count })
        } else if status_str.starts_with("failed:") {
            let error = status_str
                .split(':')
                .nth(1)
                .unwrap_or("Unknown error")
                .to_string();
            Ok(EmbeddingStatus::Failed { error })
        } else if status_str == "in_progress" {
            Ok(EmbeddingStatus::InProgress)
        } else {
            Ok(EmbeddingStatus::NotProcessed)
        }
    }
}

impl DbValueDecoder for EmbeddingStatus {
    fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
        let status_str = String::decode(value)?;
        Self::from_str(&status_str).map_err(|e: String| anyhow::anyhow!(e))
    }
}


impl std::fmt::Display for EmbeddingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingStatus::NotProcessed => write!(f, "not_processed"),
            EmbeddingStatus::InProgress => write!(f, "in_progress"),
            EmbeddingStatus::Completed { chunk_count } => write!(f, "completed:{}", chunk_count),
            EmbeddingStatus::Failed { error } => write!(f, "failed:{}", error),
        }
    }
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

crate::db_row_decoder!(DocumentChunk {
    id,
    document_id,
    content,
    chunk_index,
    start_pos,
    end_pos,
    token_count,
});


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

impl DbRowDecoder for SearchResult {
    fn decode_row(row: &PostgresDbRow, columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
        let chunk = DocumentChunk::decode_row(row, columns)?;
        let similarity_score = Self::decode_field(row, Self::find_column_index(columns, "similarity_score")?, "similarity_score")?;
        Ok(SearchResult {
            chunk,
            similarity_score,
            relevance_explanation: None, 
        })
    }
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

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub chunk: DocumentChunk,
    pub semantic_score: f32,
    pub keyword_score: f32,
    pub combined_score: f32,
    pub match_type: MatchType,
    pub relevance_explanation: Option<String>,
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct HybridSearchConfig {
    pub semantic_weight: f32,  // Weight for semantic search (0.0-1.0)
    pub keyword_weight: f32,   // Weight for keyword search (0.0-1.0)
    pub rrf_k: f32,            // Reciprocal Rank Fusion parameter
    pub enable_semantic: bool, // Enable semantic search
    pub enable_keyword: bool,  // Enable keyword search
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            semantic_weight: 0.7,
            keyword_weight: 0.3,
            rrf_k: 60.0,
            enable_semantic: true,
            enable_keyword: true,
        }
    }
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub enum MatchType {
    SemanticOnly, // Only semantic match
    KeywordOnly,  // Only keyword match
    BothMatch,    // Both semantic and keyword match
}
