# Vibe CLI

A comprehensive AI-powered CLI assistant for system administration, development automation, and intelligent command execution. Features natural language processing, system information analysis, package installation, multi-step task automation, and enterprise-grade security with clean, universal terminal compatibility.

## Overview

Vibe CLI is a comprehensive AI-powered CLI assistant that transforms natural language into safe system operations. It combines intelligent command generation, system information processing, package installation capabilities, and multi-step task automation with enterprise-grade security and user experience.

**Core Capabilities:**
- **Natural Language Processing**: Convert descriptions into precise, safe shell commands
- **System Information Analysis**: AI-processed answers to "what's my GPU?", "how much RAM?", etc.
- **Installation & Setup**: Safe package and service installation with dependency management
- **Multi-Step Agent Execution**: Complex task automation with planning, safety assessment, and progress tracking
- **Code Analysis**: AI-powered file and codebase understanding with RAG technology
- **Security First**: Two-level confirmations, safety policy overrides, and sandbox isolation

The system prioritizes security while offering powerful automation features for developers, system administrators, and DevOps professionals.

## Key Features

### Security First
- **Ultra-safe execution**: Blocks dangerous commands while allowing essential system operations
- **Content sanitization**: Prevents prompt injection and malicious input
- **Secrets detection**: Automatically masks sensitive information
- **Sandbox isolation**: Controlled command execution environment
- **Two-level confirmations**: Separate approval for intent and safety overrides

### AI-Powered Intelligence
- **Natural language processing**: Convert descriptions to precise shell commands
- **System information processing**: AI-processed answers for "what's my GPU?", "how much RAM?"
- **Installation capabilities**: Safe package and service installation with dependency analysis
- **Multi-step agent execution**: Complex task planning with safety assessment and progress tracking
- **Context-aware responses**: RAG system with codebase embeddings
- **Intelligent caching**: Semantic similarity matching with bincode optimization

### High Performance
- **Async architecture**: Non-blocking operations throughout
- **Memory-mapped I/O**: Efficient file processing
- **Parallel processing**: Concurrent scanning and embedding generation
- **Optimized storage**: SQLite with WAL mode and compressed serialization
- **Clean text interface**: No emojis, universal terminal compatibility

## Architecture

Built with clean architecture principles and Domain-Driven Design:

- **domain**: Core business logic and models
- **application**: Use case orchestration and services
- **infrastructure**: External integrations and persistence
- **presentation**: CLI interface and user interaction
- **shared**: Common utilities and cross-cutting concerns
- **tests**: Comprehensive testing suite
- **cli**: Binary entry point

## RAG System

The Retrieval-Augmented Generation system delivers contextually relevant responses by analyzing your codebase:

### Pipeline Components
- **Intelligent Scanning**: Memory-mapped I/O with parallel processing and smart file filtering
- **Semantic Chunking**: Context-preserving text segmentation with deduplication
- **Embedding Generation**: Async batched processing via Ollama API
- **Vector Storage**: SQLite with WAL mode and optimized serialization
- **Similarity Search**: Cosine similarity ranking with relevance scoring
- **Context Injection**: Dynamic prompt engineering for accurate responses

### Supported Formats
**Code Files**: Rust (.rs), Go (.go), Python (.py), JavaScript (.js), TypeScript (.ts)
**Documentation**: Markdown (.md), reStructuredText (.rst)
**Configuration**: TOML (.toml), JSON (.json), YAML (.yaml)
**Schemas**: GraphQL (.graphql), Protocol Buffers (.proto)
**Documents**: PDF (.pdf), Microsoft Office (.docx, .xlsx)

## Prerequisites

### System Requirements
- **Rust Toolchain**: Latest stable version with Cargo package manager
- **Build Optimization**: Configure RUSTFLAGS for performance
  ```bash
  export RUSTFLAGS="-C target-cpu=native -C link-arg=-fuse-ld=lld"
  ```

### AI Model Setup
- **Ollama**: Local AI model server (recommended for privacy and offline usage)

