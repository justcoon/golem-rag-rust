use common_lib::*;
use golem_rust::{agent_definition, agent_implementation};
use std::string::String;
use try_match::try_match;

pub type AgentResult<T> = std::result::Result<T, String>;

#[agent_definition(ephemeral)]
pub trait SearchAgent {
    fn new() -> Self;

    /// Search for documents using semantic similarity
    ///
    /// # Arguments
    /// * `query` - Search query text
    /// * `limit` - Maximum number of results to return (default: 10)
    /// * `threshold` - Similarity threshold (0.0 to 1.0, default: 0.7)
    ///
    /// # Returns
    /// List of search results with relevance scores
    async fn search(
        &self,
        query: String,
        limit: Option<usize>,
        threshold: Option<f32>,
    ) -> AgentResult<Vec<SearchResult>>;

    /// Search documents with metadata filters
    async fn search_with_filters(
        &self,
        query: String,
        filters: SearchFilters,
        limit: Option<usize>,
        threshold: Option<f32>,
    ) -> AgentResult<Vec<SearchResult>>;

    /// Get similar documents to a specific document
    async fn find_similar_documents(
        &self,
        document_id: String,
        limit: Option<usize>,
    ) -> AgentResult<Vec<SearchResult>>;
}

struct SearchAgentImpl {
    db_config: PostgresDbConfig,
}

#[agent_implementation]
impl SearchAgent for SearchAgentImpl {
    fn new() -> Self {
        let db_config =
            PostgresDbConfig::from_env().expect("Failed to load PostgresDbConfig from environment");
        Self { db_config }
    }

    async fn search(
        &self,
        query: String,
        limit: Option<usize>,
        threshold: Option<f32>,
    ) -> AgentResult<Vec<SearchResult>> {
        log::info!("Performing semantic search for query: {}", query);

        let db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_config.db_url()) {
            Ok(helper) => helper,
            Err(e) => {
                log::error!("Failed to create database helper: {:?}", e);
                return Err(format!("Failed to create database helper: {:?}", e));
            }
        };

        let limit = limit.unwrap_or(10);
        let threshold = threshold.unwrap_or(0.7);

        let query_embedding = self.generate_query_embedding(&query).await?;
        let results = self.vector_similarity_search(
            &db_helper,
            &query_embedding,
            &query,
            limit,
            threshold,
            None,
        )?;

        Ok(results)
    }

    async fn search_with_filters(
        &self,
        query: String,
        filters: SearchFilters,
        limit: Option<usize>,
        threshold: Option<f32>,
    ) -> AgentResult<Vec<SearchResult>> {
        log::info!("Performing filtered search for query: {}", query);

        let db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_config.db_url()) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

        let limit = limit.unwrap_or(10);
        let threshold = threshold.unwrap_or(0.7);

        let query_embedding = self.generate_query_embedding(&query).await?;
        let filter_conditions = self.build_filter_conditions(&filters);
        let results = self.vector_similarity_search(
            &db_helper,
            &query_embedding,
            &query,
            limit,
            threshold,
            Some(&filter_conditions),
        )?;

        Ok(results)
    }

    async fn find_similar_documents(
        &self,
        document_id: String,
        limit: Option<usize>,
    ) -> AgentResult<Vec<SearchResult>> {
        log::info!("Finding similar documents to: {}", document_id);

        let db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_config.db_url()) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };

        let limit = limit.unwrap_or(10);

        let document_embedding = self
            .get_document_embedding(&db_helper, &document_id)
            .await?;
        let results = self.vector_similarity_search(
            &db_helper,
            &document_embedding,
            "",
            limit + 1,
            0.5,
            None,
        )?;
        let filtered_results: Vec<SearchResult> = results
            .into_iter()
            .filter(|result| result.chunk.document_id != document_id)
            .take(limit)
            .collect();

        Ok(filtered_results)
    }
}

