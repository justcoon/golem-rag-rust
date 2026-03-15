use serde::{Deserialize, Serialize};
use golem_rust::Schema;
use crate::models::*;
use anyhow::Result;
use try_match::try_match;

// Re-export Golem RDBMS types for convenience
pub use golem_rust::bindings::golem::rdbms::postgres::{
    DbColumnType as PostgresDbColumnType, 
    DbConnection as PostgresDbConnection,
    DbRow as PostgresDbRow, 
    DbValue as PostgresDbValue, 
    LazyDbValue as PostgresLazyDbValue,
};

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

pub struct DatabaseHelper {
    pub connection: PostgresDbConnection,
}

impl DatabaseHelper {
    pub fn new(url: &str) -> Result<Self> {
        let connection = PostgresDbConnection::open(url)?;
        Ok(Self { connection })
    }

    pub fn document_exists_by_s3_key(&mut self, s3_key: &str) -> Result<bool> {
        let query = "SELECT COUNT(*) FROM documents WHERE metadata->'source_metadata'->>'s3_key' = $1";
        let result = self.connection.query(query, vec![PostgresDbValue::Text(s3_key.to_string())])?;
        
        Ok(!result.rows.is_empty() && match &result.rows[0].values[0] {
            PostgresDbValue::Int8(count) => *count > 0,
            _ => false,
        })
    }

    pub fn store_document(&mut self, document: &Document) -> Result<String> {
        let document_id = document.id.clone();
        
        self.connection.execute(
            "INSERT INTO documents (id, title, content, metadata, created_at, updated_at, tags, source, namespace, size_bytes) 
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            vec![
                PostgresDbValue::Text(document_id.clone()),
                PostgresDbValue::Text(document.title.clone()),
                PostgresDbValue::Text(document.content.clone()),
                PostgresDbValue::Jsonb(serde_json::to_string(&document.metadata)?),
                PostgresDbValue::Text(document.metadata.created_at.clone()),
                PostgresDbValue::Text(document.metadata.updated_at.clone()),
                PostgresDbValue::Jsonb(serde_json::to_string(&document.metadata.tags)?),
                PostgresDbValue::Text(document.metadata.source.clone()),
                PostgresDbValue::Text(document.metadata.namespace.clone()),
                PostgresDbValue::Int8(document.metadata.size_bytes as i64),
            ]
        )?;
        
        Ok(document_id)
    }

    pub fn load_document(&mut self, document_id: &str) -> Result<Option<Document>> {
        let query = "SELECT id, title, content, metadata FROM documents WHERE id = $1";
        let result = self.connection.query(query, vec![PostgresDbValue::Text(document_id.to_string())])?;
        
        if result.rows.is_empty() {
            return Ok(None);
        }

        let row = &result.rows[0];
        let id = try_match!(&row.values[0], PostgresDbValue::Text(id) => id.clone()).map_err(|_| anyhow::anyhow!("Invalid document ID type"))?;
        let title = try_match!(&row.values[1], PostgresDbValue::Text(title) => title.clone()).map_err(|_| anyhow::anyhow!("Invalid title type"))?;
        let content = try_match!(&row.values[2], PostgresDbValue::Text(content) => content.clone()).map_err(|_| anyhow::anyhow!("Invalid content type"))?;
        let metadata_str = try_match!(&row.values[3], PostgresDbValue::Jsonb(metadata) => metadata.clone()).map_err(|_| anyhow::anyhow!("Invalid metadata type"))?;
        
        let metadata: DocumentMetadata = serde_json::from_str(&metadata_str)?;
        
        Ok(Some(Document {
            id,
            title,
            content,
            metadata,
        }))
    }

    pub fn document_exists(&mut self, document_id: &str) -> Result<bool> {
        let query = "SELECT COUNT(*) FROM documents WHERE id = $1";
        let result = self.connection.query(query, vec![PostgresDbValue::Text(document_id.to_string())])?;
        
        Ok(!result.rows.is_empty() && match &result.rows[0].values[0] {
            PostgresDbValue::Int8(count) => *count > 0,
            _ => false,
        })
    }

