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
    fn search(&self, query: String, limit: Option<usize>, threshold: Option<f32>) -> AgentResult<Vec<SearchResult>>;
    
    /// Search documents with metadata filters
    fn search_with_filters(&self, query: String, filters: SearchFilters, limit: Option<usize>, threshold: Option<f32>) -> AgentResult<Vec<SearchResult>>;
    
    /// Get similar documents to a specific document
    fn find_similar_documents(&self, document_id: String, limit: Option<usize>) -> AgentResult<Vec<SearchResult>>;
}

struct SearchAgentImpl {
    db_url: String,
    embedding_model: String,
}

#[agent_implementation]
impl SearchAgent for SearchAgentImpl {
    fn new() -> Self {
        let db_url = std::env::var("DB_URL")
            .expect("DB_URL environment variable must be set");
        
        let embedding_model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "mock-embedding-v1".to_string());
        
        Self { db_url, embedding_model }
    }
    
    fn search(&self, query: String, limit: Option<usize>, threshold: Option<f32>) -> AgentResult<Vec<SearchResult>> {
        log::info!("Performing semantic search for query: {}", query);
        
        let db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_url) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };
        
        let limit = limit.unwrap_or(10);
        let threshold = threshold.unwrap_or(0.7);
        
        // Generate query embedding
        let query_embedding = self.generate_query_embedding(&query)?;
        
        // Perform vector similarity search
        let results = self.vector_similarity_search(&db_helper, &query_embedding, limit, threshold)?;
        
        Ok(results)
    }
    
    fn search_with_filters(&self, query: String, filters: SearchFilters, limit: Option<usize>, threshold: Option<f32>) -> AgentResult<Vec<SearchResult>> {
        log::info!("Performing filtered search for query: {}", query);
        
        let db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_url) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };
        
        let limit = limit.unwrap_or(10);
        let threshold = threshold.unwrap_or(0.7);
        
        // Generate query embedding
        let query_embedding = self.generate_query_embedding(&query)?;
        
        // Build filter conditions
        let filter_conditions = self.build_filter_conditions(&filters);
        
        // Perform filtered vector similarity search
        let results = self.filtered_vector_similarity_search(&db_helper, &query_embedding, &filter_conditions, limit, threshold)?;
        
        Ok(results)
    }
    
    fn find_similar_documents(&self, document_id: String, limit: Option<usize>) -> AgentResult<Vec<SearchResult>> {
        log::info!("Finding similar documents to: {}", document_id);
        
        let db_helper: DatabaseHelper = match DatabaseHelper::new(&self.db_url) {
            Ok(helper) => helper,
            Err(e) => return Err(format!("Failed to create database helper: {:?}", e)),
        };
        
        let limit = limit.unwrap_or(10);
        
        // Get document embedding
        let document_embedding = self.get_document_embedding(&db_helper, &document_id)?;
        
        // Find similar documents
        let results = self.vector_similarity_search(&db_helper, &document_embedding, limit + 1, 0.5)?;
        
        // Filter out the original document
        let filtered_results: Vec<SearchResult> = results
            .into_iter()
            .filter(|result| result.document.id != document_id)
            .take(limit)
            .collect();
        
        Ok(filtered_results)
    }
}

impl SearchAgentImpl {
    fn generate_query_embedding(&self, query: &str) -> AgentResult<Vec<f32>> {
        // For now, use mock embedding generation
        // In a real implementation, this would use the embedding client
        log::debug!("Generating embedding for query: {}", query);
        Ok(EmbeddingClient::mock_embedding(query, 768))
    }
    