```bash
# Install and start Ollama
ollama serve

# Pull the recommended model
ollama pull qwen2.5:1.5b-instruct
```

### Alternative Configuration
For custom Ollama deployments or cloud endpoints:

```bash
export OLLAMA_BASE_URL=http://your-ollama-server:11434
export BASE_MODEL=qwen2.5:1.5b-instruct
```

## Installation

### Build from Source
```bash
git clone <repository-url>
cd vibe_cli
cargo build --release
```

### Binary Location
The compiled binary is available at:
```bash
target/release/vibe_cli
```

### System Integration
Add to your PATH for global access:
```bash
# Option 1: Copy to system directory
sudo cp target/release/vibe_cli /usr/local/bin/

# Option 2: Create symlink
sudo ln -sf $(pwd)/target/release/vibe_cli /usr/local/bin/
```

## Usage

Vibe CLI accepts natural language queries and supports multiple operational modes through command-line flags.

### Core Commands

#### Natural Language Command Generation
Transform descriptions into safe, executable shell commands:

```bash
# File operations
vibe_cli find all Rust files larger than 1MB
vibe_cli compress old log files

# System monitoring
vibe_cli check SSH service status
vibe_cli systemctl status sshd

# Development tasks
vibe_cli run all unit tests
vibe_cli check code coverage
```

#### System Information Queries
Get AI-processed answers to system questions:

```bash
# Hardware information
vibe_cli "what's my GPU"
vibe_cli "how much RAM do I have"
vibe_cli "what CPU is this"

# System status
vibe_cli "show disk usage"
vibe_cli "check network interfaces"
vibe_cli "list running processes"

# Software information
vibe_cli "what packages are installed"
vibe_cli "check service status"
vibe_cli "show system logs"
```

#### Installation Commands
Safely install packages and configure services:

```bash
# Package installation
vibe_cli "install python development tools"
vibe_cli "setup nginx web server"
vibe_cli "add git and development tools"

# Service configuration
vibe_cli "configure firewall"
vibe_cli "setup database server"
vibe_cli "enable SSH service"

# Development environments
vibe_cli "install Node.js and npm"
vibe_cli "setup Rust development environment"
vibe_cli "configure Docker"
```

#### Interactive Mode
Start an interactive session for multiple commands:

```bash
vibe_cli --chat
```

**Features:**
- Real-time command generation and validation
- Safety confirmation for potentially risky operations
- Session persistence across commands

### Advanced Modes

#### Multi-Step Agent Workflow
Execute complex, multi-phase tasks with intelligent planning, safety assessment, and progress tracking:

```bash
# Complete automated execution
vibe_cli --agent "set up a new Rust project with CI/CD pipeline"
vibe_cli --agent "create a web application with database"

# Step-by-step execution with manual control
vibe_cli --agent --step-by-step "setup development environment"
vibe_cli --agent --step-by-step "deploy application to server"

# Dry-run mode for validation
vibe_cli --agent --dry-run "configure production server"
vibe_cli --agent --dry-run "set up monitoring stack"

# Safety-conscious execution
vibe_cli --agent --network=allow "install development tools"
vibe_cli --agent --permissive "configure system services"
```

**Agent Features:**
- **Pre-analysis**: Task decomposition with dependency analysis
- **Safety assessment**: Risk evaluation for each command
- **Structured planning**: Clear execution plans with time estimates
- **Progress tracking**: Real-time status with completion indicators
- **Error recovery**: Intelligent handling of failures with retry options
- **Next-step suggestions**: Automated guidance for post-execution tasks

#### System Information Processing
AI-powered analysis of system queries with human-readable answers:

```bash
# Direct answers without command execution details
$ vibe "what's my GPU"
GPU: NVIDIA GeForce RTX 2080

$ vibe "how much RAM do I have"
RAM: 16GB total, 4GB used (75% available)

$ vibe "show disk usage"
/dev/sda1: 256GB total, 89GB used (65% full)
```

