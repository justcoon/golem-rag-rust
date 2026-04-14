use crate::models::*;
use anyhow::Result;
use crate::common_lib::database::DatabaseHelper;
use crate::encode_params;

pub trait DatabaseHelperRagext {
    fn delete_from_tables(&self, document_id: &str, tables: &[&str]) -> Result<()>;
    fn delete_document(&self, document_id: &str) -> Result<()>;
    fn store_document(&self, document: &Document) -> Result<String>;
    fn load_document(&self, document_id: &str) -> Result<Option<Document>>;
    fn document_exists(&self, document_id: &str) -> Result<bool>;
    fn update_embedding_status(&self, document_id: &str, status: &EmbeddingStatus) -> Result<()>;
    fn get_embedding_status(&self, document_id: &str) -> Result<EmbeddingStatus>;
    fn store_embedding(
        &self,
        embedding: &Embedding,
        document_id: &str,
        chunk_index: i32,
        chunk_text: &str,
    ) -> Result<()>;
    fn store_document_chunk(&self, chunk: &DocumentChunk) -> Result<()>;
}

impl DatabaseHelperRagext for DatabaseHelper {
    fn delete_from_tables(&self, document_id: &str, tables: &[&str]) -> Result<()> {
        self.transactional(|transaction| {
            for table in tables {
                let query = format!("DELETE FROM {} WHERE document_id = $1", table);
                transaction.execute(&query, encode_params![document_id])?;
            }
            Ok(())
        })
    }

    fn delete_document(&self, document_id: &str) -> Result<()> {
        log::info!("Deleting document: {}", document_id);
        self.connection.execute(
            "DELETE FROM documents WHERE id = $1",
            encode_params![document_id],
        )?;
        Ok(())
    }

    fn store_document(&self, document: &Document) -> Result<String> {
        let document_id = document.id.clone();

        self.connection.execute(
            "INSERT INTO documents (id, title, content, metadata, created_at, updated_at, tags, source, namespace, size_bytes) 
             VALUES ($1, $2, $3, $4, $5::timestamptz, $6::timestamptz, $7, $8, $9, $10)",
             encode_params![
                document_id.clone(),
                document.title.clone(),
                document.content.clone(),
                &document.metadata,
                document.created_at.clone(),
                document.updated_at.clone(),
                &document.tags,
                document.source.clone(),
                document.namespace.clone(),
                document.size_bytes as i64,
            ]
        )?;

        Ok(document_id)
    }

    fn load_document(&self, document_id: &str) -> Result<Option<Document>> {
        let query = "SELECT id, title, content, metadata, source, namespace, tags, size_bytes, created_at::text, updated_at::text FROM documents WHERE id = $1";
        let result = self.connection.query(query, encode_params![document_id])?;

        use crate::common_lib::database::decode::DbResultDecoder;
        let documents = Document::decode_result(result)?;
        Ok(documents.into_iter().next())
    }

    fn document_exists(&self, document_id: &str) -> Result<bool> {
        let query = "SELECT COUNT(*) FROM documents WHERE id = $1";
        let result = self.connection.query(query, encode_params![document_id])?;

        use crate::common_lib::database::decode::{DbResultDecoder, Single};
        let counts = Single::<i64>::decode_result(result)?;
        Ok(counts.first().map(|s| s.0 > 0).unwrap_or(false))
    }

    fn update_embedding_status(&self, document_id: &str, status: &EmbeddingStatus) -> Result<()> {
        let status_str = status.to_string();

        self.connection.execute(
            "UPDATE document_embeddings SET embedding_status = $1, updated_at = NOW() WHERE document_id = $2",
            encode_params![status_str, document_id],
        )?;

        Ok(())
    }

    fn get_embedding_status(&self, document_id: &str) -> Result<EmbeddingStatus> {
        let query =
            "SELECT embedding_status FROM document_embeddings WHERE document_id = $1 LIMIT 1";
        let result = self.connection.query(query, encode_params![document_id])?;

        use crate::common_lib::database::decode::{DbResultDecoder, Single};
        let status = Single::<EmbeddingStatus>::decode_result(result)?;
        Ok(status
            .into_iter()
            .next()
            .map(|s| s.0)
            .unwrap_or(EmbeddingStatus::NotProcessed))
    }

    fn store_embedding(
        &self,
        embedding: &Embedding,
        document_id: &str,
        chunk_index: i32,
        chunk_text: &str,
    ) -> Result<()> {
        self.connection.execute(
            "INSERT INTO document_embeddings (id, document_id, chunk_index, chunk_text, embedding, embedding_status, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz)",
            encode_params![
                embedding.id.clone(),
                document_id,
                chunk_index,
                chunk_text,
                &embedding.vector,
                EmbeddingStatus::InProgress.to_string(),
                &embedding.created_at,
                &embedding.created_at,
            ],
        )?;

        Ok(())
    }

    fn store_document_chunk(&self, chunk: &DocumentChunk) -> Result<()> {
        self.connection.execute(
            "INSERT INTO document_chunks (id, document_id, content, chunk_index, start_pos, end_pos, token_count) 
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            encode_params![
                chunk.id.clone(),
                chunk.document_id.clone(),
                chunk.content.clone(),
                chunk.chunk_index as i32,
                chunk.start_pos as i32,
                chunk.end_pos as i32,
                chunk.token_count.unwrap_or(0) as i32,
            ],
        )?;

        Ok(())
    }
}
