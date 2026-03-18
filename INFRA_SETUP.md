# Infrastructure Setup for Golem RAG

This document describes how to set up the required infrastructure for the Golem RAG system using Docker Compose.

## Prerequisites

- Docker and Docker Compose installed
- At least 4GB of available RAM
- Ports 5432 and 9000 available

## Services

### PostgreSQL with pgvector
- **Image**: `pgvector/pgvector:pg18-trixie`
- **Port**: 5432
- **Database**: `golem_rag`
- **User**: `golem_user`
- **Password**: `golem_password`

### RustFS (S3-compatible storage)
- **Image**: `rustfs/rustfs:latest`
- **Port**: 9000
- **Access Key**: `rustfsadmin`
- **Secret Key**: `rustfsadmin123`
- **Bucket**: `golem-documents`
- **Data Directory**: `/data` (persisted in Docker volume)

### Ollama (Embeddings)
- **Image**: `ollama/ollama:latest`
- **Port**: 11434
- **Model**: `nomic-embed-text` (default)
- **Auto-setup**: The `ollama-setup` service automatically pulls the model on startup.

## Quick Start

1. **Start the infrastructure**:
   ```bash
   docker-compose up -d
   ```

2. **Wait for services to be ready** (approximately 30 seconds):
   ```bash
   docker-compose ps
   ```

3. **Configure environment variables**:
   ```bash
   cp .env.example .env
   # Edit .env if needed
   ```

4. **Verify setup**:
   - **RustFS API**: http://localhost:9000
   - **PostgreSQL**: Connect with `psql -h localhost -p 5432 -U golem_user -d golem_rag`

## Directory Structure

```
.
├── docker-compose.yml          # Main compose file
├── .env.example              # Environment variables template
├── migrations/               # Database migrations
│   └── 001_initial_schema.sql
└── INFRA_SETUP.md          # This file
```

## Environment Variables

Copy `.env.example` to `.env` and configure as needed:

```bash
# Database Configuration
DB_URL=postgresql://golem_user:golem_password@localhost:5432/golem_rag

# S3 Configuration (RustFS / AWS)
AWS_ACCESS_KEY_ID=rustfsadmin
AWS_SECRET_ACCESS_KEY=rustfsadmin123
AWS_DEFAULT_REGION=us-east-1
AWS_S3_BUCKET=golem-documents
S3_PORT=9000
S3_ENDPOINT_URL=http://localhost:9000

# Optional: Embedding Configuration
EMBEDDING_MODEL=mock-embedding-v1

# Optional: Log Level
RUST_LOG=info
```

## Testing the Setup

### 1. Test PostgreSQL Connection
```bash
docker-compose exec postgres psql -U golem_user -d golem_rag -c "\dt"
```

### 2. Test RustFS Connection
```bash
# Test basic connectivity
curl -f http://localhost:9000/

# List buckets (if supported by RustFS)
aws s3 ls --endpoint-url http://localhost:9000 \
  --access-key rustfsadmin --secret-key rustfsadmin123

# Upload a test file
echo "Hello RustFS" > test.txt
aws s3 cp test.txt s3://golem-documents/test.txt \
  --endpoint-url http://localhost:9000 \
  --access-key rustfsadmin --secret-key rustfsadmin123
```

### 3. Test S3DocumentLoaderAgent
```bash
# Deploy and test the Golem agent
golem deploy
golem agent invoke 's3-document-loader-agent("test")' 'golem-rust:rag/s3-document-loader-agent.{list-namespace-documents}' '"legal"'
```

## Sample S3 Structure

Upload test documents to verify the namespace mapping:

```
s3://golem-documents/
├── documents/
│   ├── legal/
│   │   ├── contract-2023.pdf
│   │   └── privacy-policy.md
│   ├── technical/
│   │   ├── reports/
│   │   │   └── q4-2023.pdf
│   │   └── guides/
│   │       └── setup-guide.md
│   └── public/
│       └── blog-posts/
│           └── announcement.md
```

## RustFS Specifics

RustFS is a lightweight S3-compatible object storage server written in Rust:

### Advantages
- **Lightweight**: Smaller footprint compared to MinIO
- **Rust-native**: Built with memory safety and performance
- **S3-compatible**: Full AWS S3 API compatibility
- **Simple**: Minimal configuration required

### Configuration
- **Data Directory**: `/data` inside container
- **Port**: 9000 (configurable via `RUSTFS_ADDR`)
- **Access Control**: Simple access key/secret key authentication
- **Persistence**: Docker volume `rustfs_data`

### Limitations
- No web console (unlike MinIO's UI)
- Fewer advanced features (no versioning, lifecycle policies, etc.)
- Simpler bucket management

## Troubleshooting

### Issues with PostgreSQL
- Check if pgvector extension is enabled: `\dx` in psql
- Verify connection string format
- Check Docker logs: `docker-compose logs postgres`

### Issues with RustFS
- Verify service is running: `curl http://localhost:9000/`
- Check Docker logs: `docker-compose logs rustfs`
- Verify credentials in environment variables
- Ensure bucket exists: Check rustfs-setup container logs

### Issues with Ollama
- Verify service is running: `curl http://localhost:11434/api/tags`
- Check setup logs: `docker logs golem-rag-ollama-setup`
- Verify model is pulled: `docker exec golem-rag-ollama ollama list`
- Ensure `OLLAMA_HOST` is correctly set in the environment if accessing from outside Docker.

### Issues with Golem Agent
- Ensure all environment variables are set
- Check that the agent can reach both services
- Verify WASM compilation: `golem build`

## Cleanup

To stop and remove all services:
```bash
docker-compose down -v
```

To remove only containers but keep data:
```bash
docker-compose down
```

## Production Considerations

For production deployment:

1. **Security**: Change default credentials and access keys
2. **Persistence**: Use named volumes or bind mounts for data persistence
3. **Networking**: Consider using Docker networks for isolation
4. **Monitoring**: Add health checks and monitoring
5. **Backup**: Implement regular database and S3 backups
6. **Scaling**: Consider using external managed services for production

## Next Steps

Once infrastructure is running:

1. Deploy the Golem RAG agents: `golem deploy`
2. Test document loading: Use the S3DocumentLoaderAgent
3. Implement remaining agents: EmbeddingGeneratorAgent, SearchAgent, etc.
4. Set up automated workflows for document processing

## RustFS vs MinIO

| Feature | RustFS | MinIO |
|---------|---------|--------|
| Language | Rust | Go |
| Size | ~10MB | ~100MB+ |
| Web UI | No | Yes |
| Console Port | N/A | 9001 |
| Advanced Features | Basic | Full-featured |
| Performance | High | Good |
| Memory Usage | Low | Higher |
| Setup Complexity | Simple | Moderate |
