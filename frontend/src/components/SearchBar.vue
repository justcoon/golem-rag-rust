<script setup lang="ts">
import { ref, watch } from 'vue';
import type { SearchFilters as SearchFiltersType, HybridSearchConfig, ContentType, DateRange } from '../types/search';

const props = defineProps(['loading']);
const emit = defineEmits(['search']);

const query = ref('');
const showConfig = ref(false);
const showFilters = ref(false);
const filters = ref<SearchFiltersType>({
  tags: [],
  sources: [],
  'content-types': [],
  'date-range': null
});
const newTag = ref('');
const newSource = ref('');
const dateRange = ref<DateRange>({
  start: '',
  end: ''
});
const config = ref<HybridSearchConfig>({
  semantic_weight: 0.7,
  keyword_weight: 0.3,
  enable_semantic: true,
  enable_keyword: true,
  rrf_k: 60.0
});

// Available options (these could be fetched from API in a real app)
const availableTags = ref(['research', 'documentation', 'tutorial', 'reference', 'guide']);
const availableSources = ref(['github', 'wikipedia', 'documentation', 'blog', 'academic']);
const availableContentTypes = ref([
  { value: 'text' as ContentType, label: 'Text' },
  { value: 'markdown' as ContentType, label: 'Markdown' },
  { value: 'pdf' as ContentType, label: 'PDF' },
  { value: 'html' as ContentType, label: 'HTML' },
  { value: 'json' as ContentType, label: 'JSON' }
]);

const handleSearch = () => {
  if (query.value.trim()) {
    showFilters.value = false; // Hide filters when search is triggered
    emit('search', query.value, config.value, filters.value);
  }
};

const handleFiltersToggle = () => {
  showFilters.value = !showFilters.value;
};

const addTag = () => {
  if (newTag.value.trim() && !filters.value.tags.includes(newTag.value.trim())) {
    filters.value.tags.push(newTag.value.trim());
    newTag.value = '';
  }
};

const removeTag = (tag: string) => {
  const index = filters.value.tags.indexOf(tag);
  if (index > -1) {
    filters.value.tags.splice(index, 1);
  }
};

const addSource = () => {
  if (newSource.value.trim() && !filters.value.sources.includes(newSource.value.trim())) {
    filters.value.sources.push(newSource.value.trim());
    newSource.value = '';
  }
};

const removeSource = (source: string) => {
  const index = filters.value.sources.indexOf(source);
  if (index > -1) {
    filters.value.sources.splice(index, 1);
  }
};

const toggleContentType = (contentType: ContentType) => {
  const index = filters.value['content-types'].indexOf(contentType);
  if (index > -1) {
    filters.value['content-types'].splice(index, 1);
  } else {
    filters.value['content-types'].push(contentType);
  }
};

const clearAllFilters = () => {
  filters.value = {
    tags: [],
    sources: [],
    'content-types': [],
    'date-range': null
  };
  dateRange.value = { start: '', end: '' };
  newTag.value = '';
  newSource.value = '';
};

// Helper function to format date for PostgreSQL timestamptz
const formatDateForPostgres = (date: Date, endOfDay = false) => {
  if (endOfDay) {
    date.setHours(23, 59, 59, 999);
  }
  
  // Simple approach: use toISOString() and convert to PostgreSQL format
  const iso = date.toISOString();
  // Convert "2024-01-01T00:00:00.000Z" to "2024-01-01 00:00:00.000+00"
  return iso.replace('T', ' ').replace('Z', '+00');
};

watch(dateRange, (newRange) => {
  if (newRange.start || newRange.end) {
    const startDate = newRange.start ? formatDateForPostgres(new Date(newRange.start)) : null;
    const endDate = newRange.end ? formatDateForPostgres(new Date(newRange.end), true) : null;
    
    filters.value['date-range'] = { 
      start: startDate || '', 
      end: endDate || '' 
    };
  } else {
    filters.value['date-range'] = null;
  }
}, { deep: true });

watch([() => config.value.semantic_weight, () => config.value.keyword_weight], () => {
  // Ensure weights sum to 1.0 or just let them be weights? 
  // RRF usually uses weights to balance the results.
});
</script>