    pub fn update_embedding_status(&mut self, document_id: &str, status: &EmbeddingStatus) -> Result<()> {
        let status_str = match status {
            EmbeddingStatus::NotProcessed => "not_processed".to_string(),
            EmbeddingStatus::InProgress => "in_progress".to_string(),
            EmbeddingStatus::Completed { chunk_count } => format!("completed:{}", chunk_count),
            EmbeddingStatus::Failed { error } => format!("failed:{}", error),
        };

        self.connection.execute(
            "UPDATE document_embeddings SET embedding_status = $1, updated_at = NOW() WHERE document_id = $2",
            vec![
                PostgresDbValue::Text(status_str),
                PostgresDbValue::Text(document_id.to_string()),
            ]
        )?;

        Ok(())
    }

    pub fn get_embedding_status(&mut self, document_id: &str) -> Result<EmbeddingStatus> {
        let query = "SELECT embedding_status, chunk_count FROM document_embeddings WHERE document_id = $1 LIMIT 1";
        let result = self.connection.query(query, vec![PostgresDbValue::Text(document_id.to_string())])?;
        
        if result.rows.is_empty() {
            return Ok(EmbeddingStatus::NotProcessed);
        }

        let row = &result.rows[0];
        let status_str = try_match!(&row.values[0], PostgresDbValue::Text(status) => status.clone()).map_err(|_| anyhow::anyhow!("Invalid status type"))?;

        if status_str.starts_with("completed:") {
            let chunk_count = status_str.split(':').nth(1)
                .and_then(|s: &str| s.parse::<usize>().ok())
                .unwrap_or(0);
            Ok(EmbeddingStatus::Completed { chunk_count })
        } else if status_str.starts_with("failed:") {
            let error = status_str.split(':').nth(1).unwrap_or("Unknown error").to_string();
            Ok(EmbeddingStatus::Failed { error })
        } else if status_str == "in_progress" {
            Ok(EmbeddingStatus::InProgress)
        } else {
            Ok(EmbeddingStatus::NotProcessed)
        }
    }

    pub fn store_embedding(&mut self, embedding: &Embedding) -> Result<()> {
        let vector_params: Vec<PostgresLazyDbValue> = embedding.vector
            .iter()
            .map(|&v| PostgresLazyDbValue::new(PostgresDbValue::Float4(v)))
            .collect();

        self.connection.execute(
            "INSERT INTO document_embeddings (id, chunk_id, embedding, model_name, created_at) 
             VALUES ($1, $2, $3, $4, $5)",
            vec![
                PostgresDbValue::Text(embedding.id.clone()),
                PostgresDbValue::Text(embedding.chunk_id.clone()),
                PostgresDbValue::Array(vector_params),
                PostgresDbValue::Text(embedding.model_name.clone()),
                PostgresDbValue::Text(embedding.created_at.clone()),
            ]
        )?;

        Ok(())
    }

    pub fn store_document_chunk(&mut self, chunk: &DocumentChunk) -> Result<()> {
        self.connection.execute(
            "INSERT INTO document_chunks (id, document_id, content, chunk_index, start_pos, end_pos, token_count) 
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            vec![
                PostgresDbValue::Text(chunk.id.clone()),
                PostgresDbValue::Text(chunk.document_id.clone()),
                PostgresDbValue::Text(chunk.content.clone()),
                PostgresDbValue::Int4(chunk.chunk_index as i32),
                PostgresDbValue::Int4(chunk.start_pos as i32),
                PostgresDbValue::Int4(chunk.end_pos as i32),
                PostgresDbValue::Int4(chunk.token_count.unwrap_or(0) as i32),
            ]
        )?;

        Ok(())
    }
}
