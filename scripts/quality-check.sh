#!/bin/bash
set -e

echo "🔍 Running quality gates for Golem RAG System..."

echo "📦 Building components..."
golem build

echo "🎨 Checking formatting..."
cargo fmt --all --check

echo "🔍 Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "🧪 Running tests..."
cargo test

echo "✅ All quality gates passed!"
