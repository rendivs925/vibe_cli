# qwen_cli_assistant

Ultra-safe CLI assistant powered by a local Ollama model with Retrieval-Augmented Generation (RAG) capabilities. Built with Domain-Driven Design (DDD) for scalability and performance.

Latest improvements include a full RAG pipeline for codebase-aware responses, parallel file processing, and enterprise-grade architecture.

## Features

- **Natural Language → Shell Command Suggestion**: Convert descriptions to safe shell commands
- **Ultra-Safe Mode (Default)**: Blocks dangerous commands (`rm -rf /`, `mkfs`, `dd` on disks, etc.)
- **Retrieval-Augmented Generation (RAG)**: Context-aware responses using codebase embeddings
- **Multi-Step Agent Mode**: Complex task planning with safety validation
- **File Explanation**: AI-powered code explanation for Rust files
- **Context Loading**: Load external docs (Leptos, GraphQL schemas, etc.)
- **Leptos Mode**: Automatic loading of Leptos documentation and examples
- **Performance Optimized**: Memory-mapped file I/O, parallel processing, SQLite WAL storage
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

- **File Scanning**: Memory-mapped I/O with parallel Rayon processing
- **Chunking**: Zero-copy text splitting with configurable overlap
- **Embeddings**: Async batched generation via Ollama API
- **Storage**: SQLite with WAL mode and optimized PRAGMA settings
- **Retrieval**: Cosine similarity search for relevant context
- **Injection**: Dynamic context injection into LLM prompts

Supported file types: Rust (.rs), Markdown (.md), TOML (.toml), JSON (.json), GraphQL (.graphql)

## Requirements

- Rust toolchain (cargo, rustc) with RUSTFLAGS="-C target-cpu=native -C link-arg=-fuse-ld=lld"
- Ollama running locally:

```bash
ollama serve
ollama pull qwen2.5:7b
```

Or configure via environment:
```bash
export OLLAMA_BASE_URL=http://localhost:11434
export OLLAMA_MODEL=qwen2.5:7b
```

## Build

```bash
cd qwen_cli_assistant
cargo build --release
```

The binary will be at:

```bash
target/release/qwen_cli_assistant
```

You can then move or symlink it into your PATH, e.g.:

```bash
sudo mv target/release/qwen_cli_assistant /usr/local/bin/qwen-cli
```

## Usage

### Basic Commands

One-shot command suggestion:
```bash
qwen-cli "find all .rs files larger than 1MB"
```

Interactive chat:
```bash
qwen-cli chat
```

### Agent and Explanation

Multi-step agent:
```bash
qwen-cli agent "collect system health info: disk usage, top cpu processes, memory hogs"
```

Explain a file:
```bash
qwen-cli explain src/main.rs
```

### RAG Commands

Query with codebase context:
```bash
qwen-cli rag "how does the session management work?"
```

Load specific context:
```bash
qwen-cli context ./docs/ chat
```

Leptos documentation mode:
```bash
qwen-cli leptos-mode
```

Interactive chat:
```bash
qwen-cli --chat
```

### RAG Commands

Query with codebase context:
```bash
qwen-cli --rag "how does the session management work?"
```

Load specific context:
```bash
qwen-cli --context ./docs/ --chat
```

Leptos documentation mode:
```bash
qwen-cli --leptos-mode
```

### Agent and Explanation

Multi-step agent:
```bash
qwen-cli --agent "collect system health info: disk usage, top cpu processes, memory hogs"
```

Explain a file:
```bash
qwen-cli --explain src/main.rs
```

## Configuration

Create a `.env` file in the project root:

```env
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_MODEL=qwen2.5:7b
DB_PATH=embeddings.db
```

## Performance

- **Release Profile**: opt-level=3, LTO, codegen-units=1, panic=abort, strip=true
- **Async Runtime**: Custom Tokio builder with multi-thread, stack size, max blocking threads
- **Memory Management**: SmallVec, ArrayVec, Arc<str> for efficient allocations
- **File I/O**: Memory-mapped reading with memmap2
- **Parallel Processing**: Rayon for concurrent scanning and chunking
- **Database**: SQLite WAL mode with optimized settings

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
qwen_cli_widget() {
  BUFFER="qwen-cli --chat"
  zle accept-line
}
zle -N qwen_cli_widget
bindkey '^G' qwen_cli_widget
```

Press `Ctrl-G` to start interactive session.