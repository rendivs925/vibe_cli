# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/SemVer).

## [Unreleased]

### Added

- **Build Mode (--build)**: New CLI mode for safe code modifications with user confirmation and AI-powered planning (Phase 1 - 100% Complete)
- **BuildService**: Comprehensive orchestration service for file operations with risk assessment and color-coded previews
- **Transaction Framework**: Full ACID transaction support with automatic rollback on failure
- **TransactionGuard**: RAII-style transaction management with auto-commit and cleanup
- **Interactive Confirmation**: Three modes (Interactive, ConfirmAll, None) with detailed diff preview and risk-based defaults
- **Advanced Agent Monitoring**: Real-time memory tracking, convergence detection, and resource usage monitoring (Phase 2 - 100% Complete)
- **Memory Tracking**: Cross-platform memory usage estimation with peak and current tracking
- **Convergence Detection**: Smart early termination based on iteration stability, confidence trends, and goal progress
- **Resource Monitoring**: Complete tracking of CPU time, I/O operations, and network requests
- **Parallel Agent Orchestrator**: CPU-aware parallel task execution with dependency resolution and load balancing (Phase 6.1)
- **Intelligent Task Decomposition**: 5 decomposition strategies with complexity analysis and dependency optimization (Phase 6.2)
- **Result Aggregation**: Multi-strategy aggregation with conflict detection and resolution (Phase 6.3)
- **Candle Inference Service**: Complete architecture for Rust-based ML inference with quantization, GPU support, and HuggingFace integration (Phase 3.1 - Design complete, awaiting upstream dependency fixes)
- **System Command Support**: Expanded sandbox to allow essential system monitoring commands (systemctl, ps, df, free, uptime, etc.) while maintaining security
- **RAG Safety Override**: Users can now override RAG query blocking when sensitive information is detected, proceeding with sanitized (masked) content
- **Project Isolation**: Complete separation of caches, embeddings, and context between different projects
- **Enhanced Security Patterns**: Improved detection of command injection, SQL injection, and dangerous patterns
- **Professional Documentation**: Complete README rewrite with comprehensive usage guide and troubleshooting

### Changed

- **Default Security Behavior**: More permissive for legitimate system administration tasks while maintaining core safety guarantees
- **RAG Content Filtering**: Non-blocking approach with user choice for handling sensitive content
- **Project Context Management**: Automatic project root detection and context-specific database isolation

### Removed

- **Leptos Mode Feature**: Removed automatic Leptos documentation loading feature (commit: 7f1d4c2)

### Fixed

- **Test Compilation**: Resolved all test compilation errors and improved test coverage
- **Sandbox Command Validation**: Fixed systemctl and other system commands being incorrectly blocked
- **Content Sanitization**: Enhanced prompt injection and malicious content detection

### Security

- **Enhanced Command Safety**: Pattern-based validation for shell commands and arguments
- **Content Security**: Multi-layer protection against prompt injection and malicious inputs
- **Secrets Detection**: Comprehensive detection and masking of sensitive information in responses

## [1.0.0] - 2025-01-01

### Added

- Initial release with core RAG capabilities
- Domain-Driven Design architecture
- Ultra-safe command execution with sandboxing
- Intelligent caching system
- Multi-step agent mode
- File explanation with AI assistance
- Comprehensive security features

### Features

- Natural language to shell command conversion
- Retrieval-Augmented Generation with codebase context
- Real-time progress indicators
- Bincode-optimized storage
- Semantic chunking and deduplication
- Enterprise-ready async architecture</content>

