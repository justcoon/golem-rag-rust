import type { SearchFilters, HybridSearchConfig, SearchOptions } from '../types/search';

const API_BASE_URL = '/api';

export const ApiService = {
  async hybridSearch(query: string, options: SearchOptions = {}) {
    const { filters = null, limit = 10, threshold = 0.5, config = null } = options;
    
    try {
      const response = await fetch(`${API_BASE_URL}/search`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          query,
          filters,
          limit,
          threshold,
          config: config ? {
            'semantic_weight': config.semantic_weight,
            'keyword_weight': config.keyword_weight,
            'enable_semantic': config.enable_semantic,
            'enable_keyword': config.enable_keyword,
            'rrf_k': config.rrf_k
          } : {
            'semantic_weight': 0.7,
            'keyword_weight': 0.3,
            'enable_semantic': true,
            'enable_keyword': true,
            'rrf_k': 60.0
          }
        }),
      });

      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }

      const data = await response.json();
      return data;
    } catch (error) {
      console.error('Hybrid search failed:', error);
      throw error;
    }
  },

  async getDocument(documentId) {
    try {
      const response = await fetch(`${API_BASE_URL}/documents/${documentId}`);
      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }
      const data = await response.json();
      return data;
    } catch (error) {
      console.error('Failed to fetch document:', error);
      throw error;
    }
  },

  async findSimilar(documentId, limit = 5) {
    try {
      const response = await fetch(`${API_BASE_URL}/search/similar`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ 'document_id': documentId, limit }),
      });
      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }
      const data = await response.json();
      return data;
    } catch (error) {
      console.error('Failed to find similar documents:', error);
      throw error;
    }
  }
};
