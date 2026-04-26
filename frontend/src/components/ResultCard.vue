<script setup>
import { computed } from 'vue';

const props = defineProps(['result']);
const emit = defineEmits(['preview', 'find-similar']);

const formatScore = (score) => (score * 100).toFixed(1) + '%';

const getMatchTypeLabel = (type) => {
  switch (type) {
    case 'SemanticOnly': return 'Semantic';
    case 'KeywordOnly': return 'Keyword';
    case 'BothMatch': return 'Hybrid';
    default: return type;
  }
};

const highlightedExplanation = computed(() => {
  const explanation = props.result['relevance_explanation'];
  if (!explanation) return '';
  // Replace [keyword] with <strong>keyword</strong> for visual highlighting
  return explanation.replace(/\[(.*?)\]/g, '<strong>$1</strong>');
});

const onFindSimilar = (e) => {
  e.stopPropagation();
  emit('find-similar', props.result.chunk['document_id']);
};
</script>

<template>
  <div class="result-card glass animate-fade-in" @click="emit('preview', result.chunk['document_id'])">
    <div class="result-header">
      <span class="tag" :class="result['match_type'].toLowerCase()">
        {{ getMatchTypeLabel(result['match_type']) }}
      </span>
      <span class="score">Score: {{ formatScore(result['combined_score']) }}</span>
    </div>
    
    <div class="result-content">
      <p v-if="highlightedExplanation" class="explanation">
        <span v-html="highlightedExplanation"></span>
      </p>
      <p v-else class="preview">
        {{ result.chunk.content.substring(0, 200) }}...
      </p>
    </div>

    <div class="result-footer">
      <span class="doc-id">Document: {{ result.chunk['document_id'] }}</span>
      <button 
        class="similar-btn" 
        title="Find Similar Documents"
        @click="onFindSimilar"
      >
        ✨ Similar
      </button>
    </div>
  </div>
</template>

<style scoped>
.result-card {
  padding: 16px;
  cursor: pointer;
  transition: all 0.2s ease;
  text-align: left;
  margin-bottom: 12px;
}

.result-card:hover {
  transform: scale(1.01);
  border-color: var(--primary);
  background: rgba(255, 255, 255, 0.08);
}

.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.tag {
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
}

.tag.semanticonly { background: rgba(168, 85, 247, 0.2); color: #a855f7; }
.tag.keywordonly { background: rgba(59, 130, 246, 0.2); color: #3b82f6; }
.tag.bothmatch { background: rgba(34, 197, 94, 0.2); color: #22c55e; }

.score {
  font-size: 0.85rem;
  color: var(--text-muted);
}

.result-content p {
  font-size: 0.95rem;
  line-height: 1.6;
  color: var(--text-main);
  margin-bottom: 12px;
}

.explanation :deep(strong) {
  color: var(--primary);
  background: rgba(147, 51, 234, 0.1);
  padding: 0 2px;
  border-radius: 2px;
}

.result-footer {
  font-size: 0.8rem;
  color: var(--text-muted);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.similar-btn {
  background: rgba(147, 51, 234, 0.1);
  border: 1px solid rgba(147, 51, 234, 0.3);
  color: #a855f7;
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 0.75rem;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  gap: 4px;
}

.similar-btn:hover {
  background: rgba(147, 51, 234, 0.2);
  border-color: #a855f7;
  transform: translateY(-1px);
}
</style>
