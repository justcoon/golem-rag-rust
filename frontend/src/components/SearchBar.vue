<script setup>
import { ref, watch } from 'vue';

const props = defineProps(['loading']);
const emit = defineEmits(['search']);

const query = ref('');
const showConfig = ref(false);
const config = ref({
  semantic_weight: 0.7,
  keyword_weight: 0.3,
  enable_semantic: true,
  enable_keyword: true,
  rrf_k: 60.0
});

const handleSearch = () => {
  if (query.value.trim()) {
    emit('search', query.value, config.value);
  }
};

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
  </div>
</template>

<style scoped>
.search-container {
  width: 100%;
  max-width: 800px;
  margin: 0 auto;
}

.search-bar {
  display: flex;
  padding: 8px;
  gap: 8px;
  margin-bottom: 16px;
  transition: all 0.3s ease;
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