impl SearchAgentImpl {
    fn extract_search_result_from_row(
        &self,
        row: &PostgresDbRow,
        query: &str,
        threshold: f32,
    ) -> AgentResult<Option<SearchResult>> {
        let chunk_id = try_match!(&row.values[0], PostgresDbValue::Text(id) => id.clone())
            .map_err(|_| "Invalid chunk ID type".to_string())?;
        let document_id = try_match!(&row.values[1], PostgresDbValue::Text(id) => id.clone())
            .map_err(|_| "Invalid document ID type".to_string())?;
        let chunk_index = try_match!(&row.values[2], PostgresDbValue::Int4(index) => *index as u32)
            .map_err(|_| "Invalid chunk index type".to_string())?;
        let chunk_text = try_match!(&row.values[3], PostgresDbValue::Text(text) => text.clone())
            .map_err(|_| "Invalid chunk text type".to_string())?;
        let start_pos = try_match!(&row.values[4], PostgresDbValue::Int4(pos) => *pos as u32)
            .map_err(|_| "Invalid start position type".to_string())?;
        let end_pos = try_match!(&row.values[5], PostgresDbValue::Int4(pos) => *pos as u32)
            .map_err(|_| "Invalid end position type".to_string())?;
        let token_count =
            try_match!(&row.values[6], PostgresDbValue::Int4(count) => Some(*count as u32))
                .map_err(|_| "Invalid token count type".to_string())?;
        let similarity_score = match &row.values[7] {
            PostgresDbValue::Float4(score) => *score,
            PostgresDbValue::Float8(score) => *score as f32,
            PostgresDbValue::Text(text) => text.parse::<f32>().unwrap_or(0.0),
            _other => 0.0,
        };

        // Filter by threshold
        if similarity_score >= threshold {
            let document_chunk = DocumentChunk {
                id: chunk_id,
                document_id: document_id.clone(),
                content: chunk_text.clone(),
                chunk_index,
                start_pos,
                end_pos,
                token_count,
            };

            let search_result = SearchResult {
                chunk: document_chunk,
                similarity_score,
                relevance_explanation: self.generate_highlight(&chunk_text, query),
            };

            Ok(Some(search_result))
        } else {
            Ok(None)
        }
    }

    async fn generate_query_embedding(&self, query: &str) -> AgentResult<Vec<f32>> {
        let embedding_client = EmbeddingClient::from_env().map_err(|e| {
            format!(
                "Failed to create embedding client from environment: {:?}",
                e
            )
        })?;

        match embedding_client
            .generate_embedding_with_fallback(query)
            .await
        {
            Ok(embedding) => Ok(embedding),
            Err(e) => {
                log::error!(
                    "Failed to generate query embedding even with fallback: {:?}",
                    e
                );
                Err(format!("Failed to generate query embedding: {:?}", e))
            }
        }
    }

    fn vector_similarity_search(
        &self,
        db_helper: &DatabaseHelper,
        query_embedding: &[f32],
        query: &str,
        limit: usize,
        threshold: f32,
        filter_conditions: Option<&str>,
    ) -> AgentResult<Vec<SearchResult>> {
        let embedding_array: Vec<PostgresLazyDbValue> = query_embedding
            .iter()
            .map(|&v| PostgresLazyDbValue::new(PostgresDbValue::Float4(v)))
            .collect();

        let filters = filter_conditions.unwrap_or("1=1");

        let sql_query = format!(
            r#"
            SELECT 
                dc.id as chunk_id,
                dc.document_id,
                dc.chunk_index,
                dc.content as chunk_text,
                dc.start_pos,
                dc.end_pos,
                dc.token_count,
                MAX(1 - (e.embedding <=> $2::vector)) as similarity_score
            FROM document_chunks dc
            JOIN document_embeddings e ON dc.document_id = e.document_id AND dc.chunk_index = e.chunk_index
            WHERE e.embedding_status LIKE 'completed%'
              AND {}
            GROUP BY dc.id, dc.document_id, dc.chunk_index, dc.content, dc.start_pos, dc.end_pos, dc.token_count
            HAVING MAX(1 - (e.embedding <=> $2::vector)) >= $3
            ORDER BY similarity_score DESC
            LIMIT $1
            "#,
            filters
        );

        let result = db_helper
            .connection
            .query(
                &sql_query,
                vec![
                    PostgresDbValue::Int4(limit as i32),
                    PostgresDbValue::Array(embedding_array),
                    PostgresDbValue::Float4(threshold),
                ],
            )
            .map_err(|e| format!("Failed to execute vector search query: {:?}", e))?;

        let mut search_results = Vec::new();

        for row in result.rows {
            if let Some(result) = self.extract_search_result_from_row(&row, query, threshold)? {
                search_results.push(result);
            }
        }

        Ok(search_results)
    }

