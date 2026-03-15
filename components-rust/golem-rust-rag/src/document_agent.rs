use common_lib::*;
use golem_rust::{agent_definition, agent_implementation};
use std::string::String;
use try_match::try_match;

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

struct DocumentAgentImpl {
    db_url: String,
}

#[agent_implementation]
impl DocumentAgent for DocumentAgentImpl {
    fn new() -> Self {
        let db_url = std::env::var("DB_URL").expect("DB_URL environment variable must be set");

        Self { db_url }
    }

    fn get_document(&self, document_id: String) -> AgentResult<Option<Document>> {
        let mut db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_url) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

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
        let db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_url) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

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
        let db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_url) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

        let query = r#"
            SELECT chunk_id, chunk_index, chunk_text, start_pos, end_pos, token_count
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
            let chunk = self.parse_chunk_from_row(&row)?;
            chunks.push(chunk);
        }

        Ok(chunks)
    }

    fn document_exists(&self, document_id: String) -> AgentResult<bool> {
        let mut db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_url) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

        db_helper
            .document_exists(&document_id)
            .map_err(|e| format!("Failed to check document existence: {:?}", e))
    }
}

impl DocumentAgentImpl {
    fn parse_document_from_row(&self, row: &PostgresDbRow) -> AgentResult<Document> {
        let id = try_match!(&row.values[0], PostgresDbValue::Text(id) => id.clone())
            .map_err(|_| "Invalid document ID type".to_string())?;
        let title = try_match!(&row.values[1], PostgresDbValue::Text(title) => title.clone())
            .map_err(|_| "Invalid title type".to_string())?;
        let content = try_match!(&row.values[2], PostgresDbValue::Text(content) => content.clone())
            .map_err(|_| "Invalid content type".to_string())?;
        let metadata_str =
            try_match!(&row.values[3], PostgresDbValue::Jsonb(metadata) => metadata.clone())
                .map_err(|_| "Invalid metadata type".to_string())?;

        let metadata: DocumentMetadata = serde_json::from_str(&metadata_str)
            .map_err(|e| format!("Failed to parse document metadata: {:?}", e))?;

        Ok(Document {
            id,
            title,
            content,
            metadata,
        })
    }

    fn parse_chunk_from_row(&self, row: &PostgresDbRow) -> AgentResult<DocumentChunk> {
        let chunk_id = try_match!(&row.values[0], PostgresDbValue::Text(id) => id.clone())
            .map_err(|_| "Invalid chunk ID type".to_string())?;
        let document_id =
            try_match!(&row.values[1], PostgresDbValue::Text(document_id) => document_id.clone())
                .map_err(|_| "Invalid document ID type".to_string())?;
        let chunk_index = try_match!(&row.values[2], PostgresDbValue::Int4(index) => *index as u32)
            .map_err(|_| "Invalid chunk index type".to_string())?;
        let chunk_text = try_match!(&row.values[3], PostgresDbValue::Text(text) => text.clone())
            .map_err(|_| "Invalid chunk text type".to_string())?;
        let start_pos = try_match!(&row.values[4], PostgresDbValue::Int4(pos) => *pos as u32)
            .map_err(|_| "Invalid start position type".to_string())?;
        let end_pos = try_match!(&row.values[5], PostgresDbValue::Int4(pos) => *pos as u32)
            .map_err(|_| "Invalid end position type".to_string())?;
        let token_count = match &row.values[6] {
            PostgresDbValue::Int4(count) => Some(*count as u32),
            _ => None,
        };

        Ok(DocumentChunk {
            id: chunk_id,
            document_id,
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
                    query_conditions.push(format!("metadata->'tags' ? ${}", param_index));
                    params.push(PostgresDbValue::Text(tag.clone()));
                    param_index += 1;
                }
            }

            // Add source filters
            if !filters.sources.is_empty() {
                for source in &filters.sources {
                    query_conditions.push(format!("metadata->>'source' = ${}", param_index));
                    params.push(PostgresDbValue::Text(source.clone()));
                    param_index += 1;
                }
            }

            // Add date range filter
            if let Some(date_range) = &filters.date_range {
                query_conditions.push(format!("created_at >= ${}", param_index));
                params.push(PostgresDbValue::Text(date_range.start.clone()));
                param_index += 1;

                query_conditions.push(format!("created_at <= ${}", param_index));
                params.push(PostgresDbValue::Text(date_range.end.clone()));
                // param_index += 1; // Not needed since this is the last parameter
            }
        }

        let where_clause = if query_conditions.len() > 1 {
            format!("WHERE {}", query_conditions[1..].join(" AND "))
        } else {
            String::new()
        };

        let sql_query = format!(
            r#"
            SELECT id, title, metadata, created_at, updated_at
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
