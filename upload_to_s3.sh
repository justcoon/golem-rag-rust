#!/bin/bash

# Upload script for RustFS S3-compatible storage
# Usage: ./upload_to_s3.sh [data_directory] [namespace]

# Load environment variables from .env file
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
else
    echo "Warning: .env file not found. Using default values."
fi

# Configuration (will be overridden by .env if available)
S3_PORT="${S3_PORT:-9000}"
S3_ENDPOINT="http://localhost:$S3_PORT"
S3_BUCKET="${AWS_S3_BUCKET:-golem-documents}"
S3_ACCESS_KEY="${AWS_ACCESS_KEY_ID:-rustfsadmin}"
S3_SECRET_KEY="${AWS_SECRET_ACCESS_KEY:-rustfsadmin123}"
S3_REGION="${AWS_DEFAULT_REGION:-us-east-1}"

# Data directory (default: data or from .env)
DATA_DIR="${1:-${DATA_DIR:-data}}"
NAMESPACE="${2}"

echo "Uploading files to RustFS S3 storage..."
echo "Endpoint: $S3_ENDPOINT"
echo "Bucket: $S3_BUCKET"
echo "Data Directory: $DATA_DIR"
if [ -n "$NAMESPACE" ]; then
    echo "Namespace: $NAMESPACE"
fi
echo

# Check if data directory exists
if [ ! -d "$DATA_DIR" ]; then
    echo "Error: Data directory '$DATA_DIR' does not exist"
    exit 1
fi

# Check if AWS CLI is installed
if ! command -v aws &> /dev/null; then
    echo "Error: AWS CLI is not installed"
    echo "Install with: brew install awscli"
    exit 1
fi

# Wait for RustFS to be ready
echo "Checking RustFS connectivity..."
while ! curl -s "$S3_ENDPOINT/health" > /dev/null; do
    echo "Waiting for RustFS to be ready..."
    sleep 2
done
echo "RustFS is ready!"

# Configure AWS CLI for RustFS
export AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY"
export AWS_SECRET_ACCESS_KEY="$S3_SECRET_KEY"
export AWS_DEFAULT_REGION="$S3_REGION"
export AWS_ENDPOINT_URL="$S3_ENDPOINT"

# Check if bucket exists, create if not
echo "Checking bucket..."
if ! aws s3 ls "s3://$S3_BUCKET" --endpoint-url "$S3_ENDPOINT" &> /dev/null; then
    echo "Creating bucket: $S3_BUCKET"
    aws s3 mb "s3://$S3_BUCKET" --endpoint-url "$S3_ENDPOINT" --region "$S3_REGION"
fi

# Upload files
echo "Uploading files..."
file_count=0
success_count=0

for file in "$DATA_DIR"/*; do
    if [ -f "$file" ]; then
        filename=$(basename "$file")
        
        # Determine upload path
        if [ -n "$NAMESPACE" ]; then
            DEST_PATH="s3://$S3_BUCKET/$NAMESPACE/$filename"
        else
            DEST_PATH="s3://$S3_BUCKET/$filename"
        fi
        
        echo "Uploading: $filename to $DEST_PATH"
        
        if aws s3 cp "$file" "$DEST_PATH" --endpoint-url "$S3_ENDPOINT"; then
            echo "✓ Successfully uploaded: $filename"
            ((success_count++))
        else
            echo "✗ Failed to upload: $filename"
        fi
        ((file_count++))
    fi
done

echo
echo "Upload Summary:"
echo "Total files: $file_count"
echo "Successfully uploaded: $success_count"
echo "Failed uploads: $((file_count - success_count))"

# List uploaded files
echo
echo "Files in bucket:"
aws s3 ls "s3://$S3_BUCKET" --endpoint-url "$S3_ENDPOINT" --human-readable --recursive

if [ $success_count -eq $file_count ]; then
    echo "All files uploaded successfully!"
    exit 0
else
    echo "Some files failed to upload. Check the logs above."
    exit 1
fi