    async fn get_document_embedding(
        &self,
        db_helper: &DatabaseHelper,
        document_id: &str,
    ) -> AgentResult<Vec<f32>> {
        let query = r#"
            SELECT e.embedding
            FROM document_embeddings e
            WHERE e.document_id = $1 AND e.embedding_status LIKE 'completed%'
            LIMIT 1
        "#;

        let result = db_helper
            .connection
            .query(query, vec![PostgresDbValue::Text(document_id.to_string())])
            .map_err(|e| format!("Failed to get document embedding: {:?}", e))?;

        if result.rows.is_empty() {
            return Err("Document embedding not found".to_string());
        }

        // Get the embedding vector from the database
        let embedding_array =
            try_match!(&result.rows[0].values[0], PostgresDbValue::Array(array) => array)
                .map_err(|_| "Invalid embedding type".to_string())?;

        // Convert the array values to f32 vector
        let embedding: Vec<f32> = embedding_array
            .iter()
            .map(|lazy_value: &PostgresLazyDbValue| match lazy_value.get() {
                PostgresDbValue::Float4(value) => Ok(value),
                PostgresDbValue::Float8(value) => Ok(value as f32),
                _ => Err("Invalid embedding type: expected Float4 or Float8".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(embedding)
    }

    fn build_filter_conditions(&self, filters: &SearchFilters) -> String {
        let mut conditions = Vec::new();

        if !filters.content_types.is_empty() {
            let placeholders: Vec<String> = filters
                .content_types
                .iter()
                .map(|ct| format!("'{}'", format!("{:?}", ct).trim_matches('"')))
                .collect();
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM documents d WHERE d.id = dc.document_id AND d.metadata->>'content_type' IN ({}))",
                placeholders.join(", ")
            ));
        }

        if !filters.tags.is_empty() {
            for tag in &filters.tags {
                conditions.push(format!("EXISTS (SELECT 1 FROM documents d WHERE d.id = dc.document_id AND d.tags ? '{}')", tag));
            }
        }

        if !filters.sources.is_empty() {
            let source_conditions: Vec<String> = filters
                .sources
                .iter()
                .map(|source| format!("'{}'", source))
                .collect();
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM documents d WHERE d.id = dc.document_id AND d.source IN ({}))",
                source_conditions.join(", ")
            ));
        }

        if let Some(ref date_range) = filters.date_range {
            conditions.push(format!("EXISTS (SELECT 1 FROM documents d WHERE d.id = dc.document_id AND d.created_at BETWEEN '{}'::timestamptz AND '{}'::timestamptz)", date_range.start, date_range.end));
        }

        if conditions.is_empty() {
            "1=1".to_string()
        } else {
            conditions.join(" AND ")
        }
    }

    fn generate_highlight(&self, chunk_text: &str, query: &str) -> Option<String> {
        // Simple highlight generation
        if query.is_empty() {
            return None;
        }

        let query_lower = query.to_lowercase();
        let chunk_lower = chunk_text.to_lowercase();

        if let Some(start_pos) = chunk_lower.find(&query_lower) {
            let end_pos = (start_pos + query.len()).min(chunk_text.len());
            let before = start_pos.saturating_sub(50);
            let after = if end_pos + 50 < chunk_text.len() {
                end_pos + 50
            } else {
                chunk_text.len()
            };

            Some(format!(
                "...{}[{}]{}...",
                &chunk_text[before..start_pos],
                &chunk_text[start_pos..end_pos],
                &chunk_text[end_pos..after]
            ))
        } else {
            None
        }
    }
}
