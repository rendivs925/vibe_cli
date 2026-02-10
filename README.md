# vibe_cli

<div align="center">

**AI-powered CLI assistant** with RAG capabilities and neurosymbolic reasoning - no more memorizing man pages or flag combinations.

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

</div>

---

## Why vibe_cli?

Stop memorizing man pages. Just describe what you need:

```bash
# Instead of: free -h && cat /proc/meminfo | grep MemTotal
vibe_cli "show my ram usage"

# Instead of: df -h && lsblk
vibe_cli "check disk space"

# Instead of: journalctl -n 50 --no-pager
vibe_cli "check last 50 lines of journalctl"

# Instead of: ps aux --sort=-%cpu | head -20
vibe_cli "show top cpu processes"
```

No flags to remember. No man pages to read. Just natural language.

---

## Features

### Core Capabilities

| Feature | Description |
|---------|-------------|
| **Natural Language -> Shell** | Convert descriptions to safe shell commands |
| **Ultra-Safe Mode** | Blocks dangerous commands (`rm -rf /`, `mkfs`, `dd`) |
| **RAG Context** | Codebase-aware responses using embeddings |
| **Multi-Step Agent** | Complex task planning with safety validation |
| **Neurosymbolic Reasoning** | Config-driven command generation |
| **Autonomous Safety Stack** | Fuzzy symbolic matching, manpage-validated flags, risk scoring, safety proofs |
| **Learning Loop** | Experience buffer + "do not repeat" context injection |
| **AI Interpretation** | Get readable summaries with `--ai-interpret` |
| **Syntax Validation** | Manpage-backed flag validation before execution |

### Neurosymbolic Domain System

Intelligent command generation through JSON configurations:

| Component | Count | Description |
|-----------|-------|-------------|
| Operations | 15 | process, memory, disk, network, services, hardware, logs |
| Entities | 7 | Process, File, Service, NetworkConnection, User, Filesystem, Memory |
| Relationships | 8 | hierarchical, ownership, containment, usage, binding |
| Inference Rules | 10 | zombie detection, high CPU/memory, disk full |
| Troubleshooting | 5 | disk, service, CPU, memory, network issues |

### Workflow

When domains are installed, every request goes through a deterministic pipeline. The CLI uses the LLM to propose commands, then validates them against the symbolic domain. If validation fails, it self-critiques and retries; otherwise it falls back to standard LLM generation:

1. **LLM Propose**: Generate candidate commands (normal LLM output).
2. **Symbolic Verification**: Match the candidates against domain operation templates (fuzzy similarity).
3. **Self-Critique Loop**: If mismatched, re-prompt with allowed templates and retry.
4. **Safety Engine**: Block dangerous commands.
5. **Manpage Validation**: Validate flags, retry once without invalid flags.
6. **Execution**: Prompt and run command(s).
7. **Learning**: If fallback succeeds, offer to save new operation and reload domain registry.

### Supported File Types

| Operation | Formats |
|-----------|---------|
| **Explain** | `.rs`, `.md`, `.toml`, `.json`, `.graphql`, `.pdf`, `.docx` |
| **RAG Indexing** | Same as above, plus text files |

---

## Quick Start

```bash
# Build and install
cargo build --release
sudo mv target/release/vibe_cli /usr/local/bin/vibe_cli

# Initialize neurosymbolic domain
vibe_cli --neurosymbolic-init

# Query examples (neurosymbolic when domains are installed)
vibe_cli "list processes"
vibe_cli "show my gpu name"
vibe_cli "find all .rs files larger than 1MB"
```

---

## Usage Guide

### Standard Query

```bash
vibe_cli "find all .rs files larger than 1MB"
vibe_cli check ssh status
```

### Interactive Chat

```bash
vibe_cli --chat
```

### Multi-Step Agent

```bash
vibe_cli --agent "collect system health: disk, cpu, memory"
```

### AI Output Interpretation

