use crate::models::*;
use anyhow::Result;
use golem_rust::Schema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

// Re-export Golem RDBMS types for convenience
pub use golem_rust::bindings::golem::rdbms::postgres::{
    DbColumnType as PostgresDbColumnType, DbConnection as PostgresDbConnection,
    DbRow as PostgresDbRow, DbTransaction as PostgresDbTransaction, DbValue as PostgresDbValue,
    LazyDbValue as PostgresLazyDbValue,
};

#[macro_export]
macro_rules! extract_db_field {
    ($row:expr, $idx:expr, $type:pat => $map:expr) => {
        try_match::try_match!(&$row.values[$idx], $type => $map)
            .map_err(|_| anyhow::anyhow!(concat!("Invalid field type at index ", $idx)))?
    };
    ($row:expr, $idx:expr, $type:pat => $map:expr, String) => {
        try_match::try_match!(&$row.values[$idx], $type => $map)
            .map_err(|_| format!("Invalid field type at index {}", $idx))?
    };
}

#[macro_export]
macro_rules! extract_db_array_field {
    ($row:expr, $idx:expr, $inner_type:pat => $inner_map:expr) => {{
        let array = extract_db_field!($row, $idx, $crate::PostgresDbValue::Array(a) => a);
        array.iter()
            .map(|lazy_value| match lazy_value.get() {
                $inner_type => Ok($inner_map),
                _ => Err(anyhow::anyhow!("Invalid array element type")),
            })
            .collect::<Result<Vec<_>, _>>()?
    }};
    ($row:expr, $idx:expr, $inner_type:pat => $inner_map:expr, String) => {{
        let array = extract_db_field!($row, $idx, $crate::PostgresDbValue::Array(a) => a, String);
        array.iter()
            .map(|lazy_value| match lazy_value.get() {
                $inner_type => Ok($inner_map),
                _ => Err("Invalid array element type".to_string()),
            })
            .collect::<std::result::Result<Vec<_>, _>>()?
    }};
}

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

    pub fn from_env() -> Result<Self> {
        let config = PostgresDbConfig::from_env()?;
        Self::new(&config.db_url())
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

    /// Delete a document and all associated data (chunks, embeddings)
    ///
    /// # Arguments
    /// * `document_id` - The document ID to delete
    pub fn delete_document(&self, document_id: &str) -> Result<()> {
        log::info!("Deleting document: {}", document_id);
        self.connection.execute(
            "DELETE FROM documents WHERE id = $1",
            vec![PostgresDbValue::Text(document_id.to_string())],
        )?;
        Ok(())
    }

    pub fn store_document(&self, document: &Document) -> Result<String> {
        let document_id = document.id.clone();

        self.connection.execute(
            "INSERT INTO documents (id, title, content, metadata, created_at, updated_at, tags, source, namespace, size_bytes) 
             VALUES ($1, $2, $3, $4, $5::timestamptz, $6::timestamptz, $7, $8, $9, $10)",
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

        // Use macro for simple fields
        let id = extract_db_field!(row, 0, PostgresDbValue::Text(id) => id.clone());
        let title = extract_db_field!(row, 1, PostgresDbValue::Text(title) => title.clone());
        let content = extract_db_field!(row, 2, PostgresDbValue::Text(content) => content.clone());
        let metadata_str = extract_db_field!(row, 3, PostgresDbValue::Jsonb(m) => m.clone());
        let source = extract_db_field!(row, 4, PostgresDbValue::Text(s) => s.clone());
        let namespace = extract_db_field!(row, 5, PostgresDbValue::Text(n) => n.clone());
        let tags = extract_db_array_field!(row, 6, PostgresDbValue::Text(t) => t.clone());
        let size_bytes = extract_db_field!(row, 7, PostgresDbValue::Int8(s) => *s);
        let created_at = extract_db_field!(row, 8, PostgresDbValue::Text(c) => c.clone());
        let updated_at = extract_db_field!(row, 9, PostgresDbValue::Text(u) => u.clone());

        let metadata: DocumentMetadata = serde_json::from_str(&metadata_str)?;

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

        if result.rows.is_empty() {
            return Ok(false);
        }

        let count = extract_db_field!(result.rows[0], 0, PostgresDbValue::Int8(count) => *count);
        Ok(count > 0)
    }

    pub fn update_embedding_status(
        &self,
        document_id: &str,
        status: &EmbeddingStatus,
    ) -> Result<()> {
        let status_str = status.to_string();

        self.connection.execute(
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
        let status_str = extract_db_field!(row, 0, PostgresDbValue::Text(status) => status.clone());

        EmbeddingStatus::from_str(&status_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse embedding status: {}", e))
    }

    pub fn store_embedding(
        &self,
        embedding: &Embedding,
        document_id: &str,
        chunk_index: i32,
        chunk_text: &str,
    ) -> Result<()> {
        let vector_params: Vec<PostgresLazyDbValue> = embedding
            .vector
            .iter()
            .map(|&v| PostgresLazyDbValue::new(PostgresDbValue::Float4(v)))
            .collect();

        self.connection.execute(
            "INSERT INTO document_embeddings (id, document_id, chunk_index, chunk_text, embedding, embedding_status, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz)",
            vec![
                PostgresDbValue::Text(embedding.id.clone()),
                PostgresDbValue::Text(document_id.to_string()),
                PostgresDbValue::Int4(chunk_index),
                PostgresDbValue::Text(chunk_text.to_string()),
                PostgresDbValue::Array(vector_params),
                PostgresDbValue::Text(EmbeddingStatus::InProgress.to_string()),
                PostgresDbValue::Text(embedding.created_at.clone()),
                PostgresDbValue::Text(embedding.created_at.clone()),
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
