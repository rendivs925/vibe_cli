# Vibe CLI - Production-Ready Makefile
# ==================================
#
# This Makefile provides comprehensive commands for development, testing,
# building, deployment, and maintenance of the Vibe CLI application.
#
# Quick Start:
#   make setup    - Install dependencies and setup development environment
#   make build    - Build the application in release mode
#   make test     - Run all tests
#   make run      - Run the application
#
# For full documentation, see docs/DEVELOPMENT.md

.PHONY: help setup build test run clean install lint format docs check-all release deploy monitor logs backup restore health-check performance-test security-audit

# Default target
help: ## Show this help message
	@echo "🚀 Vibe CLI - Production-Ready Makefile"
	@echo "========================================"
	@echo ""
	@echo "Available commands:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-20s %s\n", $$1, $$2}'
	@echo ""
	@echo "Quick start:"
	@echo "  make setup    - Setup development environment"
	@echo "  make build    - Build in release mode"
	@echo "  make test     - Run all tests"
	@echo "  make run      - Run the application"
	@echo ""
	@echo "For detailed documentation, run: make docs"

# =============================================================================
# DEVELOPMENT ENVIRONMENT SETUP
# =============================================================================

setup: ## Install dependencies and setup development environment
	@echo "🔧 Setting up development environment..."
	@echo "========================================"

	# Check for required tools
	@command -v cargo >/dev/null 2>&1 || { echo "❌ Cargo is required but not installed. Please install Rust."; exit 1; }
	@command -v rustc >/dev/null 2>&1 || { echo "❌ Rust compiler is required but not installed."; exit 1; }

	@echo "✅ Rust toolchain detected"

	# Setup Rust components
	@echo "📦 Installing Rust components..."
	rustup component add clippy
	rustup component add rustfmt
	rustup component add llvm-tools-preview

	# Install development tools
	@echo "🛠️ Installing development tools..."
	cargo install cargo-audit --quiet
	cargo install cargo-tarpaulin --quiet
	cargo install cargo-udeps --quiet
	cargo install cargo-outdated --quiet

	# Setup pre-commit hooks (if git is available)
	@if command -v git >/dev/null 2>&1 && [ -d .git ]; then \
		echo "🔗 Setting up git hooks..."; \
		mkdir -p .git/hooks; \
		echo "#!/bin/sh\nmake pre-commit" > .git/hooks/pre-commit; \
		chmod +x .git/hooks/pre-commit; \
		echo "✅ Git hooks configured"; \
	else \
		echo "⚠️ Git not available or not a git repository"; \
	fi

	@echo "🎉 Development environment setup complete!"
	@echo ""
	@echo "Next steps:"
	@echo "  1. make build    - Build the project"
	@echo "  2. make test     - Run tests"
	@echo "  3. make run      - Start the application"

setup-ollama: ## Setup Ollama for AI features
	@echo "🤖 Setting up Ollama..."
	@echo "========================"

	# Check if Ollama is installed
	@if ! command -v ollama >/dev/null 2>&1; then \
		echo "❌ Ollama is required for AI features."; \
		echo "Please install Ollama from: https://ollama.ai"; \
		exit 1; \
	fi

	@echo "✅ Ollama detected"

	# Start Ollama service
	@echo "🚀 Starting Ollama service..."
	ollama serve &
	sleep 2

	# Pull required models
	@echo "📥 Pulling required models..."
	ollama pull qwen2.5:1.5b-instruct || echo "⚠️ Failed to pull model, you may need to run this manually"

	@echo "🎉 Ollama setup complete!"

# =============================================================================
# BUILDING
# =============================================================================

build: ## Build the application in release mode
	@echo "🔨 Building Vibe CLI (release mode)..."
	@echo "======================================"
	RUSTFLAGS="-C target-cpu=native" cargo build --release
	@echo "✅ Build complete: target/release/vibe_cli"

build-debug: ## Build the application in debug mode
	@echo "🔨 Building Vibe CLI (debug mode)..."
	@echo "===================================="
	cargo build
	@echo "✅ Build complete: target/debug/vibe_cli"

build-all: ## Build all targets and check for errors
	@echo "🔨 Building all targets..."
	@echo "=========================="
	cargo build --all-targets
	cargo build --all-targets --release
	@echo "✅ All builds successful"

# =============================================================================
# TESTING
# =============================================================================

test: ## Run all tests
	@echo "🧪 Running test suite..."
	@echo "========================"
	cargo test --workspace --verbose

test-unit: ## Run unit tests only
	@echo "🧪 Running unit tests..."
	@echo "======================="
	cargo test --lib --verbose