    fn vector_similarity_search(&self, db_helper: &DatabaseHelper, query_embedding: &[f32], limit: usize, threshold: f32) -> AgentResult<Vec<SearchResult>> {
        // Simplified vector similarity search
        // In a real implementation, this would use pgvector's <-> operator
        
        let query = r#"
            SELECT 
                dc.id as chunk_id,
                dc.document_id,
                dc.chunk_index,
                dc.content as chunk_text,
                dc.start_pos,
                dc.end_pos,
                dc.token_count,
                d.title,
                d.content as document_content,
                d.metadata
            FROM document_chunks dc
            JOIN documents d ON dc.document_id = d.id
            JOIN embeddings e ON dc.id = e.chunk_id
            WHERE e.embedding_status = 'completed'
            ORDER BY dc.id
            LIMIT $1
        "#;
        
        let result = db_helper.connection.query(query, vec![PostgresDbValue::Int4(limit as i32)])
            .map_err(|e| format!("Failed to execute vector search query: {:?}", e))?;
        
        let mut search_results = Vec::new();
        
        for row in result.rows {
            let chunk_id = try_match!(&row.values[0], PostgresDbValue::Text(id) => id.clone()).map_err(|_| "Invalid chunk ID type".to_string())?;
            let document_id = try_match!(&row.values[1], PostgresDbValue::Text(id) => id.clone()).map_err(|_| "Invalid document ID type".to_string())?;
            let chunk_index = try_match!(&row.values[2], PostgresDbValue::Int4(index) => *index as u32).map_err(|_| "Invalid chunk index type".to_string())?;
            let chunk_text = try_match!(&row.values[3], PostgresDbValue::Text(text) => text.clone()).map_err(|_| "Invalid chunk text type".to_string())?;
            let start_pos = try_match!(&row.values[4], PostgresDbValue::Int4(pos) => *pos as u32).map_err(|_| "Invalid start position type".to_string())?;
            let end_pos = try_match!(&row.values[5], PostgresDbValue::Int4(pos) => *pos as u32).map_err(|_| "Invalid end position type".to_string())?;
            let token_count = try_match!(&row.values[6], PostgresDbValue::Int4(count) => Some(*count as u32)).map_err(|_| "Invalid token count type".to_string())?;
            let title = try_match!(&row.values[7], PostgresDbValue::Text(title) => title.clone()).map_err(|_| "Invalid title type".to_string())?;
            let document_content = try_match!(&row.values[8], PostgresDbValue::Text(content) => content.clone()).map_err(|_| "Invalid document content type".to_string())?;
            let metadata_str = try_match!(&row.values[9], PostgresDbValue::Jsonb(metadata) => metadata.clone()).map_err(|_| "Invalid metadata type".to_string())?;
            
            let metadata: DocumentMetadata = serde_json::from_str(&metadata_str)
                .map_err(|e| format!("Failed to parse document metadata: {:?}", e))?;
            
            // Calculate similarity score (mock implementation)
            let similarity_score = self.calculate_similarity(query_embedding, &[]);
            
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
                
                let document = Document {
                    id: document_id,
                    title,
                    content: document_content,
                    metadata,
                };
                
                let search_result = SearchResult {
                    chunk: document_chunk,
                    document,
                    similarity_score,
                    relevance_explanation: self.generate_highlight(&chunk_text, ""),
                };
                
                search_results.push(search_result);
            }
        }
        