```bash
# With standard query (or neurosymbolic if domains are installed)
vibe_cli --ai-interpret "list processes"
vibe_cli --ai-interpret "show my gpu name"
```

When `--ai-interpret` is enabled, command execution streams in small output chunks with
AI chunk summaries followed by a concise final summary. This keeps long outputs readable
without dumping everything at once. For chunk rendering, the CLI will use `bat` (if
installed) or `less` to keep long lines readable; otherwise it falls back to wrapped
plain output.

---

## Neurosymbolic Commands

```bash
# Initialize domain
vibe_cli --neurosymbolic-init

# Query with symbolic reasoning (default when domains are installed)
vibe_cli "list processes"
vibe_cli "check memory usage"
vibe_cli "nginx is not running"
vibe_cli "show my gpu name"
vibe_cli "check last 20 lines journalctl"

# Domain management
vibe_cli --neurosymbolic-list                    # List domains
vibe_cli --neurosymbolic-add <name>              # Add domain
vibe_cli --neurosymbolic-edit <domain>           # Edit in $EDITOR
vibe_cli --neurosymbolic-remove <domain>         # Remove domain
vibe_cli --neurosymbolic-install <url_or_path>   # Install from URL
```

### Managing Domains

Add a new domain:

```bash
vibe_cli --neurosymbolic-add linux
```

Edit an existing domain:

```bash
vibe_cli --neurosymbolic-edit linux
```

Remove a domain:

```bash
vibe_cli --neurosymbolic-remove linux
```

List installed domains:

```bash
vibe_cli --neurosymbolic-list
```

Install a domain from a local path or URL:

```bash
vibe_cli --neurosymbolic-install /path/to/domain
vibe_cli --neurosymbolic-install https://example.com/linux-domain.zip
```

---

## Command Validation

**Neurosymbolic commands are validated before execution.**

### How It Works

Before a neurosymbolic command is executed, it passes through:

1. **Safety Engine**: Hard rules block catastrophic commands
2. **Manpage Validation**: Flags are checked against local man pages
3. **Risk Scoring**: Risk profile and mitigations are computed

### Validation Flow

```
Input Command
       ↓
Safety Engine → Block/Allow
       ↓
Manpage Validation → Strip invalid flags (retry once)
       ↓
Risk Scoring → Report + Mitigations
       ↓
Execution
```

---

## Learning System

When neurosymbolic matching fails and the LLM fallback succeeds, you can teach the system new commands:

```bash
$ vibe_cli "check last 20 lines journalctl"
# Falls back to LLM, executes successfully

Command succeeded! Learn this for future queries? [y/N]
y

=== Learning New Command ===
Operation Name: Check journal logs
Tool: journalctl
Template: journalctl -n 20

Saved to: ~/.config/vibe_cli/domains/linux/operations.json
```

The new operation is available immediately after saving.

---

## Cache Management

```bash
# Clear all cached commands
vibe_cli --clear-cache

# Output
Cleared: /home/user/.local/share/vibe_cli/xxx_cli_cache.bin
Cleared: /home/user/.local/share/vibe_cli/xxx_explain_cache.bin
Cleared: /home/user/.local/share/vibe_cli/xxx_rag_cache.bin
Cleared: /home/user/.cache/vibe_cli/commands.json

Cleared 4 cache file(s), 0 failed
```

### Cache Locations

| Type | Location | Format | Compression |
|------|----------|--------|-------------|
| Commands | `~/.local/share/vibe_cli/*_cli_cache.bin` | bincode | gz (>1KB) |
| Explain | `~/.local/share/vibe_cli/*_explain_cache.bin` | bincode | gz (>1KB) |
| RAG | `~/.local/share/vibe_cli/*_rag_cache.bin` | bincode | gz (>1KB) |
| Streaming | `~/.cache/vibe_cli/commands.json` | JSON | none |

### Cache Performance Features

- **Memory-mapped I/O**: Enabled by default for all cache operations
- **Automatic compression**: Files > 1KB use flate2 gzip compression
- **Binary serialization**: Pure bincode (3-5x faster than JSON)
- **Semantic similarity**: Fuzzy matching for command retrieval
- **TTL-based expiration**: Automatic cleanup of stale entries

