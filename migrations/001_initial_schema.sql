-- Complete database schema for Golem RAG System
-- This file creates all necessary tables, indexes, and extensions
-- Supports nomic-embed-text (768 dimensions) embeddings

-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Create documents table
CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,                           -- String UUID
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata JSONB NOT NULL,                       -- Serialized DocumentMetadata
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    tags TEXT[] DEFAULT '{}',                     -- Extracted from metadata for indexing
    source TEXT NOT NULL,                         -- Extracted from metadata for indexing
    namespace TEXT NOT NULL,                      -- Extracted from metadata for indexing
    size_bytes BIGINT DEFAULT 0                   -- Extracted from metadata for indexing
);

-- Create document_chunks table
CREATE TABLE IF NOT EXISTS document_chunks (
    id TEXT PRIMARY KEY,                           -- String UUID
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    start_pos INTEGER NOT NULL,
    end_pos INTEGER NOT NULL,
    token_count INTEGER,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create document_embeddings table
CREATE TABLE IF NOT EXISTS document_embeddings (
    id TEXT PRIMARY KEY,                           -- String UUID
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    chunk_text TEXT NOT NULL,
    embedding vector(768),                        -- Nomic-embed-text embedding dimension
    embedding_status TEXT NOT NULL DEFAULT 'not_processed',
    chunk_count INTEGER DEFAULT 0,
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create processing_summaries table
CREATE TABLE IF NOT EXISTS processing_summaries (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace TEXT NOT NULL,
    documents_loaded INTEGER NOT NULL DEFAULT 0,
    embeddings_generated INTEGER NOT NULL DEFAULT 0,
    embeddings_failed INTEGER NOT NULL DEFAULT 0,
    processing_time_ms BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create migration tracking table
CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create indexes for documents table
CREATE INDEX IF NOT EXISTS idx_documents_namespace ON documents(namespace);
CREATE INDEX IF NOT EXISTS idx_documents_source ON documents(source);
CREATE INDEX IF NOT EXISTS idx_documents_created_at ON documents(created_at);
CREATE INDEX IF NOT EXISTS idx_documents_tags ON documents USING GIN(tags);
CREATE INDEX IF NOT EXISTS idx_documents_metadata ON documents USING GIN(metadata);
CREATE INDEX IF NOT EXISTS idx_documents_composite ON documents(namespace, source, created_at);

-- Create indexes for document_chunks table
CREATE INDEX IF NOT EXISTS idx_document_chunks_document_id ON document_chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_document_chunks_chunk_index ON document_chunks(document_id, chunk_index);

-- Create indexes for document_embeddings table
CREATE UNIQUE INDEX IF NOT EXISTS idx_document_embeddings_unique 
    ON document_embeddings(document_id, chunk_index);
CREATE INDEX IF NOT EXISTS idx_document_embeddings_embedding 
    ON document_embeddings USING ivfflat (embedding vector_cosine_ops);
CREATE INDEX IF NOT EXISTS idx_document_embeddings_status ON document_embeddings(embedding_status);
CREATE INDEX IF NOT EXISTS idx_document_embeddings_document_id ON document_embeddings(document_id);
CREATE INDEX IF NOT EXISTS idx_document_embeddings_composite 
    ON document_embeddings(document_id, embedding_status);

-- Create indexes for processing_summaries table
CREATE INDEX IF NOT EXISTS idx_processing_summaries_namespace ON processing_summaries(namespace);
CREATE INDEX IF NOT EXISTS idx_processing_summaries_created_at ON processing_summaries(created_at);

-- Create full-text search indexes
CREATE INDEX IF NOT EXISTS idx_documents_content_fts 
    ON documents USING GIN(to_tsvector('english', content));
CREATE INDEX IF NOT EXISTS idx_document_embeddings_chunk_fts 
    ON document_embeddings USING GIN(to_tsvector('english', chunk_text));

-- Record migration
INSERT INTO schema_migrations (version, applied_at) 
VALUES ('001_initial_schema', NOW()) 
ON CONFLICT (version) DO NOTHING;