        // Sort by similarity score
        search_results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(search_results)
    }
    
    fn filtered_vector_similarity_search(&self, db_helper: &DatabaseHelper, query_embedding: &[f32], filter_conditions: &str, limit: usize, threshold: f32) -> AgentResult<Vec<SearchResult>> {
        // Similar to vector_similarity_search but with additional WHERE clauses
        let query = format!(r#"
            SELECT 
                dc.id as chunk_id,
                dc.document_id,
                dc.chunk_index,
                dc.content as chunk_text,
                d.title,
                d.metadata,
                e.vector
            FROM document_chunks dc
            JOIN documents d ON dc.document_id = d.id
            JOIN embeddings e ON dc.id = e.chunk_id
            WHERE e.embedding_status = 'completed' AND {}
            ORDER BY dc.id
            LIMIT $1
        "#, filter_conditions);
        
        let result = db_helper.connection.query(&query, vec![PostgresDbValue::Int4(limit as i32)])
            .map_err(|e| format!("Failed to execute filtered vector search query: {:?}", e))?;
        
        // Process results similar to vector_similarity_search
        let mut search_results = Vec::new();
        
        for row in result.rows {
            let chunk_id = try_match!(&row.values[0], PostgresDbValue::Text(id) => id.clone()).map_err(|_| "Invalid chunk ID type".to_string())?;
            let document_id = try_match!(&row.values[1], PostgresDbValue::Text(id) => id.clone()).map_err(|_| "Invalid document ID type".to_string())?;
            let chunk_index = try_match!(&row.values[2], PostgresDbValue::Int4(index) => *index as u32).map_err(|_| "Invalid chunk index type".to_string())?;
            let chunk_text = try_match!(&row.values[3], PostgresDbValue::Text(text) => text.clone()).map_err(|_| "Invalid chunk text type".to_string())?;
            let start_pos = try_match!(&row.values[4], PostgresDbValue::Int4(pos) => *pos as u32).map_err(|_| "Invalid start position type".to_string())?;
            let end_pos = try_match!(&row.values[5], PostgresDbValue::Int4(pos) => *pos as u32).map_err(|_| "Invalid end position type".to_string())?;
            let token_count = try_match!(&row.values[6], PostgresDbValue::Int4(count) => Some(*count as u32)).map_err(|_| "Invalid token count type".to_string())?;
            let title = try_match!(&row.values[7], PostgresDbValue::Text(title) => title.clone()).map_err(|_| "Invalid title type".to_string())?;
            let document_content = try_match!(&row.values[8], PostgresDbValue::Text(content) => content.clone()).map_err(|_| "Invalid document content type".to_string())?;
            let metadata_str = try_match!(&row.values[9], PostgresDbValue::Jsonb(metadata) => metadata.clone()).map_err(|_| "Invalid metadata type".to_string())?;
            
            let metadata: DocumentMetadata = serde_json::from_str(&metadata_str)
                .map_err(|e| format!("Failed to parse document metadata: {:?}", e))?;
            
            let similarity_score = self.calculate_similarity(query_embedding, &[]);
            
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
                
                let document = Document {
                    id: document_id,
                    title,
                    content: document_content,
                    metadata,
                };
                
                let search_result = SearchResult {
                    chunk: document_chunk,
                    document,
                    similarity_score,
                    relevance_explanation: self.generate_highlight(&chunk_text, ""),
                };
                
                search_results.push(search_result);
            }
        }
        
        search_results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(search_results)
    }
    
    fn get_document_embedding(&self, db_helper: &DatabaseHelper, document_id: &str) -> AgentResult<Vec<f32>> {
        let query = r#"
            SELECT e.vector
            FROM embeddings e
            JOIN document_chunks dc ON e.chunk_id = dc.id
            WHERE dc.document_id = $1 AND e.embedding_status = 'completed'
            LIMIT 1
        "#;
        
        let result = db_helper.connection.query(query, vec![PostgresDbValue::Text(document_id.to_string())])
            .map_err(|e| format!("Failed to get document embedding: {:?}", e))?;
        
        if result.rows.is_empty() {
            return Err("Document embedding not found".to_string());
        }
        
        // For now, return a mock embedding
        // In a real implementation, this would extract the actual vector from the database
        Ok(EmbeddingClient::mock_embedding(document_id, 768))
    }
    
    fn build_filter_conditions(&self, filters: &SearchFilters) -> String {
        let mut conditions = Vec::new();
        
        if !filters.content_types.is_empty() {
            let placeholders: Vec<String> = filters.content_types.iter()
                .map(|ct| format!("'{}{:?}'", "", ct))
                .collect();
            conditions.push(format!("d.metadata->>'content_type' IN ({})", placeholders.join(", ")));
        }
        
        if !filters.tags.is_empty() {
            for tag in &filters.tags {
                conditions.push(format!("d.metadata->'tags' ? '{}'", tag));
            }
        }
        
        if !filters.sources.is_empty() {
            for source in &filters.sources {
                conditions.push(format!("d.metadata->>'source' = '{}'", source));
            }
        }
        
        if let Some(ref date_range) = filters.date_range {
            conditions.push(format!("d.created_at >= '{}'", date_range.start));
            conditions.push(format!("d.created_at <= '{}'", date_range.end));
        }
        
        if conditions.is_empty() {
            "1=1".to_string()
        } else {
            conditions.join(" AND ")
        }
    }
    
    fn calculate_similarity(&self, query_embedding: &[f32], document_embedding: &[f32]) -> f32 {
        // Mock similarity calculation
        // In a real implementation, this would calculate cosine similarity
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        query_embedding.iter().for_each(|&x| x.to_bits().hash(&mut hasher));
        let hash = hasher.finish();
        
        (hash % 1000) as f32 / 1000.0
    }
    
    fn calculate_keyword_score(&self, chunk_text: &str, title: &str, query: &str) -> f32 {
        // Simple keyword scoring based on term frequency
        let combined_text = format!("{} {}", title.to_lowercase(), chunk_text.to_lowercase());
        let query_lower = query.to_lowercase();
        
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let mut score = 0.0;
        
        for word in &query_words {
            let count = combined_text.matches(word).count() as f32;
            score += count * 0.1; // Each occurrence adds 0.1 to the score
        }
        
        // Normalize score
        (score / query_words.len() as f32).min(1.0)
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
            let before = if start_pos > 50 { start_pos - 50 } else { 0 };
            let after = if end_pos + 50 < chunk_text.len() { end_pos + 50 } else { chunk_text.len() };
            
            Some(format!("...{}[{}]{}...", 
                &chunk_text[before..start_pos], 
                &chunk_text[start_pos..end_pos], 
                &chunk_text[end_pos..after]))
        } else {
            None
        }
    }
}