test-integration: ## Run integration tests only
	@echo "🧪 Running integration tests..."
	@echo "==============================="
	cargo test --test '*' --verbose

test-security: ## Run security-focused tests
	@echo "🔒 Running security tests..."
	@echo "============================"
	cargo test security --verbose
	cargo test sandbox --verbose
	cargo test dangerous --verbose

test-performance: ## Run performance tests
	@echo "⚡ Running performance tests..."
	@echo "==============================="
	cargo test performance --verbose --release
	cargo test benchmark --verbose --release

test-coverage: ## Generate test coverage report
	@echo "📊 Generating test coverage..."
	@echo "=============================="
	cargo tarpaulin --out Html --output-dir coverage/
	@echo "✅ Coverage report generated: coverage/tarpaulin-report.html"

test-watch: ## Run tests in watch mode (requires cargo-watch)
	@echo "👀 Running tests in watch mode..."
	@echo "================================="
	cargo watch -x test

# =============================================================================
# CODE QUALITY
# =============================================================================

lint: ## Run linting checks
	@echo "🔍 Running linters..."
	@echo "===================="
	cargo clippy -- -D warnings
	@echo "✅ Linting complete"

format: ## Format code
	@echo "💅 Formatting code..."
	@echo "===================="
	cargo fmt
	@echo "✅ Code formatted"

format-check: ## Check code formatting
	@echo "🔍 Checking code formatting..."
	@echo "=============================="
	cargo fmt --check

check-all: ## Run all code quality checks
	@echo "🔬 Running comprehensive code quality checks..."
	@echo "==============================================="
	cargo check
	cargo clippy -- -D warnings
	cargo fmt --check

	# Check for unused dependencies
	@echo "🔍 Checking for unused dependencies..."
	cargo +nightly udeps

	# Security audit
	@echo "🔒 Running security audit..."
	cargo audit

	@echo "✅ All quality checks passed"

# =============================================================================
# RUNNING
# =============================================================================

run: ## Run the application
	@echo "🚀 Starting Vibe CLI..."
	@echo "======================="
	cargo run --release

run-debug: ## Run the application in debug mode
	@echo "🚀 Starting Vibe CLI (debug mode)..."
	@echo "===================================="
	cargo run

run-with-env: ## Run with environment file
	@echo "🚀 Starting Vibe CLI with environment..."
	@echo "========================================="
	cargo run --release -- --env-file .env

# =============================================================================
# DEVELOPMENT WORKFLOW
# =============================================================================

dev: ## Start development workflow (build + test + run)
	@echo "🔄 Starting development workflow..."
	@echo "==================================="
	make build
	make test
	make run

watch: ## Watch for changes and rebuild automatically
	@echo "👀 Watching for changes..."
	@echo "=========================="
	cargo watch -x 'build --release'

pre-commit: ## Run pre-commit checks
	@echo "🔒 Running pre-commit checks..."
	@echo "==============================="
	make format-check
	make lint
	make test-unit
	@echo "✅ Pre-commit checks passed"

# =============================================================================
# DOCUMENTATION
# =============================================================================

docs: ## Generate documentation
	@echo "📚 Generating documentation..."
	@echo "=============================="
	cargo doc --open --no-deps
	@echo "✅ Documentation generated"

docs-serve: ## Serve documentation locally
	@echo "🌐 Serving documentation..."
	@echo "==========================="
	cargo doc --open --no-deps &
	@echo "✅ Documentation server started"

# =============================================================================
# CLEANING
# =============================================================================

clean: ## Clean build artifacts
	@echo "🧹 Cleaning build artifacts..."
	@echo "==============================="
	cargo clean
	rm -rf target/
	rm -rf coverage/
	rm -f *.profraw
	@echo "✅ Clean complete"

clean-all: ## Clean everything including caches
	@echo "🧹 Deep cleaning..."
	@echo "=================="
	cargo clean
	rm -rf target/
	rm -rf coverage/
	rm -rf .cargo/
	rm -f Cargo.lock
	rm -f *.profraw
	@echo "✅ Deep clean complete"

# =============================================================================
# INSTALLATION & DEPLOYMENT
# =============================================================================

install: ## Install the application system-wide
	@echo "📦 Installing Vibe CLI..."
	@echo "========================="
	make build
	sudo cp target/release/vibe_cli /usr/local/bin/vibe_cli
	sudo chmod +x /usr/local/bin/vibe_cli
	@echo "✅ Installation complete"

uninstall: ## Uninstall the application
	@echo "🗑️ Uninstalling Vibe CLI..."
	@echo "==========================="
	sudo rm -f /usr/local/bin/vibe_cli
	@echo "✅ Uninstallation complete"

