# Embedding Options for Golem RAG

This document describes the embedding service options available for the Golem RAG system.

## 🚀 **Recommended: Ollama**

Ollama is the recommended choice for local embeddings due to its simplicity and OpenAI-compatible API.

### **Why Ollama?**
- ✅ **OpenAI Compatible**: Drop-in replacement for OpenAI API
- ✅ **Lightweight**: Minimal resource usage
- ✅ **Easy Setup**: Single Docker container
- ✅ **Good Models**: Access to quality embedding models
- ✅ **Local**: No external dependencies or API keys needed

### **Available Embedding Models**

| Model | Dimensions | Size | Performance |
|-------|------------|-------|-------------|
| `nomic-embed-text` | 768 | ~274MB | Excellent |
| `all-minilm` | 384 | ~110MB | Good |
| `mxbai-embed-large` | 1024 | ~670MB | Very Good |

### **Quick Start**

```bash
# Start infrastructure with Ollama
docker-compose up -d

# Wait for model to download (check logs)
docker-compose logs ollama-setup

# Test embedding API
curl -X POST http://localhost:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer ollama" \
  -d '{
    "model": "nomic-embed-text",
    "input": "Hello, world!"
  }'
```

## 🔧 **Configuration**

The system automatically detects the embedding provider from environment variables:

```bash
# Ollama Configuration
EMBEDDING_MODEL=nomic-embed-text
EMBEDDING_API_BASE=http://localhost:11434/v1
EMBEDDING_API_KEY=ollama
```

## 📊 **Model Comparison**

### **nomic-embed-text** (Recommended)
- **Dimensions**: 768
- **Size**: 274MB
- **Use Case**: General purpose, good balance of quality and size
- **Performance**: Fast inference, good quality

### **all-minilm**
- **Dimensions**: 384
- **Size**: 110MB
- **Use Case**: Lightweight applications, faster processing
- **Performance**: Very fast, decent quality

### **mxbai-embed-large**
- **Dimensions**: 1024
- **Size**: 670MB
- **Use Case**: High-quality requirements
- **Performance**: Slower but higher quality

## 🐳 **Docker Services**

### **Ollama Service**
```yaml
ollama:
  image: ollama/ollama:latest
  container_name: golem-rag-ollama
  ports:
    - "11434:11434"
  volumes:
    - ollama_data:/root/.ollama
  environment:
    - OLLAMA_HOST=0.0.0.0
  networks:
    - golem-rag-network
```

### **Model Download Service**
```yaml
ollama-setup:
  image: ollama/ollama:latest
  container_name: golem-rag-ollama-setup
  depends_on:
    ollama:
      condition: service_healthy
  entrypoint: >
    /bin/sh -c "
    ollama pull nomic-embed-text &&
    echo 'Embedding model downloaded successfully'
    "
```

## 🔌 **Integration with Golem Agents**

The embedding client is designed to work seamlessly with Golem agents:

```rust
use common_lib::*;

// Auto-detect and create embedding client
let (embedding_client, provider) = EmbeddingClient::from_env()?;

// Generate embedding
let embedding = embedding_client
    .generate_embedding_with_fallback("Your text here", &provider)
    .await?;
```

## 🛠 **Alternative Options**

### **1. LocalAI**
```yaml
localai:
  image: localai/localai:latest
  ports:
    - "8080:8080"
  environment:
    - MODELS_PATH=/build/models
```

**Pros**: Feature rich, supports multiple models
**Cons**: More complex setup, larger resource usage

### **2. Text-Generation-WebUI**
```yaml
textgen:
  image: ghcr.io/huggingface/text-generation-inference:latest
  ports:
    - "8080:80"
  environment:
    - MODEL_ID=sentence-transformers/all-MiniLM-L6-v2
    - TASK=feature-extraction
```

**Pros**: HuggingFace integration
**Cons**: More complex, specific to transformers

### **3. OpenAI API (Cloud)**
```bash
EMBEDDING_API_BASE=https://api.openai.com/v1
EMBEDDING_API_KEY=your-openai-api-key
EMBEDDING_MODEL=text-embedding-3-small
```

**Pros**: High quality, no setup
**Cons**: Costs money, requires internet

## 📈 **Performance Considerations**

### **Resource Requirements**

| Model | RAM Usage | CPU | Storage |
|--------|------------|------|---------|
| nomic-embed-text | ~500MB | Low | 274MB |
| all-minilm | ~200MB | Very Low | 110MB |
| mxbai-embed-large | ~1GB | Medium | 670MB |

### **Performance Tips**

1. **Choose Right Model**: `nomic-embed-text` for most use cases
2. **Batch Requests**: Process multiple texts together when possible
3. **Cache Results**: Store embeddings for repeated content
4. **Dimension Trade-off**: Lower dimensions = faster but less detailed

## 🔍 **Testing Embeddings**

### **Test API**
```bash
# Simple test
curl -X POST http://localhost:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer ollama" \
  -d '{"model": "nomic-embed-text", "input": "test"}'

# Check model availability
curl http://localhost:11434/api/tags
```

### **Test with Golem Agent**
```bash
# Deploy agent
golem deploy

# Test embedding generation
golem agent invoke 'embedding-agent("test")' 'golem-rust:rag/embedding-agent.{generate-embedding}' '"Hello, world!"'
```

## 🚨 **Troubleshooting**

### **Common Issues**

1. **Model Not Downloaded**
   ```bash
   # Check logs
   docker-compose logs ollama-setup
   
   # Manual download
   docker-compose exec ollama ollama pull nomic-embed-text
   ```

2. **API Not Responding**
   ```bash
   # Check service health
   curl -f http://localhost:11434/api/tags
   
   # Restart service
   docker-compose restart ollama
   ```

3. **Memory Issues**
   - Use smaller model: `all-minilm`
   - Increase Docker memory limits
   - Check system RAM usage

4. **Network Issues**
   - Verify port 11434 is available
   - Check firewall settings
   - Ensure Docker network is working

### **Debug Mode**

Enable verbose logging:
```bash
# Add to environment
RUST_LOG=debug

# Check detailed logs
docker-compose logs -f ollama
```

## 📚 **Next Steps**

1. **Start Infrastructure**: `docker-compose up -d`
2. **Verify Setup**: Test embedding API
3. **Configure Agents**: Set environment variables
4. **Deploy Golem Agents**: `golem deploy`
5. **Test Integration**: Verify document processing pipeline

## 🎯 **Recommendations**

For **development**:
- Use `nomic-embed-text` for best balance
- Enable mock fallback for testing
- Use local Docker volumes

For **production**:
- Consider `mxbai-embed-large` for higher quality
- Implement proper monitoring
- Use external managed services if scaling needed
- Set up backup for model files
