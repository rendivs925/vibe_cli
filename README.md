# vibe_cli

Ultra-safe CLI assistant powered by a local Ollama model with Retrieval-Augmented Generation (RAG) capabilities. Built with Domain-Driven Design (DDD) for scalability and performance.

Latest improvements include intelligent caching, real-time progress indicators, bincode-optimized storage, semantic chunking, and comprehensive codebase indexing with smart file filtering.

## Features

- **Natural Language → Shell Command Suggestion**: Convert descriptions to safe shell commands
- **Ultra-Safe Mode (Default)**: Blocks dangerous commands (`rm -rf /`, `mkfs`, `dd` on disks, etc.)
- **Retrieval-Augmented Generation (RAG)**: Context-aware responses using codebase embeddings
- **Multi-Step Agent Mode**: Complex task planning with safety validation
- **File Explanation**: AI-powered code explanation with intelligent caching
- **Context Loading**: Load external docs (Leptos, GraphQL schemas, etc.)
- **Leptos Mode**: Automatic loading of Leptos documentation and examples
- **Intelligent Caching**: Multi-level caching with semantic similarity and bincode optimization
- **Real-time Progress**: Live status indicators for all operations
- **Smart File Processing**: Semantic chunking, deduplication, and comprehensive ignore lists
- **Performance Optimized**: Async operations, memory-mapped I/O, parallel processing, SQLite WAL storage
- **Enterprise Ready**: DDD architecture, async runtime, rootless container support

## Architecture

The project follows Domain-Driven Design with clean architecture:

- **domain**: Core business logic (CommandPlan, SafetyPolicy, Session, RAG models)
- **application**: Use cases (AgentService, RagService, ExplainService, SafetyService)
- **infrastructure**: External concerns (Ollama client, file scanning, embedding storage, search)
- **presentation**: CLI interface and adapters
- **shared**: Common utilities, errors, telemetry, types
- **tests**: Integration and performance testing
- **cli**: Binary entry point

## RAG Pipeline

The RAG system provides context-aware responses:

- **Smart File Scanning**: Memory-mapped I/O with parallel Rayon processing and comprehensive ignore lists
- **Semantic Chunking**: Intelligent text splitting on paragraph boundaries with deduplication
- **Embeddings**: Async batched generation via Ollama API with incremental updates
- **Optimized Storage**: SQLite with WAL mode, bincode serialization, and async operations
- **Fast Retrieval**: Cosine similarity search with progress indicators
- **Context Injection**: Dynamic context injection into LLM prompts

Supported file types: Rust (.rs), Markdown (.md), TOML (.toml), JSON (.json), GraphQL (.graphql), PDFs, DOCX

## Requirements

- Rust toolchain (cargo, rustc) with RUSTFLAGS="-C target-cpu=native -C link-arg=-fuse-ld=lld"
- Ollama running locally:

```bash
ollama serve
ollama pull qwen2.5-coder:3b
```

Or configure via environment:
```bash
export OLLAMA_BASE_URL=http://localhost:11434
export BASE_MODEL=qwen2.5-coder:3b
```

**Note**: Default model changed to `qwen2.5-coder:3b` for better performance.

## Build

```bash
cd vibe_cli
cargo build --release
```

The binary will be at:

```bash
target/release/vibe_cli
```

You can then move or symlink it into your PATH, e.g.:

```bash
sudo mv target/release/vibe_cli /usr/local/bin/vibe-cli
```

## Usage

The CLI accepts natural language queries directly. Use flags for special modes.

### Intelligent Caching

The CLI features multi-level intelligent caching with:
- **Command Caching**: Semantic similarity matching for shell commands (TTL: 7 days)
- **Explain Caching**: Exact-match caching for file explanations (TTL: 7 days)
- **RAG Caching**: Exact-match caching for RAG queries (TTL: 7 days)
- **Bincode Optimization**: 2-5x faster serialization than JSON
- **Persistent Storage**: Caches stored in `~/.local/share/vibe_cli/` with `.bin` extensions
- **Automatic Cleanup**: Expired entries removed on access

Cached responses are returned instantly for repeated queries.

### Basic Commands

One-shot command suggestion with intelligent caching:
```bash
vibe-cli find all .rs files larger than 1MB
vibe-cli check ssh status
```

The CLI will check for cached commands first, offering to reuse them, then generate new commands with AI if needed, and cache successful executions.

Interactive command execution:
```bash
vibe-cli --chat
```

### Agent and Explanation

Multi-step agent:
```bash
vibe-cli --agent "collect system health info: disk usage, top cpu processes, memory hogs"
```

Explain a file (with intelligent caching):
```bash
vibe-cli --explain src/main.rs
vibe-cli --explain document.pdf  # Supports PDF text extraction
vibe-cli --explain file.docx     # Supports DOCX text extraction
```

Supported file types: Rust (.rs), Markdown (.md), TOML (.toml), JSON (.json), text files, PDFs, DOCX. Binary files are detected and rejected. Explanations are cached for instant retrieval on repeat.

### RAG Commands

Query with codebase context (with intelligent caching):
```bash
vibe-cli --rag "how does the session management work?"
```

Load specific context:
```bash
vibe-cli --context ./docs/
```

Leptos documentation mode:
```bash
vibe-cli --leptos-mode
```

RAG queries scan and index your codebase using semantic chunking, parallel processing, and smart file filtering. Responses include relevant code snippets for accurate, context-aware answers.



## Configuration

Create a `.env` file in the project root:

```env
OLLAMA_BASE_URL=http://localhost:11434
BASE_MODEL=qwen2.5-coder:3b
DB_PATH=~/.local/share/vibe_cli/embeddings.db
```

**Data Storage**: All data files (embeddings database, caches) are stored in `~/.local/share/vibe_cli/` to avoid cluttering the project directory. Caches use bincode for optimal performance.

## Performance

- **Release Profile**: opt-level=3, LTO, codegen-units=1, panic=abort, strip=true
- **Async Runtime**: Custom Tokio builder with multi-thread, stack size, max blocking threads
- **Memory Management**: SmallVec, ArrayVec, Arc<str> for efficient allocations
- **File I/O**: Memory-mapped reading with memmap2
- **Parallel Processing**: Rayon for concurrent scanning and chunking
- **Database**: SQLite WAL mode with bincode serialization and async operations
- **Caching**: Multi-level bincode-optimized caches with semantic similarity
- **Chunking**: Semantic paragraph-based splitting with deduplication
- **Progress Indicators**: Real-time status updates for better UX

## Deployment

Prepared for rootless Podman microservices:

- Minimal base images (distroless or ubi-minimal)
- Configurable Ollama endpoint
- Infrastructure layer supports HTTP API extension

## Development

Run tests:
```bash
cargo test --workspace
```

Lint with clippy:
```bash
cargo clippy -- -D unwrap_used -D panic -W expect_used
```

## Optional zsh Keybinding

Add to `.zshrc`:
```zsh
vibe_cli_widget() {
  BUFFER="vibe-cli --chat"
  zle accept-line
}
zle -N vibe_cli_widget
bindkey '^G' vibe_cli_widget
```

Press `Ctrl-G` to start interactive session.