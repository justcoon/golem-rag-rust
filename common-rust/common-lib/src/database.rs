use crate::models::*;
use anyhow::Result;
use golem_rust::Schema;
use serde::{Deserialize, Serialize};

// Re-export Golem RDBMS types for convenience
pub use golem_rust::bindings::golem::rdbms::postgres::{
    DbColumn as PostgresDbColumn, DbColumnType as PostgresDbColumnType,
    DbConnection as PostgresDbConnection, DbResult as PostgresDbResult, DbRow as PostgresDbRow,
    DbTransaction as PostgresDbTransaction, DbValue as PostgresDbValue,
    LazyDbValue as PostgresLazyDbValue,
};

pub mod decode {
    use super::*;

    pub trait DbValueDecoder: Sized {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self>;
    }

    /// A wrapper for types that should be decoded from JSON/JSONB
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Json<T>(pub T);

    impl<T: serde::de::DeserializeOwned> DbValueDecoder for Json<T> {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Jsonb(s) | PostgresDbValue::Json(s) => serde_json::from_str(s)
                    .map(Json)
                    .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e)),
                _ => Err(anyhow::anyhow!("Expected Jsonb or Json, got {:?}", value)),
            }
        }
    }

    /// A wrapper for single-column results
    #[derive(Debug, Clone)]
    pub struct Single<T>(pub T);

    impl<T: DbValueDecoder> DbRowDecoder for Single<T> {
        fn decode_row(row: &PostgresDbRow, _columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
            let value = row
                .values
                .first()
                .ok_or_else(|| anyhow::anyhow!("Row is empty"))?;
            T::decode(value).map(Single)
        }
    }

    // Tuple implementations
    impl<T1: DbValueDecoder, T2: DbValueDecoder> DbRowDecoder for (T1, T2) {
        fn decode_row(row: &PostgresDbRow, _columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
            let v1 = T1::decode(
                row.values
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("Missing column 0"))?,
            )?;
            let v2 = T2::decode(
                row.values
                    .get(1)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 1"))?,
            )?;
            Ok((v1, v2))
        }
    }

    impl<T1: DbValueDecoder, T2: DbValueDecoder, T3: DbValueDecoder> DbRowDecoder for (T1, T2, T3) {
        fn decode_row(row: &PostgresDbRow, _columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
            let v1 = T1::decode(
                row.values
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("Missing column 0"))?,
            )?;
            let v2 = T2::decode(
                row.values
                    .get(1)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 1"))?,
            )?;
            let v3 = T3::decode(
                row.values
                    .get(2)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 2"))?,
            )?;
            Ok((v1, v2, v3))
        }
    }

    impl<T1: DbValueDecoder, T2: DbValueDecoder, T3: DbValueDecoder, T4: DbValueDecoder> DbRowDecoder
        for (T1, T2, T3, T4)
    {
        fn decode_row(row: &PostgresDbRow, _columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
            let v1 = T1::decode(
                row.values
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("Missing column 0"))?,
            )?;
            let v2 = T2::decode(
                row.values
                    .get(1)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 1"))?,
            )?;
            let v3 = T3::decode(
                row.values
                    .get(2)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 2"))?,
            )?;
            let v4 = T4::decode(
                row.values
                    .get(3)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 3"))?,
            )?;
            Ok((v1, v2, v3, v4))
        }
    }

    #[macro_export]
    macro_rules! db_value_decoder_json {
        ($t:ty) => {
            impl $crate::database::decode::DbValueDecoder for $t {
                fn decode(
                    value: &$crate::database::PostgresDbValue,
                ) -> anyhow::Result<Self> {
                    match value {
                        $crate::database::PostgresDbValue::Jsonb(s)
                        | $crate::database::PostgresDbValue::Json(s) => serde_json::from_str(s)
                            .map_err(|e| {
                                anyhow::anyhow!(
                                    "Failed to parse JSON for {}: {}",
                                    stringify!($t),
                                    e
                                )
                            }),
                        _ => Err(anyhow::anyhow!(
                            "Expected Jsonb or Json for {}, got {:?}",
                            stringify!($t),
                            value
                        )),
                    }
                }
            }
        };
    }

    pub trait DbRowDecoder: Sized {
        fn decode_row(row: &PostgresDbRow, columns: &[PostgresDbColumn]) -> anyhow::Result<Self>;

        fn find_column_index(columns: &[PostgresDbColumn], name: &str) -> anyhow::Result<usize> {
            columns
                .iter()
                .position(|c| c.name == name)
                .ok_or_else(|| anyhow::anyhow!("Column {} not found", name))
        }

        fn decode_field<T: DbValueDecoder>(
            row: &PostgresDbRow,
            idx: usize,
            field_name: &str,
        ) -> anyhow::Result<T> {
            let value = row
                .values
                .get(idx)
                .ok_or_else(|| anyhow::anyhow!("Field index {} out of bounds for row", idx))?;
            DbValueDecoder::decode(value)
                .map_err(|e| anyhow::anyhow!("Error decoding field '{}': {}", field_name, e))
        }
    }

    pub trait DbResultDecoder: Sized {
        fn decode_result(result: PostgresDbResult) -> anyhow::Result<Vec<Self>>;
    }

    impl<T: DbRowDecoder> DbResultDecoder for T {
        fn decode_result(result: PostgresDbResult) -> anyhow::Result<Vec<Self>> {
            result
                .rows
                .iter()
                .map(|row| T::decode_row(row, &result.columns))
                .collect()
        }
    }

    #[macro_export]
    macro_rules! db_row_decoder {
        ($struct_name:ident { $($field:ident),* $(,)? }) => {
            impl $crate::database::decode::DbRowDecoder for $struct_name {
                fn decode_row(
                    row: &$crate::database::PostgresDbRow,
                    columns: &[$crate::database::PostgresDbColumn],
                ) -> anyhow::Result<Self> {
                    let find_idx = |name: &str| {
                        <Self as $crate::database::decode::DbRowDecoder>::find_column_index(columns, name)
                    };

                    Ok(Self {
                        $(
                            $field: {
                                let idx = find_idx(stringify!($field))?;
                                <Self as $crate::database::decode::DbRowDecoder>::decode_field(row, idx, stringify!($field))?
                            },
                        )*
                    })
                }
            }
        };
    }

    // Implementations for common types
    impl DbValueDecoder for String {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Text(s) => Ok(s.clone()),
                PostgresDbValue::Varchar(s) => Ok(s.clone()),
                _ => Err(anyhow::anyhow!("Expected Text or Varchar, got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for bool {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Boolean(b) => Ok(*b),
                _ => Err(anyhow::anyhow!("Expected Boolean, got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for i64 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Int8(i) => Ok(*i),
                PostgresDbValue::Int4(i) => Ok(*i as i64),
                PostgresDbValue::Int2(i) => Ok(*i as i64),
                _ => Err(anyhow::anyhow!("Expected Int8, Int4 or Int2 (for i64), got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for i32 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Int4(i) => Ok(*i),
                PostgresDbValue::Int2(i) => Ok(*i as i32),
                _ => Err(anyhow::anyhow!("Expected Int4 or Int2 (for i32), got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for u64 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Int8(i) => Ok(*i as u64),
                PostgresDbValue::Int4(i) => Ok(*i as u64),
                _ => Err(anyhow::anyhow!("Expected Int8 or Int4 (for u64), got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for f32 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Float4(f) => Ok(*f),
                PostgresDbValue::Float8(f) => Ok(*f as f32),
                _ => Err(anyhow::anyhow!("Expected Float4 or Float8 (for f32), got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for f64 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Float8(f) => Ok(*f),
                PostgresDbValue::Float4(f) => Ok(*f as f64),
                _ => Err(anyhow::anyhow!("Expected Float8 or Float4 (for f64), got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for i16 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Int2(i) => Ok(*i),
                _ => Err(anyhow::anyhow!("Expected Int2, got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for u32 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Oid(o) => Ok(*o),
                PostgresDbValue::Int4(i) => Ok(*i as u32),
                _ => Err(anyhow::anyhow!("Expected Oid or Int4 (for u32), got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for Vec<u8> {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Bytea(b) => Ok(b.clone()),
                _ => Err(anyhow::anyhow!("Expected Bytea, got {:?}", value)),
            }
        }
    }

    impl<T: DbValueDecoder> DbValueDecoder for Option<T> {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Null => Ok(None),
                _ => T::decode(value).map(Some),
            }
        }
    }

    impl<T: DbValueDecoder> DbValueDecoder for Vec<T> {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Array(vals) => vals
                    .iter()
                    .map(|lazy| T::decode(&lazy.get()))
                    .collect::<anyhow::Result<Vec<_>>>(),
                _ => Err(anyhow::anyhow!("Expected Array, got {:?}", value)),
            }
        }
    }
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

        use crate::database::decode::DbResultDecoder;
        let documents = Document::decode_result(result)?;
        Ok(documents.into_iter().next())
    }

    pub fn document_exists(&self, document_id: &str) -> Result<bool> {
        let query = "SELECT COUNT(*) FROM documents WHERE id = $1";
        let result = self
            .connection
            .query(query, vec![PostgresDbValue::Text(document_id.to_string())])?;

        use crate::database::decode::{DbResultDecoder, Single};
        let counts = Single::<i64>::decode_result(result)?;
        Ok(counts.first().map(|s| s.0 > 0).unwrap_or(false))
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
        let query =
            "SELECT embedding_status FROM document_embeddings WHERE document_id = $1 LIMIT 1";
        let result = self
            .connection
            .query(query, vec![PostgresDbValue::Text(document_id.to_string())])?;

        use crate::database::decode::{DbResultDecoder, Single};
        let status = Single::<EmbeddingStatus>::decode_result(result)?;
        Ok(status
            .into_iter()
            .next()
            .map(|s| s.0)
            .unwrap_or(EmbeddingStatus::NotProcessed))
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
