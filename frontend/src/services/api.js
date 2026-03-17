const API_BASE_URL = '/api';

export const ApiService = {
  async hybridSearch(query, { filters = null, limit = 10, threshold = 0.5, config = null } = {}) {
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
            'semantic-weight': config.semantic_weight,
            'keyword-weight': config.keyword_weight,
            'enable-semantic': config.enable_semantic,
            'enable-keyword': config.enable_keyword,
            'rrf-k': config.rrf_k
          } : {
            'semantic-weight': 0.7,
            'keyword-weight': 0.3,
            'enable-semantic': true,
            'enable-keyword': true,
            'rrf-k': 60.0
          }
        }),
      });

      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }

      const data = await response.json();
      if (data.ok) return data.ok;
      if (data.err) throw new Error(data.err.error || 'Failed to search');
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
      if (data.ok) return data.ok;
      if (data.err) throw new Error(data.err.error || 'Failed to load document');
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
        body: JSON.stringify({ 'document-id': documentId, limit }),
      });
      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }
      const data = await response.json();
      if (data.ok) return data.ok;
      if (data.err) throw new Error(data.err.error || 'Failed to find similar documents');
      return data;
    } catch (error) {
      console.error('Failed to find similar documents:', error);
      throw error;
    }
  }
};