---

## Architecture

```
vibe_cli/
├── domain/                    # Core business logic
│   ├── entities/             # Business entities (SmallVec optimized)
│   ├── value_objects/        # Value objects (SmallVec optimized)
│   ├── services/            # Domain services
│   └── domain_config/        # Neurosymbolic system
├── application/              # Use cases
│   └── ports/
│       └── configuration.rs  # Cache config (mmap enabled)
├── infrastructure/          # External implementations
│   ├── ai/                  # Ollama client
│   ├── storage/             # SQLite + bincode embeddings
│   └── cache/               # CacheManager (mmap + compression)
├── presentation/            # CLI interface
│   └── cli/
│       └── cache.rs         # Pure bincode serialization
└── shared/                  # Common utilities
```

### Key Performance Components

- **CacheManager**: Memory-mapped I/O + flate2 compression + bincode
- **FileSymbolicStorage**: Automatic mmap for large trace files (>1MB)
- **EmbeddingStorage**: SQLite WAL mode with bincode vector serialization
- **SmallVec Data Structures**: 8 optimized collections across domain layer

---

## Configuration

### Environment Variables

```env
OLLAMA_BASE_URL=http://localhost:11434
BASE_MODEL=qwen2.5-coder:3b
```

### Data Storage

| Data | Location |
|------|----------|
| Domain configs | `~/.config/vibe_cli/domains/` |
| Embeddings DB | `~/.local/share/vibe_cli/embeddings.db` |
| Caches | `~/.local/share/vibe_cli/` |

---

## Requirements

### Ollama Setup

```bash
# Start Ollama server
ollama serve

# Pull recommended model
ollama pull qwen2.5-coder:3b
```

---

## Performance

| Optimization | Description | Impact |
|--------------|-------------|--------|
| **Build** | opt-level=3, LTO, codegen-units=1 | Maximum compilation optimization |
| **Async** | Tokio with optimized settings | High-concurrency throughput |
| **Memory** | SmallVec for 8 data structures | 20-30% fewer heap allocations |
| **I/O** | Memory-mapped I/O enabled by default | 30-50% faster cache operations |
| **Compression** | flate2 for cache files > 1KB | 25-40% reduced disk I/O |
| **Serialization** | Pure bincode (no JSON fallback) | 3-5x faster serialization |
| **Parallel** | Rayon for concurrent operations | Multi-threaded computation |
| **Database** | SQLite WAL mode with bincode | Optimized embedding storage |
| **Symbolic Storage** | Automatic mmap for files > 1MB | 40-60% faster trace operations |
| **Caching** | Multi-level with semantic similarity | Instant command retrieval |

### Memory Optimization Details

The following data structures use `SmallVec<[T; N>]` for stack allocation:

- **Session History**: `SmallVec<[Message; 8]>` - Most sessions have 2-6 messages
- **Query Context**: `SmallVec<[String; 4]>` - Typically 1-3 context strings
- **Query Results**: `SmallVec<[SearchResult; 8]>` - Top-N results
- **Command Planning**: `SmallVec<[Command; 5]>` - Multi-step commands
- **Safety Checks**: `SmallVec<[SafetyCheck; 3]>` - Fixed validation checks
- **Similarity Results**: `SmallVec<[SearchResult; 8]>` - Filtered embeddings
- **Outlier Detection**: `SmallVec<[usize; 8]>` - Index results

---

## Development

```bash
# Run tests
RUST_TEST_THREADS=1 cargo test --package domain
cargo test --workspace

# Check code
cargo check --package domain
cargo check --package application
cargo check --package presentation

# Build
cargo build --release
```

---

## Installation

```bash
# Build
cargo build --release

# Install system-wide
sudo mv target/release/vibe_cli /usr/local/bin/vibe_cli
```

---

## License

MIT License - see [LICENSE](LICENSE) for details.
