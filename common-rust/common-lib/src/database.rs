use crate::models::*;
use anyhow::Result;
use golem_rust::Schema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use try_match::try_match;

// Re-export Golem RDBMS types for convenience
pub use golem_rust::bindings::golem::rdbms::postgres::{
    DbColumnType as PostgresDbColumnType, DbConnection as PostgresDbConnection,
    DbRow as PostgresDbRow, DbTransaction as PostgresDbTransaction, DbValue as PostgresDbValue,
    LazyDbValue as PostgresLazyDbValue,
};

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct PostgresDbConfig {
    pub host: String,
    pub db: String,
    pub user: String,
    pub password: String,
    pub port: String,
}

impl PostgresDbConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: std::env::var("POSTGRES_HOST")
                .map_err(|_| anyhow::anyhow!("POSTGRES_HOST environment variable not set"))?,
            db: std::env::var("POSTGRES_DB")
                .map_err(|_| anyhow::anyhow!("POSTGRES_DB environment variable not set"))?,
            user: std::env::var("POSTGRES_USER")
                .map_err(|_| anyhow::anyhow!("POSTGRES_USER environment variable not set"))?,
            password: std::env::var("POSTGRES_PASSWORD")
                .map_err(|_| anyhow::anyhow!("POSTGRES_PASSWORD environment variable not set"))?,
            port: std::env::var("POSTGRES_PORT")
                .map_err(|_| anyhow::anyhow!("POSTGRES_PORT environment variable not set"))?,
        })
    }

    pub fn db_url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.db
        )
    }
}

pub struct DatabaseHelper {
    pub connection: PostgresDbConnection,
}

impl DatabaseHelper {
    pub fn new(url: &str) -> Result<Self> {
        let connection = PostgresDbConnection::open(url)?;
        Ok(Self { connection })
    }

