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
        match self.get_document(document_id) {
            Ok(Some(document)) => Ok(Some(document.metadata)),
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Failed to get document metadata: {:?}", e)),
        }
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

        // Process results
        let mut documents = Vec::new();
        for row in result.rows {
            let document = self.parse_document_from_row(&row)?;
            documents.push(document);
        }

        Ok(documents)
    }

    fn get_document_chunks(&self, document_id: String) -> AgentResult<Vec<DocumentChunk>> {
        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        let query = r#"
            SELECT id, chunk_index, content, start_pos, end_pos, token_count
            FROM document_chunks 
            WHERE document_id = $1 
            ORDER BY chunk_index
        "#;

        let result = db_helper
            .connection
            .query(query, vec![PostgresDbValue::Text(document_id.to_string())])
            .map_err(|e| format!("Failed to query document chunks: {:?}", e))?;

        let mut chunks = Vec::new();
        for row in result.rows {
            let chunk = self.parse_chunk_from_row(&row, &document_id)?;
            chunks.push(chunk);
        }

        Ok(chunks)
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
    fn parse_document_from_row(&self, row: &PostgresDbRow) -> AgentResult<Document> {
        let id = extract_db_field!(row, 0, PostgresDbValue::Text(id) => id.clone(), String);
        let title =
            extract_db_field!(row, 1, PostgresDbValue::Text(title) => title.clone(), String);
        let content =
            extract_db_field!(row, 2, PostgresDbValue::Text(content) => content.clone(), String);
        let metadata_str =
            extract_db_field!(row, 3, PostgresDbValue::Jsonb(m) => m.clone(), String);
        let source = extract_db_field!(row, 4, PostgresDbValue::Text(s) => s.clone(), String);
        let namespace = extract_db_field!(row, 5, PostgresDbValue::Text(n) => n.clone(), String);
        let tags: Vec<String> =
            extract_db_array_field!(row, 6, PostgresDbValue::Text(t) => t.clone(), String);
        let size_bytes = extract_db_field!(row, 7, PostgresDbValue::Int8(s) => *s, String);
        let created_at = extract_db_field!(row, 8, PostgresDbValue::Text(c) => c.clone(), String);
        let updated_at = extract_db_field!(row, 9, PostgresDbValue::Text(u) => u.clone(), String);

        let metadata: DocumentMetadata = serde_json::from_str(&metadata_str)
            .map_err(|e| format!("Failed to parse document metadata: {:?}", e))?;

        Ok(Document {
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
        })
    }

    fn parse_chunk_from_row(
        &self,
        row: &PostgresDbRow,
        document_id: &str,
    ) -> AgentResult<DocumentChunk> {
        let chunk_id = extract_db_field!(row, 0, PostgresDbValue::Text(id) => id.clone(), String);
        let chunk_index =
            extract_db_field!(row, 1, PostgresDbValue::Int4(index) => *index as u32, String);
        let chunk_text =
            extract_db_field!(row, 2, PostgresDbValue::Text(text) => text.clone(), String);
        let start_pos =
            extract_db_field!(row, 3, PostgresDbValue::Int4(pos) => *pos as u32, String);
        let end_pos = extract_db_field!(row, 4, PostgresDbValue::Int4(pos) => *pos as u32, String);
        let token_count = match &row.values[5] {
            PostgresDbValue::Int4(count) => Some(*count as u32),
            _ => None,
        };

        Ok(DocumentChunk {
            id: chunk_id,
            document_id: document_id.to_string(),
            content: chunk_text,
            chunk_index,
            start_pos,
            end_pos,
            token_count,
        })
    }

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
                    params.push(PostgresDbValue::Text(format!("{:?}", content_type)));
                    param_index += 1;
                }
            }

            // Add tag filters
            if !filters.tags.is_empty() {
                for tag in &filters.tags {
                    query_conditions.push(format!("tags ? ${}", param_index));
                    params.push(PostgresDbValue::Text(tag.clone()));
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
                    params.push(PostgresDbValue::Text(source.clone()));
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
                params.push(PostgresDbValue::Text(date_range.start.clone()));
                params.push(PostgresDbValue::Text(date_range.end.clone()));
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
