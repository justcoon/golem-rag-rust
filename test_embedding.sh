#!/bin/bash

# Test embedding generation with Ollama
# Usage: ./test_embedding.sh [text_to_embed]

# Load environment variables from .env file with proper expansion
if [ -f .env ]; then
    # Read .env and export variables with proper expansion
    while IFS='=' read -r key value; do
        # Skip comments and empty lines
        [[ $key =~ ^[[:space:]]*# ]] && continue
        [[ -z $key ]] && continue
        
        # Remove surrounding quotes from value if present
        value=$(echo "$value" | sed 's/^"//;s/"$//')
        
        # Expand variables in the value
        value=$(eval echo "$value")
        
        # Export the variable
        export "$key"="$value"
    done < <(grep -v '^#' .env | grep '=')
else
    echo "Warning: .env file not found. Using default values."
fi

OLLAMA_URL="${EMBEDDING_API_BASE:-http://localhost:11434}"
MODEL="${EMBEDDING_MODEL:-nomic-embed-text}"

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
