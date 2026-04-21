use crate::common_lib::database::DatabaseHelper;
use crate::common_lib::embedding_client::EmbeddingClient;
use crate::encode_params;
use crate::models::*;
use golem_rust::{agent_definition, agent_implementation, endpoint};
use std::string::String;

pub type AgentResult<T> = std::result::Result<T, ErrorResponse>;

#[agent_definition(mount = "/search", ephemeral)]
pub trait SearchAgent {
    fn new() -> Self;

    /// Get similar documents to a specific document
    #[endpoint(post = "/similar")]
    async fn find_similar_documents(
        &self,
        document_id: String,
        limit: Option<u64>,
    ) -> AgentResult<Vec<SearchResult>>;

    /// Search for documents using semantic and/or keyword search
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `filters` - Optional search filters
    /// * `limit` - Optional limit on number of results
    /// * `threshold` - Optional similarity threshold
    /// * `config` - Optional hybrid search configuration
    ///
    /// # Returns
    /// List of search results with combined relevance scores
    #[endpoint(post = "/")]
    async fn search(
        &self,
        query: String,
        filters: Option<SearchFilters>,
        limit: Option<u64>,
        threshold: Option<f32>,
        config: Option<HybridSearchConfig>,
    ) -> AgentResult<Vec<HybridSearchResult>>;
}

struct SearchAgentImpl;

#[agent_implementation]
impl SearchAgent for SearchAgentImpl {
    fn new() -> Self {
        Self
    }

    async fn find_similar_documents(
        &self,
        document_id: String,
        limit: Option<u64>,
    ) -> AgentResult<Vec<SearchResult>> {
        log::info!("Finding similar documents to: {}", document_id);

        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        let limit = limit.map(|l| l as usize).unwrap_or(10);

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

    async fn search(
        &self,
        query: String,
        filters: Option<SearchFilters>,
        limit: Option<u64>,
        threshold: Option<f32>,
        config: Option<HybridSearchConfig>,
    ) -> AgentResult<Vec<HybridSearchResult>> {
        log::info!("Performing search for query: {}", query);

        let db_helper = DatabaseHelper::from_env()
            .map_err(|e| format!("Failed to create database helper: {:?}", e))?;

        let limit = limit.map(|l| l as usize).unwrap_or(10);
        let threshold = threshold.unwrap_or(0.7);
        let config = config.unwrap_or_default();

        let mut semantic_results = Vec::new();
        let mut keyword_results = Vec::new();

        // Prepare filter conditions if provided
        let filter_conditions = filters.as_ref().map(|f| self.build_filter_conditions(f));
        let filter_conditions_ref = filter_conditions.as_deref();

        // Perform semantic search if enabled
        if config.enable_semantic {
            let query_embedding = self.generate_query_embedding(&query).await?;
            semantic_results = self.vector_similarity_search(
                &db_helper,
                &query_embedding,
                &query,
                limit,
                threshold,
                filter_conditions_ref,
            )?;
        }

        // Perform keyword search if enabled
        if config.enable_keyword {
            keyword_results = self.keyword_search(&db_helper, &query, limit, filters.as_ref())?;
        }

        // Fuse results using Reciprocal Rank Fusion (RRF)
        let fused_results = self.fuse_results(semantic_results, keyword_results, &config)?;

        Ok(fused_results)
    }
}

impl SearchAgentImpl {
    async fn generate_query_embedding(&self, query: &str) -> AgentResult<Vec<f32>> {
        let embedding_client = EmbeddingClient::from_env().map_err(|e| {
            ErrorResponse::from(format!(
                "Failed to create embedding client from environment: {:?}",
                e
            ))
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
                Err(format!("Failed to generate query embedding: {:?}", e).into())
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
        let filters = filter_conditions.unwrap_or("1=1");

        let sql_query = format!(
            r#"
            SELECT 
                dc.id,
                dc.document_id,
                dc.chunk_index,
                dc.content,
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
                encode_params![limit as i32, query_embedding, threshold,],
            )
            .map_err(|e| format!("Failed to execute vector search query: {:?}", e))?;

        use crate::common_lib::database::decode::DbResultDecoder;
        let mut search_results = SearchResult::decode_result(result)
            .map_err(|e| format!("Failed to decode search results: {:?}", e))?;

        // Add highlights
        for result in &mut search_results {
            result.relevance_explanation = self.generate_highlight(&result.chunk.content, query);
        }

        Ok(search_results)
    }

    async fn get_document_embedding(
        &self,
        db_helper: &DatabaseHelper,
        document_id: &str,
    ) -> AgentResult<Vec<f32>> {
        let query = r#"
            SELECT e.embedding::float8[]
            FROM document_embeddings e
            WHERE e.document_id = $1 AND e.embedding_status LIKE 'completed%'
            LIMIT 1
        "#;

        let result = db_helper
            .connection
            .query(query, encode_params![document_id])
            .map_err(|e| {
                ErrorResponse::from(format!("Failed to get document embedding: {:?}", e))
            })?;

        use crate::common_lib::database::decode::{DbResultDecoder, Single};
        Single::<Vec<f32>>::decode_result(result)
            .map_err(|e| ErrorResponse::from(format!("Failed to decode embedding: {:?}", e)))?
            .into_iter()
            .next()
            .map(|s| s.0)
            .ok_or_else(|| ErrorResponse::from("Document embedding not found"))
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
            let placeholders: Vec<String> = filters
                .tags
                .iter()
                .map(|tag| format!("'{}'", tag))
                .collect();
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM documents d WHERE d.id = dc.document_id AND d.tags && ARRAY[{}])",
                placeholders.join(", ")
            ));
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