<template>
  <div class="search-container animate-fade-in">
    <div class="search-bar glass">
      <input 
        v-model="query" 
        @keyup.enter="handleSearch"
        type="text" 
        placeholder="Ask anything about your documents..."
        :disabled="loading"
      />
      <button @click="handleSearch" :disabled="loading || !query.trim()">
        <span v-if="!loading">Search</span>
        <span v-else class="loader"></span>
      </button>
      <button class="filters-toggle" @click="handleFiltersToggle" :class="{ active: showFilters }">
        🔽
      </button>
      <button class="config-toggle" @click="showConfig = !showConfig">
        ⚙️
      </button>
    </div>

    <div v-if="showConfig" class="config-panel glass animate-fade-in">
      <h3>Hybrid Search Configuration</h3>
      <div class="config-grid">
        <div class="config-item">
          <label>Semantic Weight: {{ config.semantic_weight }}</label>
          <input type="range" v-model.number="config.semantic_weight" min="0" max="1" step="0.1" />
        </div>
        <div class="config-item">
          <label>Keyword Weight: {{ config.keyword_weight }}</label>
          <input type="range" v-model.number="config.keyword_weight" min="0" max="1" step="0.1" />
        </div>
        <div class="config-item checkbox">
          <label><input type="checkbox" v-model="config.enable_semantic" /> Semantic Search</label>
        </div>
        <div class="config-item checkbox">
          <label><input type="checkbox" v-model="config.enable_keyword" /> Keyword Search</label>
        </div>
      </div>
    </div>

    <div v-if="showFilters" class="filters-panel glass animate-fade-in">
      <div class="filters-header">
        <h3>Search Filters</h3>
        <button @click="clearAllFilters" class="clear-btn">Clear All</button>
      </div>

      <div class="filter-section">
        <h4>Content Type</h4>
        <div class="filter-options">
          <label 
            v-for="type in availableContentTypes" 
            :key="type.value"
            class="filter-option"
          >
            <input 
              type="checkbox" 
              :checked="filters['content-types'].includes(type.value)"
              @change="toggleContentType(type.value)"
            />
            <span>{{ type.label }}</span>
          </label>
        </div>
      </div>

      <div class="filter-section">
        <h4>Sources</h4>
        <div class="input-group">
          <div class="input-with-button">
            <input 
              type="text" 
              v-model="newSource"
              @keyup.enter="addSource"
              placeholder="Add source..."
              class="filter-input"
            />
            <button @click="addSource" class="add-btn">+</button>
          </div>
          <div class="tag-list">
            <span 
              v-for="source in filters.sources" 
              :key="source"
              class="tag-item"
            >
              {{ source }}
              <button @click="removeSource(source)" class="remove-btn">×</button>
            </span>
          </div>
        </div>
      </div>

      <div class="filter-section">
        <h4>Tags</h4>
        <div class="input-group">
          <div class="input-with-button">
            <input 
              type="text" 
              v-model="newTag"
              @keyup.enter="addTag"
              placeholder="Add tag..."
              class="filter-input"
            />
            <button @click="addTag" class="add-btn">+</button>
          </div>
          <div class="tag-list">
            <span 
              v-for="tag in filters.tags" 
              :key="tag"
              class="tag-item"
            >
              {{ tag }}
              <button @click="removeTag(tag)" class="remove-btn">×</button>
            </span>
          </div>
        </div>
      </div>

      <div class="filter-section">
        <h4>Date Range</h4>
        <div class="date-range">
          <input 
            type="date" 
            v-model="dateRange.start"
            placeholder="Start date"
          />
          <span>to</span>
          <input 
            type="date" 
            v-model="dateRange.end"
            placeholder="End date"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.search-container {
  width: 100%;
  max-width: 900px;
  margin: 0 auto;
}

.search-bar {
  display: flex;
  padding: 8px;
  gap: 8px;
  margin-bottom: 16px;
  transition: all 0.3s ease;
  position: relative;
}

.search-bar:focus-within {
  box-shadow: 0 0 30px var(--primary-glow);
  border-color: var(--primary);
}

input[type="text"] {
  flex: 1;
  background: transparent;
  border: none;
  color: white;
  padding: 12px 16px;
  font-size: 1.1rem;
}

.config-toggle {
  background: transparent;
  width: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  font-size: 1.2rem;
}

.config-panel {
  padding: 20px;
  margin-bottom: 24px;
  text-align: left;
}

.filters-panel {
  padding: 20px;
  margin-bottom: 24px;
  text-align: left;
}

.filters-toggle {
  background: transparent;
  width: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  font-size: 1.2rem;
}

.filters-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.filters-header h3 {
  margin: 0;
  font-size: 0.9rem;
  text-transform: uppercase;
  color: var(--text-muted);
}

.clear-btn {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.3);
  color: white;
  padding: 4px 12px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.8rem;
  transition: all 0.3s ease;
}

.clear-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.5);
}

.filter-section {
  margin-bottom: 24px;
}

.filter-section h4 {
  margin: 0 0 12px 0;
  font-size: 0.9rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.filter-options {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 8px 16px;
}

.filter-option {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  color: white;
  font-size: 0.9rem;
}

.filter-option input[type="checkbox"] {
  accent-color: var(--primary);
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.input-with-button {
  display: flex;
  gap: 8px;
}

.filter-input {
  flex: 1;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: white;
  padding: 8px 12px;
  border-radius: 4px;
  font-size: 0.9rem;
}

.filter-input:focus {
  outline: none;
  border-color: var(--primary);
}

.filter-input::placeholder {
  color: rgba(255, 255, 255, 0.5);
}

.add-btn {
  background: var(--primary);
  border: none;
  color: white;
  width: 32px;
  height: 32px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  display: flex;
  align-items: center;
  justify-content: center;
}

.add-btn:hover {
  background: var(--primary);
  opacity: 0.8;
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.tag-item {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: white;
  padding: 4px 8px;
  border-radius: 12px;
  font-size: 0.8rem;
  display: flex;
  align-items: center;
  gap: 6px;
}

.remove-btn {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  font-size: 0.9rem;
  padding: 0;
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
}

.remove-btn:hover {
  background: rgba(255, 255, 255, 0.2);
  color: white;
}

.date-range {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  gap: 12px;
  align-items: center;
}

.date-range input {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: white;
  padding: 6px 12px;
  border-radius: 4px;
  font-size: 0.9rem;
}

.date-range input:focus {
  outline: none;
  border-color: var(--primary);
}

.date-range span {
  color: var(--text-muted);
  font-size: 0.9rem;
}

.config-panel h3 {
  margin-bottom: 16px;
  font-size: 0.9rem;
  text-transform: uppercase;
  color: var(--text-muted);
}

.config-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 20px;
}

.config-item label {
  display: block;
  margin-bottom: 8px;
  font-size: 0.9rem;
}

.config-item input[type="range"] {
  width: 100%;
  accent-color: var(--primary);
}

.checkbox label {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.loader {
  width: 20px;
  height: 20px;
  border: 2px solid #FFF;
  border-bottom-color: transparent;
  border-radius: 50%;
  display: inline-block;
  animation: rotation 1s linear infinite;
}

@keyframes rotation {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}
</style>