**Features:**
- **Intelligent parsing**: Raw command output converted to natural language
- **Confidence scoring**: Quality indicators for answer reliability
- **Progressive disclosure**: Technical details available when needed
- **Safety first**: Read-only operations with confirmation when required

#### Installation & Setup System
Comprehensive package and service installation with safety checks:

```bash
# Package installation with dependency analysis
$ vibe "install python development tools"

INSTALLATION COMMAND DETECTED
Command: sudo apt install python3-dev python3-pip
Packages to install: python3-dev, python3-pip
System changes: Disk space ~50MB
Execute installation? [y/N] y

Installation completed successfully
Next steps: pip3 install virtualenv

# Safety override for blocked commands
$ vibe "install iptables"

INSTALLATION COMMAND DETECTED
Command: sudo pacman -S iptables
Execute installation? [y/N] y

Command 'sudo pacman -S iptables' is blocked by safety policy.
Execute anyway? [y/N] y

# Installation proceeds with explicit override
```

**Safety Features:**
- **Pre-execution analysis**: Impact assessment before installation
- **Dependency resolution**: Automatic handling of package requirements
- **Safety policy integration**: Override capability for trusted operations
- **Post-installation guidance**: Configuration and usage instructions

#### Code Analysis
AI-powered file and codebase analysis:

```bash
# Explain specific files
vibe_cli --explain src/main.rs
vibe_cli --explain Cargo.toml

# Analyze entire codebases
vibe_cli --rag "how does the authentication system work?"
vibe_cli --rag "explain the error handling patterns"
```

#### Context Loading
Load documentation and schemas from external sources:

```bash
vibe_cli --context ./docs/ --rag "how does the API work?"
vibe_cli --context ./schemas/ --explain api.graphql
```

### Caching System

Vibe CLI implements intelligent multi-level caching for optimal performance:

| Cache Type | Strategy | TTL | Purpose |
|------------|----------|-----|---------|
| **Command** | Semantic similarity | 7 days | Shell command suggestions |
| **System Info** | Exact match | 1 hour | System information queries |
| **Installation** | Command validation | 1 day | Installation command validation |
| **Agent Plans** | Task similarity | 7 days | Multi-step execution plans |
| **Explain** | Exact match | 7 days | File/code explanations |
| **RAG** | Exact match | 7 days | Context-aware queries |

**Storage Details:**
- **Location**: `~/.local/share/vibe_cli/`
- **Format**: Bincode serialization (2-5x faster than JSON)
- **Cleanup**: Automatic expiration and LRU eviction
- **Safety**: Cached commands validated before reuse

### Safety & Confirmation System

Vibe CLI implements a comprehensive safety system with two-level confirmations:

#### Two-Level Confirmation Process

**Level 1: Intent Confirmation**
```bash
DATA COLLECTION REQUIRED
Purpose: Gather GPU information for analysis
Safety: Read-only, no system modifications
Allow command execution? [y/N]
```

**Level 2: Safety Policy Override (when blocked)**
```bash
Command 'sudo pacman -S iptables' is blocked by safety policy.
Execute anyway? [y/N]
```

#### Safety Classifications

| Risk Level | Description | Examples | Confirmation Required |
|------------|-------------|----------|----------------------|
| **Info Only** | Read-only queries | `ls`, `ps`, `df` | No |
| **Safe Operations** | Basic file operations | `mkdir`, `cp`, `echo` | Low |
| **Network Access** | Internet-dependent | `npm install`, `git clone` | Medium |
| **System Changes** | Configuration changes | `chmod`, `systemctl` | High |
| **Destructive** | Data-destructive | `rm -rf`, `dd` | Always blocked |

#### Safety Features

- **Command validation**: Syntax and semantic checking
- **Path verification**: Commands only use accessible files
- **Permission auditing**: Clear escalation warnings
- **Resource monitoring**: Memory and disk space tracking
- **Audit logging**: Complete execution records

#### File Analysis
Get AI-powered explanations of code and documentation:

