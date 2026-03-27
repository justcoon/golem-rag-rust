export interface SearchFilters {
  tags: string[];
  sources: string[];
  'content-types': ContentType[];
  'date-range': DateRange | null;
}

export interface DateRange {
  start: string;
  end: string;
}

export enum ContentType {
  Text = 'text',
  Markdown = 'markdown',
  Pdf = 'pdf',
  Html = 'html',
  Json = 'json'
}

export interface HybridSearchConfig {
  semantic_weight: number;
  keyword_weight: number;
  rrf_k: number;
  enable_semantic: boolean;
  enable_keyword: boolean;
}

export interface SearchOptions {
  filters?: SearchFilters | null;
  limit?: number;
  threshold?: number;
  config?: HybridSearchConfig;
}
