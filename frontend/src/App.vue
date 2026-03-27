<script setup lang="ts">
import { ref } from 'vue';
import { ApiService } from './services/api';
import SearchBar from './components/SearchBar.vue';
import ResultCard from './components/ResultCard.vue';
import DocumentModal from './components/DocumentModal.vue';
import type { SearchFilters as SearchFiltersType, HybridSearchConfig } from './types/search';

const results = ref([]);
const loading = ref(false);
const error = ref(null);

const selectedDocument = ref(null);
const showModal = ref(false);

const onSearch = async (query: string, config: HybridSearchConfig, filters: SearchFiltersType | null) => {
  loading.value = true;
  error.value = null;
  results.value = [];
  
  try {
    results.value = await ApiService.hybridSearch(query, { filters, config });
  } catch (err) {
    error.value = "Failed to fetch results. Is the Golem server running?";
  } finally {
    loading.value = false;
  }
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

      <div v-if="error" class="error-state glass animate-fade-in">
        <p>⚠️ {{ error }}</p>
      </div>

      <div v-if="results.length > 0" class="results-grid">
        <ResultCard 
          v-for="res in results" 
          :key="res.chunk.id" 
          :result="res" 
          @preview="openPreview"
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