```bash
# Code files
vibe_cli --explain src/main.rs
vibe_cli --explain lib/core.py

# Documentation
vibe_cli --explain README.md
vibe_cli --explain API_DOCS.pdf

# Configuration files
vibe_cli --explain docker-compose.yml
vibe_cli --explain kubernetes/deployment.yaml
```

**Supported Formats:**
- Programming languages (Rust, Python, Go, JavaScript, etc.)
- Markup formats (Markdown, reStructuredText)
- Configuration files (TOML, JSON, YAML)
- Documentation (PDF, DOCX)
- Schemas (GraphQL, Protocol Buffers)

#### RAG-Powered Queries
Query your codebase with full context awareness:

```bash
# Architecture questions
vibe_cli --rag "how does the authentication system work?"
vibe_cli --rag "explain the data flow between services"

# Implementation details
vibe_cli --rag "where is the user validation logic?"
vibe_cli --rag "how are database connections pooled?"

# Best practices
vibe_cli --rag "what are the error handling patterns?"
vibe_cli --rag "how is logging configured?"
```

**Context Loading:**
```bash
# Load specific directories
vibe_cli --context ./docs/ --rag "how does the API work?"
vibe_cli --context ./src/ --rag "explain the caching strategy"
```

**Security Features:**
- **Automatic sanitization**: Sensitive data is masked in responses
- **Safety override**: User choice when sensitive content is detected
- **Content isolation**: Project-specific embeddings prevent cross-contamination



## Configuration

### Environment Variables
Create a `.env` file in your project root or set environment variables:

```bash
# AI Model Configuration
OLLAMA_BASE_URL=http://localhost:11434
BASE_MODEL=qwen2.5:1.5b-instruct

# Storage Configuration
DB_PATH=~/.local/share/vibe_cli/embeddings.db

# Security Settings
VIBE_SANDBOX_ENABLED=true
VIBE_MAX_MEMORY_MB=1024
```

### Data Storage

Vibe CLI maintains isolated storage for each project:

- **Location**: `~/.local/share/vibe_cli/`
- **Project Isolation**: Each project uses hashed identifiers for separate storage
- **Cache Types**:
  - Embeddings databases (SQLite with project-specific names)
  - Command caches (semantic similarity matching)
  - Explanation caches (exact match lookup)
  - RAG response caches (context-aware storage)

## Performance Characteristics

### Optimization Features
- **Compiler Profile**: opt-level=3, LTO, single codegen unit, panic optimization
- **Async Runtime**: Custom Tokio configuration with optimized threading
- **Memory Management**: Efficient allocations with SmallVec, ArrayVec, Arc<str>
- **I/O Operations**: Memory-mapped file reading with `memmap2`
- **Concurrency**: Parallel processing with Rayon for CPU-intensive tasks

### Storage Performance
- **Database**: SQLite with WAL mode for concurrent access
- **Serialization**: Bincode format (2-5x faster than JSON)
- **Caching**: Multi-level semantic similarity matching
- **Chunking**: Intelligent text segmentation with deduplication

### Monitoring
- **Progress Indicators**: Real-time status updates for long operations
- **Resource Tracking**: Memory usage monitoring and limits
- **Performance Metrics**: Operation timing and throughput statistics

## Deployment

### Container Deployment
Vibe CLI is designed for secure containerized deployment:

```dockerfile
FROM rust:slim AS builder
# Build process...

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/vibe_cli /usr/local/bin/
ENTRYPOINT ["vibe_cli"]
```

### Kubernetes Integration
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vibe-cli
spec:
  template:
    spec:
      containers:
      - name: vibe-cli
        image: vibe-cli:latest
        env:
        - name: OLLAMA_BASE_URL
          value: "http://ollama-service:11434"
        securityContext:
          runAsNonRoot: true
          readOnlyRootFilesystem: true
