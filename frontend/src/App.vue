<script setup lang="ts">
import { ref } from 'vue';
import { ApiService } from './services/api';
import SearchBar from './components/SearchBar.vue';
import ResultCard from './components/ResultCard.vue';
import DocumentModal from './components/DocumentModal.vue';
import type { SearchFilters as SearchFiltersType, HybridSearchConfig } from './types/search';

const results = ref([]);
const originalResults = ref([]);
const loading = ref(false);
const error = ref(null);

const selectedDocument = ref(null);
const showModal = ref(false);
const similarityContext = ref<string | null>(null);

const onSearch = async (query: string, config: HybridSearchConfig, filters: SearchFiltersType | null) => {
  loading.value = true;
  error.value = null;
  results.value = [];
  originalResults.value = [];
  similarityContext.value = null;
  
  try {
    const searchResults = await ApiService.hybridSearch(query, { filters, config });
    results.value = searchResults;
    originalResults.value = searchResults;
  } catch (err) {
    error.value = "Failed to fetch results. Is the Golem server running?";
  } finally {
    loading.value = false;
  }
};

const onFindSimilar = async (docId: string) => {
  loading.value = true;
  error.value = null;
  showModal.value = false;
  
  try {
    const similarResults = await ApiService.findSimilar(docId);
    // Map SearchResult to HybridSearchResult structure expected by ResultCard
    results.value = similarResults.map((res: any) => ({
      ...res,
      combined_score: res.similarity_score,
      semantic_score: res.similarity_score,
      keyword_score: 0.0,
      match_type: 'SemanticOnly'
    }));
    similarityContext.value = docId;
  } catch (err) {
    error.value = "Failed to find similar documents.";
    console.error(err);
  } finally {
    loading.value = false;
  }
};

const clearSimilarity = () => {
  similarityContext.value = null;
  results.value = originalResults.value;
};

const openPreview = async (docId) => {
  try {
    selectedDocument.value = await ApiService.getDocument(docId);
    showModal.value = true;
  } catch (err) {
    alert("Error loading document: " + err.message);
  }
};
</script>

<template>
  <div class="app-wrapper">
    <header class="app-header animate-fade-in">
      <div class="logo">
        <span class="icon">🚀</span>
        <h1 class="gradient-text">Golem RAG</h1>
      </div>
      <p class="subtitle">Intelligent Document Retrieval with Hybrid Search</p>
    </header>

    <main class="container">
      <SearchBar @search="onSearch" :loading="loading" />

      <div v-if="similarityContext" class="similarity-banner glass animate-fade-in">
        <div class="banner-content">
          <span class="magic-icon">✨</span>
          <p>Showing documents similar to <code>{{ similarityContext }}</code></p>
        </div>
        <button class="clear-btn" @click="clearSimilarity">Clear</button>
      </div>

      <div v-if="error" class="error-state glass animate-fade-in">
        <p>⚠️ {{ error }}</p>
      </div>

      <div v-if="results.length > 0" class="results-grid">
        <ResultCard 
          v-for="res in results" 
          :key="res.chunk.id" 
          :result="res" 
          @preview="openPreview"
          @find-similar="onFindSimilar"
        />
      </div>

      <div v-else-if="!loading && !error" class="empty-state animate-fade-in">
        <div class="empty-icon">📂</div>
        <p>Your search results will appear here</p>
      </div>
    </main>

    <DocumentModal 
      :document="selectedDocument" 
      :show="showModal" 
      @close="showModal = false" 
      @find-similar="onFindSimilar"
    />

    <footer class="app-footer">
      <p>Built with Golem Cloud & Vue 3</p>
    </footer>
  </div>
</template>

<style>
.app-wrapper {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 40px 20px;
  min-height: 100vh;
}

.app-header {
  text-align: center;
  margin-bottom: 60px;
}

.logo {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  margin-bottom: 8px;
}

.logo h1 {
  font-size: 3rem;
  margin: 0;
}

.subtitle {
  color: var(--text-muted);
  font-size: 1.1rem;
}

.container {
  width: 100%;
  max-width: 900px;
}

.results-grid {
  margin-top: 40px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.similarity-banner {
  margin-top: 24px;
  padding: 12px 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-color: rgba(147, 51, 234, 0.3);
  background: rgba(147, 51, 234, 0.05);
}

.banner-content {
  display: flex;
  align-items: center;
  gap: 12px;
}

.magic-icon {
  font-size: 1.2rem;
}

.banner-content p {
  margin: 0;
  font-size: 0.95rem;
}

.banner-content code {
  background: rgba(255, 255, 255, 0.1);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: monospace;
  color: var(--primary);
}

.clear-btn {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: var(--text-muted);
  padding: 4px 12px;
  border-radius: 6px;
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.2s ease;
}

.clear-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: white;
  border-color: white;
}

.error-state {
  margin-top: 40px;
  padding: 20px;
  border-color: rgba(239, 68, 68, 0.3);
  color: #f87171;
}

.empty-state {
  margin-top: 80px;
  text-align: center;
  color: var(--text-muted);
}

.empty-icon {
  font-size: 4rem;
  margin-bottom: 16px;
  opacity: 0.3;
}

.app-footer {
  margin-top: auto;
  padding: 40px 0;
  color: var(--text-muted);
  font-size: 0.8rem;
}

/* Custom Scrollbar */
::-webkit-scrollbar {
  width: 8px;
}

::-webkit-scrollbar-track {
  background: var(--bg-dark);
}

::-webkit-scrollbar-thumb {
  background: var(--glass-border);
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: var(--text-muted);
}
</style>
