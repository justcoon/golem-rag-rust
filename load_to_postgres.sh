#!/bin/bash

# Load documents from data folder to PostgreSQL
# Usage: ./load_to_postgres.sh [data_directory]

set -e

# Load environment variables from .env file
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
else
    echo "Error: .env file not found. Please create one from .env.example"
    exit 1
fi

# Configuration (will be overridden by .env if available)
POSTGRES_HOST="${POSTGRES_HOST:-localhost}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_DB="${POSTGRES_DB:-golem_rag}"
POSTGRES_USER="${POSTGRES_USER:-golem_user}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-golem_password}"

# Data directory (default: data or from .env)
DATA_DIR="${1:-${DATA_DIR:-data}}"

echo "Loading documents to PostgreSQL..."
echo "Host: $POSTGRES_HOST:$POSTGRES_PORT"
echo "Database: $POSTGRES_DB"
echo "User: $POSTGRES_USER"
echo "Data Directory: $DATA_DIR"
echo

# Check if data directory exists
if [ ! -d "$DATA_DIR" ]; then
    echo "Error: Data directory '$DATA_DIR' does not exist"
    exit 1
fi

# Check if psql is installed and set the correct path
PSQL_CMD=""
if command -v psql &> /dev/null; then
    PSQL_CMD="psql"
elif [ -f "/usr/local/opt/libpq/bin/psql" ]; then
    PSQL_CMD="/usr/local/opt/libpq/bin/psql"
else
    echo "Error: psql is not installed"
    echo "Install with: brew install postgresql or brew install libpq"
    exit 1
fi

echo "Using psql command: $PSQL_CMD"

# Wait for PostgreSQL to be ready
echo "Checking PostgreSQL connectivity..."
while ! PGPASSWORD="$POSTGRES_PASSWORD" "$PSQL_CMD" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "SELECT 1;" &> /dev/null; do
    echo "Waiting for PostgreSQL to be ready..."
    sleep 2
done
echo "PostgreSQL is ready!"

# Function to generate consistent document ID from file path
generate_doc_id() {
    local file_path="$1"
    # Use MD5 hash of file path to create consistent ID
    echo -n "$file_path" | md5sum | cut -d' ' -f1
}

# Function to get file size in bytes
get_file_size() {
    stat -f%z "$1" 2>/dev/null || stat -c%s "$1" 2>/dev/null || echo 0
}

# Function to get file modification time
get_file_mtime() {
    stat -f%m "$1" 2>/dev/null || stat -c%Y "$1" 2>/dev/null || echo $(date +%s)
}

# Load documents
echo "Loading documents..."
file_count=0
success_count=0

for file_path in "$DATA_DIR"/*; do
    if [ -f "$file_path" ]; then
        filename=$(basename "$file_path")
        echo "Processing: $filename"
        
        # Generate document ID and metadata
        doc_id=$(generate_doc_id "$file_path")
        file_size=$(get_file_size "$file_path")
        file_mtime=$(get_file_mtime "$file_path")
        file_ext="${filename##*.}"
        
        # Read file content
        content=$(cat "$file_path")
        
        # Create metadata JSON
        # Map file extensions to proper content types
        case "$file_ext" in
            md|markdown)
                content_type="Markdown"
                ;;
            txt|text)
                content_type="Text"
                ;;
            pdf)
                content_type="Pdf"
                ;;
            html|htm)
                content_type="Html"
                ;;
            json)
                content_type="Json"
                ;;
            *)
                content_type="Text"  # Default fallback
                ;;
        esac
        
        # Get current timestamp in ISO format
        current_time=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
        
        # Create metadata JSON with all required fields
        metadata=$(cat <<EOF
{
    "filename": "$filename",
    "filepath": "$file_path",
    "content_type": "$content_type",
    "created_at": "$current_time",
    "updated_at": "$current_time",
    "source_metadata": {},
    "metadata": {}
}
EOF
)
        
        # Get current timestamp
        current_time=$(date -u +"%Y-%m-%d %H:%M:%S+00")
        
        # Insert document into database (UPSERT - update if exists, insert if not)
        # Escape single quotes in content and metadata for SQL
        escaped_content=$(echo "$content" | sed "s/'/''/g")
        escaped_metadata=$(echo "$metadata" | sed "s/'/''/g")
        
        insert_query="INSERT INTO documents (
    id, title, content, metadata, created_at, updated_at, 
    tags, source, namespace, size_bytes
) VALUES (
    '$doc_id',
    '$filename',
    '$$${escaped_content}$$',
    '$escaped_metadata',
    '$current_time',
    '$current_time',
    ARRAY['file', '$file_ext'],
    'filesystem',
    'default',
    $file_size
) ON CONFLICT (id) DO UPDATE SET
    title = EXCLUDED.title,
    content = EXCLUDED.content,
    metadata = EXCLUDED.metadata,
    updated_at = EXCLUDED.updated_at,
    size_bytes = EXCLUDED.size_bytes;"
        
        if PGPASSWORD="$POSTGRES_PASSWORD" "$PSQL_CMD" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "$insert_query" &> /dev/null; then
            echo "✓ Successfully loaded: $filename (ID: $doc_id)"
            ((success_count++))
        else
            echo "✗ Failed to load: $filename"
        fi
        ((file_count++))
    fi
done

echo
echo "Loading Summary:"
echo "Total files: $file_count"
echo "Successfully loaded: $success_count"
echo "Failed loads: $((file_count - success_count))"

# Show database statistics
echo
echo "Database Statistics:"
PGPASSWORD="$POSTGRES_PASSWORD" "$PSQL_CMD" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "
SELECT 
    COUNT(*) as total_documents,
    COUNT(DISTINCT source) as unique_sources,
    COUNT(DISTINCT namespace) as unique_namespaces,
    SUM(size_bytes) as total_size_bytes
FROM documents;
"

# Show recent documents
echo
echo "Recent Documents:"
PGPASSWORD="$POSTGRES_PASSWORD" "$PSQL_CMD" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "
SELECT 
    id,
    title,
    source,
    namespace,
    size_bytes,
    created_at
FROM documents 
ORDER BY created_at DESC 
LIMIT 5;
"

if [ $success_count -eq $file_count ]; then
    echo "All documents loaded successfully!"
    exit 0
else
    echo "Some documents failed to load. Check the logs above."
    exit 1
fi