release: ## Create a release build with optimizations
	@echo "📦 Creating release build..."
	@echo "============================"
	RUSTFLAGS="-C target-cpu=native -C link-arg=-fuse-ld=lld -C opt-level=3" cargo build --release
	strip target/release/vibe_cli

	@echo "📊 Release statistics:"
	@ls -lh target/release/vibe_cli
	@file target/release/vibe_cli

release-deb: ## Create Debian package
	@echo "📦 Creating Debian package..."
	@echo "=============================="
	@if command -v cargo-deb >/dev/null 2>&1; then \
		cargo deb; \
		echo "✅ Debian package created"; \
	else \
		echo "❌ cargo-deb not installed. Install with: cargo install cargo-deb"; \
		exit 1; \
	fi

# =============================================================================
# MONITORING & MAINTENANCE
# =============================================================================

monitor: ## Start monitoring the application
	@echo "📊 Starting application monitoring..."
	@echo "===================================="
	@if command -v htop >/dev/null 2>&1; then \
		htop; \
	else \
		top; \
	fi

logs: ## Show application logs
	@echo "📋 Showing application logs..."
	@echo "=============================="
	# Check common log locations
	@if [ -f /var/log/vibe_cli.log ]; then \
		tail -f /var/log/vibe_cli.log; \
	elif [ -f ~/.local/share/vibe_cli/logs/app.log ]; then \
		tail -f ~/.local/share/vibe_cli/logs/app.log; \
	else \
		echo "No log files found. Logs may be written to stdout/stderr."; \
		echo "Try running with: RUST_LOG=debug make run"; \
	fi

health-check: ## Run health checks
	@echo "🏥 Running health checks..."
	@echo "==========================="

	# Check if binary exists and is executable
	@if [ -x target/release/vibe_cli ]; then \
		echo "✅ Binary exists and is executable"; \
	else \
		echo "❌ Binary not found or not executable"; \
		exit 1; \
	fi

	# Check dependencies
	@echo "🔍 Checking dependencies..."
	cargo check --quiet && echo "✅ Dependencies OK" || echo "❌ Dependency issues found"

	# Quick test run
	@echo "🧪 Running quick functionality test..."
	timeout 5s target/release/vibe_cli --help >/dev/null 2>&1 && echo "✅ Basic functionality OK" || echo "❌ Basic functionality failed"

	# Memory and performance check
	@echo "⚡ Running performance check..."
	/usr/bin/time -f "Memory usage: %M KB\nTime: %e seconds" target/release/vibe_cli --help >/dev/null 2>&1

	@echo "🎉 Health check complete!"

performance-test: ## Run performance benchmarks
	@echo "⚡ Running performance benchmarks..."
	@echo "===================================="
	cargo build --release --quiet

	@echo "📊 Benchmark results:"
	@echo "===================="

	# Simple performance test
	@echo "Testing startup time..."
	time -p timeout 1s target/release/vibe_cli --help >/dev/null 2>&1

	@echo "Testing memory usage..."
	/usr/bin/time -f "Peak memory: %M KB" target/release/vibe_cli --help >/dev/null 2>&1

	# If criterion is available, run benchmarks
	@if cargo bench --help >/dev/null 2>&1; then \
		echo "Running criterion benchmarks..."; \
		cargo bench; \
	else \
		echo "Criterion benchmarks not available"; \
	fi

security-audit: ## Run security audit
	@echo "🔒 Running security audit..."
	@echo "============================"
	cargo audit

	@echo "🔍 Checking for unsafe code..."
	cargo clippy -- -W clippy::pedantic -W clippy::nursery 2>&1 | grep -i "unsafe\|vulnerability" || echo "✅ No unsafe code issues found"

	@echo "🔐 Security audit complete"

backup: ## Create backup of project and data
	@echo "💾 Creating backup..."
	@echo "===================="
	BACKUP_DIR="backup_$(date +%Y%m%d_%H%M%S)"
	mkdir -p "$BACKUP_DIR"

	# Backup source code
	cp -r src/ "$BACKUP_DIR/"
	cp -r infrastructure/ "$BACKUP_DIR/"
	cp -r domain/ "$BACKUP_DIR/"
	cp -r shared/ "$BACKUP_DIR/"
	cp -r tests/ "$BACKUP_DIR/"

	# Backup configuration files
	cp Cargo.toml Cargo.lock "$BACKUP_DIR/" 2>/dev/null || true
	cp .gitignore .env* "$BACKUP_DIR/" 2>/dev/null || true

	# Backup data directory if it exists
	if [ -d ~/.local/share/vibe_cli ]; then \
		cp -r ~/.local/share/vibe_cli "$BACKUP_DIR/user_data"; \
	fi

	# Create compressed archive
	tar -czf "${BACKUP_DIR}.tar.gz" "$BACKUP_DIR"
	rm -rf "$BACKUP_DIR"

	@echo "✅ Backup created: ${BACKUP_DIR}.tar.gz"

