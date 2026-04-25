export interface SearchFilters {
  tags: string[];
  sources: string[];
  content_types: ContentType[];
  date_range: DateRange | null;
}

export interface DateRange {
  start: string;
  end: string;
}

export enum ContentType {
  Text = 'Text',
  Markdown = 'Markdown',
  Pdf = 'Pdf',
  Html = 'Html',
  Json = 'Json'
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
