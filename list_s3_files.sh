#!/bin/bash

# List files in RustFS S3 storage
# Usage: ./list_s3_files.sh [bucket]

# Load environment variables from .env file
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
else
    echo "Warning: .env file not found. Using default values."
fi

# Configuration (will be overridden by .env if available)
S3_PORT="${S3_PORT:-9000}"
S3_ENDPOINT="http://localhost:$S3_PORT"
S3_BUCKET="${1:-${AWS_S3_BUCKET:-golem-documents}}"
S3_ACCESS_KEY="${AWS_ACCESS_KEY_ID:-rustfsadmin}"
S3_SECRET_KEY="${AWS_SECRET_ACCESS_KEY:-rustfsadmin123}"
S3_REGION="${AWS_DEFAULT_REGION:-us-east-1}"

echo "Listing files in RustFS S3 storage..."
echo "Endpoint: $S3_ENDPOINT"
echo "Bucket: $S3_BUCKET"
echo

# Configure AWS CLI for RustFS
export AWS_ACCESS_KEY_ID="$S3_ACCESS_KEY"
export AWS_SECRET_ACCESS_KEY="$S3_SECRET_KEY"
export AWS_DEFAULT_REGION="$S3_REGION"
export AWS_ENDPOINT_URL="$S3_ENDPOINT"

# List files
aws s3 ls "s3://$S3_BUCKET" --endpoint-url "$S3_ENDPOINT" --human-readable --recursive
