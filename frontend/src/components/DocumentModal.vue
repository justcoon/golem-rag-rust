<script setup>
import { computed } from 'vue';
import { marked } from 'marked';

const props = defineProps(['document', 'show']);
const emit = defineEmits(['close']);

const renderedContent = computed(() => {
  if (!props.document?.content) return '';
  return marked(props.document.content);
});

const formatDate = (dateStr) => {
  if (!dateStr) return 'N/A';
  return new Date(dateStr).toLocaleString();
};
</script>

<template>
  <div v-if="show" class="modal-overlay" @click.self="emit('close')">
    <div class="modal-content glass animate-fade-in">
      <div class="modal-header">
        <div class="header-main">
          <h2>{{ document.title || 'Document Preview' }}</h2>
          <span class="doc-id">{{ document.id }}</span>
        </div>
        <button class="close-btn" @click="emit('close')">&times;</button>
      </div>

      <div class="modal-body">
        <div class="metadata-grid">
          <div class="meta-item">
            <span class="label">Source:</span>
            <span class="value">{{ document.source }}</span>
          </div>
          <div class="meta-item">
            <span class="label">Namespace:</span>
            <span class="value">{{ document.namespace }}</span>
          </div>
          <div class="meta-item">
            <span class="label">Content Type:</span>
            <span class="value">{{ document.metadata?.['content_type'] }}</span>
          </div>
          <div class="meta-item">
            <span class="label">Size:</span>
            <span class="value">{{ (document['size_bytes'] / 1024).toFixed(1) }} KB</span>
          </div>
          <div class="meta-item">
            <span class="label">Updated:</span>
            <span class="value">{{ formatDate(document['updated_at']) }}</span>
          </div>
          <div class="meta-item tags">
            <span class="label">Tags:</span>
            <div class="tag-list">
              <span v-for="tag in document.tags" :key="tag" class="small-tag">{{ tag }}</span>
            </div>
          </div>
        </div>

        <div class="content-preview markdown-body" v-html="renderedContent"></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(4px);
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.modal-content {
  width: 100%;
  max-width: 900px;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
}

.modal-header {
  padding: 24px;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
}

.header-main h2 {
  font-size: 1.5rem;
  margin-bottom: 4px;
}

.doc-id {
  font-family: monospace;
  font-size: 0.8rem;
  color: var(--text-muted);
}

.close-btn {
  background: transparent;
  border: none;
  font-size: 2rem;
  color: var(--text-muted);
  line-height: 1;
  padding: 0;
  min-width: unset;
}

.close-btn:hover {
  color: white;
  transform: none;
  box-shadow: none;
}

.modal-body {
  padding: 24px;
  overflow-y: auto;
  flex: 1;
}

.metadata-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 16px;
  margin-bottom: 32px;
  padding: 16px;
  background: rgba(255, 255, 255, 0.03);
  border-radius: 12px;
}

.meta-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.meta-item.tags {
  grid-column: 1 / -1;
}

.label {
  font-size: 0.75rem;
  text-transform: uppercase;
  color: var(--text-muted);
  font-weight: 600;
}

.value {
  font-size: 0.9rem;
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.small-tag {
  font-size: 0.7rem;
  padding: 2px 8px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 999px;
  color: var(--text-muted);
}

.content-preview {
  line-height: 1.7;
  color: var(--text-main);
}

.markdown-body :deep(h1), .markdown-body :deep(h2) {
  margin-top: 1.5em;
  margin-bottom: 0.5em;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 0.3em;
}

.markdown-body :deep(p) { margin-bottom: 1em; }
.markdown-body :deep(code) {
  background: rgba(255, 255, 255, 0.1);
  padding: 2px 4px;
  border-radius: 4px;
  font-family: monospace;
}

.markdown-body :deep(pre) {
  background: rgba(0, 0, 0, 0.3);
  padding: 16px;
  border-radius: 8px;
  overflow-x: auto;
  margin-bottom: 1em;
}
</style>
