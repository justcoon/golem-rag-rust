<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import type { SearchFilters, DateRange, ContentType } from '../types/search';

const props = defineProps(['modelValue', 'show']);
const emit = defineEmits(['update:modelValue', 'search-triggered']);

const filters = ref<SearchFilters>({
  tags: [],
  sources: [],
  'content-types': [],
  'date-range': null
});

const showFilters = ref(false);

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

const dateRange = ref<DateRange>({
  start: '',
  end: ''
});

watch(() => props.show, (newValue) => {
  showFilters.value = newValue;
});

// Sync with v-model
watch(() => props.modelValue, (newValue) => {
  if (newValue) {
    filters.value = { ...newValue };
    if (newValue['date-range']) {
      dateRange.value = { ...newValue['date-range'] };
    }
  }
}, { immediate: true });

watch(filters, (newFilters) => {
  emit('update:modelValue', { ...newFilters });
}, { deep: true });

watch(dateRange, (newRange) => {
  if (newRange.start || newRange.end) {
    filters.value['date-range'] = { ...newRange };
  } else {
    filters.value['date-range'] = null;
  }
}, { deep: true });

const toggleTag = (tag: string) => {
  const index = filters.value.tags.indexOf(tag);
  if (index > -1) {
    filters.value.tags.splice(index, 1);
  } else {
    filters.value.tags.push(tag);
  }
};

const toggleSource = (source: string) => {
  const index = filters.value.sources.indexOf(source);
  if (index > -1) {
    filters.value.sources.splice(index, 1);
  } else {
    filters.value.sources.push(source);
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
};

const toggleFilters = () => {
  showFilters.value = !showFilters.value;
  emit('search-triggered');
};

const activeFiltersCount = computed(() => {
  let count = filters.value.tags.length + filters.value.sources.length + filters.value['content-types'].length;
  if (filters.value['date-range']) count++;
  return count;
});
</script>

<template>
  <div class="search-filters">
    <button 
      class="filters-toggle glass"
      @click="toggleFilters"
      :class="{ active: showFilters || activeFiltersCount > 0 }"
    >
      🔍 Filters
      <span v-if="activeFiltersCount > 0" class="filter-count">
        {{ activeFiltersCount }}
      </span>
    </button>

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
        <div class="filter-options">
          <label 
            v-for="source in availableSources" 
            :key="source"
            class="filter-option"
          >
            <input 
              type="checkbox" 
              :checked="filters.sources.includes(source)"
              @change="toggleSource(source)"
            />
            <span>{{ source }}</span>
          </label>
        </div>
      </div>

      <div class="filter-section">
        <h4>Tags</h4>
        <div class="filter-options">
          <label 
            v-for="tag in availableTags" 
            :key="tag"
            class="filter-option"
          >
            <input 
              type="checkbox" 
              :checked="filters.tags.includes(tag)"
              @change="toggleTag(tag)"
            />
            <span>{{ tag }}</span>
          </label>
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
.search-filters {
  position: relative;
  flex-shrink: 0;
}

.filters-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  background: transparent;
  border: 1px solid var(--glass-border);
  border-radius: 8px;
  color: white;
  cursor: pointer;
  transition: all 0.3s ease;
  font-size: 0.9rem;
}

.filters-toggle:hover {
  background: var(--glass-bg);
  border-color: var(--primary);
}

.filters-toggle.active {
  background: var(--primary);
  border-color: var(--primary);
}

.filter-count {
  background: rgba(255, 255, 255, 0.2);
  padding: 2px 6px;
  border-radius: 12px;
  font-size: 0.8rem;
  min-width: 20px;
  text-align: center;
}

.filters-panel {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  padding: 20px;
  margin-top: 8px;
  z-index: 1000;
  min-width: 400px;
  text-align: left;
}

.filters-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.filters-header h3 {
  margin: 0;
  font-size: 1rem;
  color: white;
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

/* Custom scrollbar for filters panel */
.filters-panel::-webkit-scrollbar {
  width: 6px;
}

.filters-panel::-webkit-scrollbar-track {
  background: var(--bg-dark);
}

.filters-panel::-webkit-scrollbar-thumb {
  background: var(--glass-border);
  border-radius: 3px;
}

.filters-panel::-webkit-scrollbar-thumb:hover {
  background: var(--text-muted);
}
</style>