restore: ## Restore from backup
	@echo "🔄 Restoring from backup..."
	@echo "==========================="
	@if [ -z "$(BACKUP_FILE)" ]; then \
		echo "❌ Please specify BACKUP_FILE variable"; \
		echo "Usage: make restore BACKUP_FILE=backup_20231201.tar.gz"; \
		exit 1; \
	fi

	@if [ ! -f "$(BACKUP_FILE)" ]; then \
		echo "❌ Backup file not found: $(BACKUP_FILE)"; \
		exit 1; \
	fi

	@echo "Extracting backup..."
	tar -xzf "$(BACKUP_FILE)"

	BACKUP_DIR=$$(basename "$(BACKUP_FILE)" .tar.gz)
	if [ -d "$$BACKUP_DIR" ]; then \
		cp -r "$$BACKUP_DIR"/* ./; \
		rm -rf "$$BACKUP_DIR"; \
		echo "✅ Restore complete"; \
	else \
		echo "❌ Invalid backup format"; \
		exit 1; \
	fi

update-deps: ## Update all dependencies
	@echo "📦 Updating dependencies..."
	@echo "==========================="
	cargo update

	# Check for outdated dependencies
	cargo outdated

	@echo "✅ Dependencies updated"

deps-tree: ## Show dependency tree
	@echo "🌳 Dependency tree..."
	@echo "===================="
	cargo tree

deps-unused: ## Find unused dependencies
	@echo "🔍 Finding unused dependencies..."
	@echo "================================="
	cargo +nightly udeps

# =============================================================================
# DEVELOPMENT ENVIRONMENT
# =============================================================================

shell: ## Start development shell with environment loaded
	@echo "🐚 Starting development shell..."
	@echo "==============================="
	@echo "Available commands:"
	@echo "  cargo build    - Build project"
	@echo "  cargo test     - Run tests"
	@echo "  cargo run      - Run application"
	@echo "  make help      - Show Makefile help"
	@echo ""
	bash

docker-build: ## Build Docker image
	@echo "🐳 Building Docker image..."
	@echo "==========================="
	docker build -t vibe-cli .
	@echo "✅ Docker image built"

docker-run: ## Run in Docker container
	@echo "🐳 Running in Docker..."
	@echo "======================="
	docker run -it --rm vibe-cli

# =============================================================================
# UTILITY TARGETS
# =============================================================================

version: ## Show version information
	@echo "📋 Vibe CLI Version Information"
	@echo "==============================="
	@echo "Version: $$(git describe --tags --abbrev=0 2>/dev/null || echo 'development')"
	@echo "Commit: $$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
	@echo "Branch: $$(git branch --show-current 2>/dev/null || echo 'unknown')"
	@echo "Rust: $$(rustc --version)"
	@echo "Cargo: $$(cargo --version)"

stats: ## Show project statistics
	@echo "📊 Project Statistics"
	@echo "===================="
	@echo "Lines of code:"
	@find src/ infrastructure/ domain/ shared/ tests/ -name "*.rs" -exec wc -l {} + 2>/dev/null | tail -1
	@echo ""
	@echo "File count by type:"
	@find src/ infrastructure/ domain/ shared/ tests/ -name "*.rs" | wc -l | xargs echo "  Rust files:"
	@echo ""
	@echo "Test coverage (if available):"
	@if [ -f "coverage/index.html" ]; then \
		echo "  Coverage report: coverage/index.html"; \
	else \
		echo "  No coverage report found. Run 'make test-coverage'"; \
	fi

contributors: ## Show project contributors
	@echo "👥 Project Contributors"
	@echo "======================"
	@git log --format='%aN <%aE>' | sort -u | while read author; do \
		commits=$$(git log --author="$$author" --oneline | wc -l); \
		echo "  $$author: $$commits commits"; \
	done

# =============================================================================
# ALIASES (for convenience)
# =============================================================================

b: build        ## Alias for build
t: test         ## Alias for test
r: run          ## Alias for run
c: clean        ## Alias for clean
f: format       ## Alias for format
l: lint         ## Alias for lint

# Default target when no arguments are given
.DEFAULT_GOAL := help