    /// Execute a function within a database transaction
    ///
    /// # Arguments
    /// * `f` - A function that takes a transaction reference and returns a Result
    ///
    /// # Returns
    /// The result of the function, with automatic commit/rollback handling
    pub fn transactional<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&PostgresDbTransaction) -> Result<R>,
    {
        let transaction = self.connection.begin_transaction()?;

        match f(&transaction) {
            Ok(result) => {
                transaction.commit()?;
                Ok(result)
            }
            Err(e) => {
                if let Err(rollback_err) = transaction.rollback() {
                    log::error!("Failed to rollback transaction: {:?}", rollback_err);
                }
                Err(e)
            }
        }
    }

    /// Delete from multiple tables in a single transaction
    ///
    /// # Arguments
    /// * `document_id` - The document ID to delete
    /// * `tables` - Array of table names to delete from
    ///
    /// # Returns
    /// Ok(()) if all deletions succeeded, error if any failed
    pub fn delete_from_tables(&self, document_id: &str, tables: &[&str]) -> Result<()> {
        self.transactional(|transaction| {
            for table in tables {
                let query = format!("DELETE FROM {} WHERE document_id = $1", table);
                transaction
                    .execute(&query, vec![PostgresDbValue::Text(document_id.to_string())])?;
            }
            Ok(())
        })
    }

    pub fn store_document(&self, document: &Document) -> Result<String> {
        let document_id = document.id.clone();

        self.connection .execute(
            "INSERT INTO documents (id, title, content, metadata, created_at, updated_at, tags, source, namespace, size_bytes) 
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            vec![
                PostgresDbValue::Text(document_id.clone()),
                PostgresDbValue::Text(document.title.clone()),
                PostgresDbValue::Text(document.content.clone()),
                PostgresDbValue::Jsonb(serde_json::to_string(&document.metadata)?),
                PostgresDbValue::Text(document.created_at.clone()),
                PostgresDbValue::Text(document.updated_at.clone()),
                PostgresDbValue::Array(document.tags.iter().map(|tag| PostgresLazyDbValue::new(PostgresDbValue::Text(tag.clone()))).collect()),
                PostgresDbValue::Text(document.source.clone()),
                PostgresDbValue::Text(document.namespace.clone()),
                PostgresDbValue::Int8(document.size_bytes as i64),
            ]
        )?;

        Ok(document_id)
    }

    pub fn load_document(&self, document_id: &str) -> Result<Option<Document>> {
        let query = "SELECT id, title, content, metadata, source, namespace, tags, size_bytes, created_at::text, updated_at::text FROM documents WHERE id = $1";
        let result = self
            .connection
            .query(query, vec![PostgresDbValue::Text(document_id.to_string())])?;

        if result.rows.is_empty() {
            return Ok(None);
        }

        let row = &result.rows[0];
        let id = try_match!(&row.values[0], PostgresDbValue::Text(id) => id.clone())
            .map_err(|_| anyhow::anyhow!("Invalid document ID type"))?;
        let title = try_match!(&row.values[1], PostgresDbValue::Text(title) => title.clone())
            .map_err(|_| anyhow::anyhow!("Invalid title type"))?;
        let content = try_match!(&row.values[2], PostgresDbValue::Text(content) => content.clone())
            .map_err(|_| anyhow::anyhow!("Invalid content type"))?;
        let metadata_str =
            try_match!(&row.values[3], PostgresDbValue::Jsonb(metadata) => metadata.clone())
                .map_err(|_| anyhow::anyhow!("Invalid metadata type"))?;
        let source = try_match!(&row.values[4], PostgresDbValue::Text(source) => source.clone())
            .map_err(|_| anyhow::anyhow!("Invalid source type"))?;
        let namespace =
            try_match!(&row.values[5], PostgresDbValue::Text(namespace) => namespace.clone())
                .map_err(|_| anyhow::anyhow!("Invalid namespace type"))?;
        let tags_str = try_match!(&row.values[6], PostgresDbValue::Array(tags) => tags)
            .map_err(|_| anyhow::anyhow!("Invalid tags type"))?;
        let size_bytes = try_match!(&row.values[7], PostgresDbValue::Int8(size) => *size)
            .map_err(|_| anyhow::anyhow!("Invalid size_bytes type"))?;
        let created_at =
            try_match!(&row.values[8], PostgresDbValue::Text(created_at) => created_at.clone())
                .map_err(|_| anyhow::anyhow!("Invalid created_at type"))?;
        let updated_at =
            try_match!(&row.values[9], PostgresDbValue::Text(updated_at) => updated_at.clone())
                .map_err(|_| anyhow::anyhow!("Invalid updated_at type"))?;

        let metadata: DocumentMetadata = serde_json::from_str(&metadata_str)?;

        // Parse PostgreSQL array to Vec<String>
        let tags: Vec<String> = tags_str
            .iter()
            .map(|lazy_value| match lazy_value.get() {
                PostgresDbValue::Text(tag) => Ok(tag.clone()),
                _ => Err(anyhow::anyhow!("Invalid tag type: expected Text")),
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(Document {
            id,
            title,
            content,
            source,
            namespace,
            tags,
            size_bytes: size_bytes as u64,
            created_at,
            updated_at,
            metadata,
        }))
    }

    pub fn document_exists(&self, document_id: &str) -> Result<bool> {
        let query = "SELECT COUNT(*) FROM documents WHERE id = $1";
        let result = self
            .connection
            .query(query, vec![PostgresDbValue::Text(document_id.to_string())])?;

        Ok(!result.rows.is_empty()
            && match &result.rows[0].values[0] {
                PostgresDbValue::Int8(count) => *count > 0,
                _ => false,
            })
    }

    pub fn update_embedding_status(
        &self,
        document_id: &str,
        status: &EmbeddingStatus,
    ) -> Result<()> {
        let status_str = status.to_string();

        self.connection .execute(
            "UPDATE document_embeddings SET embedding_status = $1, updated_at = NOW() WHERE document_id = $2",
            vec![
                PostgresDbValue::Text(status_str),
                PostgresDbValue::Text(document_id.to_string()),
            ]
        )?;

        Ok(())
    }

    pub fn get_embedding_status(&self, document_id: &str) -> Result<EmbeddingStatus> {
        let query = "SELECT embedding_status, chunk_count FROM document_embeddings WHERE document_id = $1 LIMIT 1";
        let result = self
            .connection
            .query(query, vec![PostgresDbValue::Text(document_id.to_string())])?;

        if result.rows.is_empty() {
            return Ok(EmbeddingStatus::NotProcessed);
        }

        let row = &result.rows[0];
        let status_str =
            try_match!(&row.values[0], PostgresDbValue::Text(status) => status.clone())
                .map_err(|_| anyhow::anyhow!("Invalid status type"))?;

        EmbeddingStatus::from_str(&status_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse embedding status: {}", e))
    }

    pub fn store_embedding(&self, embedding: &Embedding) -> Result<()> {
        let vector_params: Vec<PostgresLazyDbValue> = embedding
            .vector
            .iter()
            .map(|&v| PostgresLazyDbValue::new(PostgresDbValue::Float4(v)))
            .collect();

        self.connection .execute(
            "INSERT INTO document_embeddings (id, document_id, chunk_index, chunk_text, embedding, embedding_status, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz)",
            vec![
                PostgresDbValue::Text(embedding.id.clone()),
                PostgresDbValue::Text(embedding.chunk_id.clone()), // Using chunk_id as document_id for now
                PostgresDbValue::Int4(0), // chunk_index - will need to be updated
                PostgresDbValue::Text("chunk_text_placeholder".to_string()), // chunk_text placeholder
                PostgresDbValue::Array(vector_params),
                PostgresDbValue::Text(EmbeddingStatus::InProgress.to_string()), // embedding_status - processing individual embedding
                PostgresDbValue::Text(embedding.created_at.clone()),
                PostgresDbValue::Text(embedding.created_at.clone()), // updated_at same as created_at
            ],
        )?;

        Ok(())
    }

    pub fn store_document_chunk(&self, chunk: &DocumentChunk) -> Result<()> {
        self.connection .execute(
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