    fn keyword_search(
        &self,
        db_helper: &DatabaseHelper,
        query: &str,
        limit: usize,
        filters: Option<&SearchFilters>,
    ) -> AgentResult<Vec<SearchResult>> {
        let filter_conditions = filters
            .map(|f| self.build_filter_conditions(f))
            .unwrap_or_else(|| "1=1".to_string());

        // Use PostgreSQL full-text search with tsvector and tsquery
        let sql_query = format!(
            r#"
            SELECT 
                dc.id,
                dc.document_id,
                dc.chunk_index,
                dc.content,
                dc.start_pos,
                dc.end_pos,
                dc.token_count,
                ts_rank_cd(to_tsvector('english', dc.content), plainto_tsquery('english', $1)) as similarity_score
            FROM document_chunks dc
            WHERE {}
              AND to_tsvector('english', dc.content) @@ plainto_tsquery('english', $1)
            ORDER BY similarity_score DESC
            LIMIT $2
            "#,
            filter_conditions
        );

        let result = db_helper
            .connection
            .query(&sql_query, encode_params![query, limit as i32,])
            .map_err(|e| format!("Failed to execute keyword search query: {:?}", e))?;

        use crate::common_lib::database::decode::DbResultDecoder;
        let mut search_results = SearchResult::decode_result(result)
            .map_err(|e| format!("Failed to decode search results: {:?}", e))?;

        // Add highlights
        for result in &mut search_results {
            result.relevance_explanation = self.generate_highlight(&result.chunk.content, query);
        }

        Ok(search_results)
    }

    fn fuse_results(
        &self,
        semantic_results: Vec<SearchResult>,
        keyword_results: Vec<SearchResult>,
        config: &HybridSearchConfig,
    ) -> AgentResult<Vec<HybridSearchResult>> {
        let mut fused_results = std::collections::HashMap::new();

        // Process semantic results
        for (rank, result) in semantic_results.iter().enumerate() {
            let chunk_id = &result.chunk.id;
            let semantic_rrf_score = 1.0 / (config.rrf_k + (rank + 1) as f32);

            fused_results.insert(
                chunk_id.clone(),
                HybridSearchResult {
                    chunk: result.chunk.clone(),
                    semantic_score: result.similarity_score,
                    keyword_score: 0.0,
                    combined_score: semantic_rrf_score * config.semantic_weight,
                    match_type: MatchType::SemanticOnly,
                    relevance_explanation: result.relevance_explanation.clone(),
                },
            );
        }

        // Process keyword results
        for (rank, result) in keyword_results.iter().enumerate() {
            let chunk_id = &result.chunk.id;
            let keyword_rrf_score = 1.0 / (config.rrf_k + (rank + 1) as f32);

            if let Some(existing) = fused_results.get_mut(chunk_id) {
                // Update existing result with keyword score
                existing.keyword_score = result.similarity_score;
                existing.combined_score += keyword_rrf_score * config.keyword_weight;
                existing.match_type = MatchType::BothMatch;
                if existing.relevance_explanation.is_none() {
                    existing.relevance_explanation = result.relevance_explanation.clone();
                }
            } else {
                // Add new keyword-only result
                fused_results.insert(
                    chunk_id.clone(),
                    HybridSearchResult {
                        chunk: result.chunk.clone(),
                        semantic_score: 0.0,
                        keyword_score: result.similarity_score,
                        combined_score: keyword_rrf_score * config.keyword_weight,
                        match_type: MatchType::KeywordOnly,
                        relevance_explanation: result.relevance_explanation.clone(),
                    },
                );
            }
        }

        // Convert to sorted vector
        let mut results: Vec<HybridSearchResult> = fused_results.into_values().collect();
        results.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }
}
