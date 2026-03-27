use common_lib::*;
use golem_rust::{agent_definition, agent_implementation};
use std::string::String;

pub type AgentResult<T> = std::result::Result<T, String>;

#[agent_definition(ephemeral)]
pub trait DocumentAgent {
    fn new() -> Self;

    /// Get a specific document by ID
    ///
    /// # Arguments
    /// * `document_id` - String ID of the document to retrieve
    ///
    /// # Returns
    /// Complete Document with metadata, or None if not found
    fn get_document(&self, document_id: String) -> AgentResult<Option<Document>>;

    /// Get document metadata only
    ///
    /// # Arguments
    /// * `document_id` - String ID of the document
    ///
    /// # Returns
    /// DocumentMetadata, or None if not found
    fn get_document_metadata(&self, document_id: String) -> AgentResult<Option<DocumentMetadata>>;

    /// List documents with optional filtering
    ///
    /// # Arguments
    /// * `filters` - Optional filters to apply (content types, tags, date range, namespace, size)
    /// * `limit` - Optional maximum number of documents to return
    ///
    /// # Returns
    /// List of documents matching criteria
    fn list_documents(
        &self,
        filters: Option<DocumentFilters>,
        limit: Option<usize>,
    ) -> AgentResult<Vec<Document>>;

    /// Get document chunks for a specific document
    ///
    /// # Arguments
    /// * `document_id` - String ID of the document
    ///
    /// # Returns
    /// List of document chunks with embeddings
    fn get_document_chunks(&self, document_id: String) -> AgentResult<Vec<DocumentChunk>>;

    /// Check if document exists
    ///
    /// # Arguments
    /// * `document_id` - String ID of the document to check
    ///
    /// # Returns
    /// True if document exists, false otherwise
    fn document_exists(&self, document_id: String) -> AgentResult<bool>;
}

struct DocumentAgentImpl;

#[agent_implementation]
impl DocumentAgent for DocumentAgentImpl {
    fn new() -> Self {
        Self
    }

    fn get_document(&self, document_id: String) -> AgentResult<Option<Document>> {
        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        db_helper
            .load_document(&document_id)
            .map_err(|e| format!("Failed to load document: {:?}", e))
    }

    fn get_document_metadata(&self, document_id: String) -> AgentResult<Option<DocumentMetadata>> {
        self.get_document(document_id)
            .map(|opt| opt.map(|doc| doc.metadata))
    }

    fn list_documents(
        &self,
        filters: Option<DocumentFilters>,
        limit: Option<usize>,
    ) -> AgentResult<Vec<Document>> {
        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        // Build query with filters
        let (sql_query, params) = self.build_document_list_query(filters, limit.unwrap_or(50))?;

        // Execute query
        let result = db_helper
            .connection
            .query(&sql_query, params)
            .map_err(|e| format!("Failed to execute document list query: {:?}", e))?;

        use common_lib::decode::DbResultDecoder;
        Document::decode_result(result).map_err(|e| format!("Failed to decode documents: {:?}", e))
    }

    fn get_document_chunks(&self, document_id: String) -> AgentResult<Vec<DocumentChunk>> {
        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        let query = r#"
            SELECT id, document_id, content, chunk_index, start_pos, end_pos, token_count
            FROM document_chunks 
            WHERE document_id = $1 
            ORDER BY chunk_index
        "#;

        let result = db_helper
            .connection
            .query(query, encode_params![document_id])
            .map_err(|e| format!("Failed to query document chunks: {:?}", e))?;

        use common_lib::decode::DbResultDecoder;
        DocumentChunk::decode_result(result)
            .map_err(|e| format!("Failed to decode document chunks: {:?}", e))
    }

    fn document_exists(&self, document_id: String) -> AgentResult<bool> {
        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        db_helper
            .document_exists(&document_id)
            .map_err(|e| format!("Failed to check document existence: {:?}", e))
    }
}

impl DocumentAgentImpl {
    fn build_document_list_query(
        &self,
        filters: Option<DocumentFilters>,
        limit: usize,
    ) -> AgentResult<(String, Vec<PostgresDbValue>)> {
        let mut query_conditions = vec!["1=1".to_string()];
        let mut params: Vec<PostgresDbValue> = vec![];
        let mut param_index = 1;

        if let Some(filters) = filters {
            // Add content type filters
            if !filters.content_types.is_empty() {
                let placeholders: Vec<String> = filters
                    .content_types
                    .iter()
                    .map(|_| format!("${}", param_index + 1))
                    .collect();
                query_conditions.push(format!(
                    "metadata->>'content_type' IN ({})",
                    placeholders.join(", ")
                ));
                for content_type in &filters.content_types {
                    params.push(format!("{:?}", content_type).encode());
                    param_index += 1;
                }
            }

            // Add tag filters
            if !filters.tags.is_empty() {
                for tag in &filters.tags {
                    query_conditions.push(format!("tags ? ${}", param_index));
                    params.push(tag.encode());
                    param_index += 1;
                }
            }

            // Add source filters
            if !filters.sources.is_empty() {
                let placeholders: Vec<String> = filters
                    .sources
                    .iter()
                    .map(|_| format!("${}", param_index + 1))
                    .collect();
                query_conditions.push(format!("source IN ({})", placeholders.join(", ")));
                for source in &filters.sources {
                    params.push(source.encode());
                    param_index += 1;
                }
            }

            // Add date range filter
            if let Some(date_range) = &filters.date_range {
                query_conditions.push(format!(
                    "created_at BETWEEN ${}::timestamptz AND ${}::timestamptz",
                    param_index,
                    param_index + 1
                ));
                params.push((&date_range.start).encode());
                params.push((&date_range.end).encode());
                // param_index += 2; // Not needed since this is the last parameter
            }
        }

        let where_clause = if query_conditions.len() > 1 {
            format!("WHERE {}", query_conditions[1..].join(" AND "))
        } else {
            String::new()
        };

        let sql_query = format!(
            r#"
            SELECT id, title, content, metadata, source, namespace, tags, size_bytes, created_at::text, updated_at::text
            FROM documents
            {}
            ORDER BY created_at DESC
            LIMIT {}
        "#,
            where_clause, limit
        );

        Ok((sql_query, params))
    }
}
