.PHONY: help build fmt lint test clean quality-check pre-push

# Default target
help:
	@echo "Golem RAG System - Development Commands"
	@echo ""
	@echo "Available targets:"
	@echo "  build          Build all components"
	@echo "  fmt            Format code"
	@echo "  lint           Run clippy linting"
	@echo "  test           Run tests"
	@echo "  quality-check  Run all quality gates (build, fmt, lint, test)"
	@echo "  pre-push       Complete pre-push validation"
	@echo "  clean          Clean build artifacts"
	@echo ""
	@echo "Feature Implementation Workflow:"
	@echo "  1. Plan your feature (see docs/feature-implementation-workflow.md)"
	@echo "  2. Get confirmation for your plan"
	@echo "  3. Implement and run 'make quality-check'"

# Build all components
build:
	@echo "📦 Building Golem components..."
	golem build

# Format code
fmt:
	@echo "🎨 Formatting code..."
	cargo fmt --all

# Check formatting
fmt-check:
	@echo "🎨 Checking formatting..."
	cargo fmt --all --check

# Run linting
lint:
	@echo "🔍 Running clippy..."
	cargo clippy --all-targets --all-features -- -D warnings

# Run tests
test:
	@echo "🧪 Running tests..."
	cargo test

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf target/

# Complete quality gate check
quality-check: build fmt-check lint test
	@echo "✅ All quality gates passed!"

# Pre-push validation (more thorough)
pre-push: quality-check
	@echo "🚀 Pre-push validation complete!"

# Install git hooks (run once)
install-hooks:
	@echo "🔧 Installing git hooks..."
	@if [ -d .git ]; then \
		cp scripts/pre-commit .git/hooks/; \
		chmod +x .git/hooks/pre-commit; \
		echo "✅ Git hooks installed"; \
	else \
		echo "❌ Not a git repository"; \
	fi