```

### Service Architecture
- **Stateless Design**: No persistent state requirements
- **HTTP API**: Infrastructure layer supports REST endpoints
- **Configurable Endpoints**: Environment-based Ollama configuration
- **Resource Limits**: Memory and CPU constraints for containerized deployment

## What's New

### v2.0 Features

#### Enhanced Multi-Step Agent Workflow
- **Intelligent task planning** with dependency analysis
- **Safety assessment** for each execution step
- **Progress tracking** with real-time status updates
- **Error recovery** with automated retry strategies
- **Execution modes**: Complete, step-by-step, and dry-run

#### System Information Processing
- **AI-powered answers** to system queries ("what's my GPU?", "how much RAM?")
- **Human-readable output** instead of raw command results
- **Confidence scoring** with progressive detail disclosure
- **Safety-first execution** with user confirmations

#### Installation & Setup System
- **Package installation** with dependency analysis
- **Service configuration** with safety checks
- **Post-installation guidance** and next-step suggestions
- **Safety policy integration** with override capabilities

#### Advanced Security Features
- **Two-level confirmations** for intent and safety override
- **Command risk classification** (Info, Safe, Network, System, Destructive)
- **Safety policy system** with customizable rules
- **Audit logging** for compliance and debugging

#### Clean Interface Design
- **Text-only interface** with no emoji dependencies
- **Universal terminal compatibility** across all environments
- **Structured output** with clear visual hierarchy
- **Minimal color usage** with monochrome fallbacks

## Development

### Testing
Run the comprehensive test suite:
```bash
cargo test --workspace
```

### Code Quality
Lint with Clippy for code standards:
```bash
cargo clippy -- -D unwrap_used -D panic -W expect_used
```

### Performance Profiling
```bash
# Profile with flamegraph
cargo flamegraph --bin vibe_cli -- --rag "test query"
```

## Shell Integration

### Zsh Keybinding
Add to your `~/.zshrc` for quick access:

```zsh
vibe_cli_widget() {
  BUFFER="vibe_cli --chat"
  zle accept-line
}
zle -N vibe_cli_widget
bindkey '^G' vibe_cli_widget  # Ctrl-G to start interactive mode
```

### Bash Integration
```bash
# Add to ~/.bashrc
alias vibe='vibe_cli --chat'
```

## Troubleshooting

### Common Issues

**Ollama Connection Failed**
```bash
# Check Ollama status
ollama list
curl http://localhost:11434/api/tags

# Verify environment variables
echo $OLLAMA_BASE_URL
echo $BASE_MODEL
```

**Permission Denied**
```bash
# Ensure executable permissions
chmod +x target/release/vibe_cli

# Check sandbox permissions for system commands
vibe_cli --version
```

**Safety Policy Blocks Commands**
```bash
# Commands blocked by safety policy can be overridden
vibe "install iptables"
# Answer 'y' to both confirmation prompts

# Modify safety policy in config if needed
# Check ~/.config/vibe_cli/config.yaml
```

**System Information Commands Fail**
```bash
# Ensure you have permission to run system commands
vibe "what's my GPU"
# May require confirmation for system queries

# Check if sandbox is blocking commands
vibe --version  # Should work without issues
```

**Installation Commands Not Recognized**
```bash
# Use explicit installation language
vibe "install python"        # Works
vibe "setup nginx"          # Works
vibe "python"              # May not trigger installation mode

# Use quotes for multi-word packages
vibe "install 'visual studio code'"
```

**Agent Commands Take Too Long**
```bash
# Use dry-run to validate without execution
vibe --agent --dry-run "setup project"

# Use step-by-step for manual control
vibe --agent --step-by-step "setup project"
```

**Slow Performance**
```bash
# Enable optimizations
export RUSTFLAGS="-C target-cpu=native -C link-arg=-fuse-ld=lld"

# Rebuild with optimizations
cargo build --release
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Guidelines
- Follow Domain-Driven Design principles
- Maintain comprehensive test coverage
- Use async patterns for I/O operations
- Implement proper error handling
- Document public APIs

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**Vibe CLI** - Secure, intelligent automation for developers and system administrators.