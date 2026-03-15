#!/bin/bash

# Test embedding generation with Ollama
# Usage: ./test_embedding.sh [text_to_embed]

# Load environment variables from .env file
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
else
    echo "Warning: .env file not found. Using default values."
fi

OLLAMA_URL="http://localhost:${OLLAMA_PORT:-11434}"
MODEL="${OLLAMA_MODEL:-nomic-embed-text}"

# Default text if none provided
TEXT="${1:-"This is a test sentence for embedding generation."}"

echo "Testing embedding generation with Ollama..."
echo "Model: $MODEL"
echo "Text: \"$TEXT\""
echo "URL: $OLLAMA_URL"
echo

# Generate embedding
curl -X POST "$OLLAMA_URL/api/embeddings" \
  -H "Content-Type: application/json" \
  -d "{
    \"model\": \"$MODEL\",
    \"prompt\": \"$TEXT\"
  }" \
  | jq '.'

echo
echo "Embedding generated successfully!"
echo "Embedding dimensions: $(curl -s -X POST "$OLLAMA_URL/api/embeddings" -H "Content-Type: application/json" -d "{\"model\":\"$MODEL\",\"prompt\":\"$TEXT\"}" | jq '.embedding | length')"
