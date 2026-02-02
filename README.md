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
| **AI Interpretation** | Get readable summaries with `--ai-interpret` |

### Neurosymbolic Domain System

Intelligent command generation through JSON configurations:

| Component | Count | Description |
|-----------|-------|-------------|
| Operations | 15 | process, memory, disk, network, services, hardware, logs |
| Entities | 7 | Process, File, Service, NetworkConnection, User, Filesystem, Memory |
| Relationships | 8 | hierarchical, ownership, containment, usage, binding |
| Inference Rules | 10 | zombie detection, high CPU/memory, disk full |
| Troubleshooting | 5 | disk, service, CPU, memory, network issues |

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

# Query examples
vibe_cli --neurosymbolic "list processes"
vibe_cli --neurosymbolic "show my gpu name"
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
# With standard query
vibe_cli --ai-interpret "list processes"

# With neurosymbolic (executes multiple commands, then summarizes)
vibe_cli --neurosymbolic --ai-interpret "show my gpu name"
```

---

## Neurosymbolic Commands

```bash
# Initialize domain
vibe_cli --neurosymbolic-init

# Query with symbolic reasoning
vibe_cli --neurosymbolic "list processes"
vibe_cli --neurosymbolic "check memory usage"
vibe_cli --neurosymbolic "nginx is not running"
vibe_cli --neurosymbolic "show my gpu name"
vibe_cli --neurosymbolic "check last 20 lines journalctl"

# Domain management
vibe_cli --neurosymbolic-list                    # List domains
vibe_cli --neurosymbolic-add <name>              # Add domain
vibe_cli --neurosymbolic-edit <domain>           # Edit in $EDITOR
vibe_cli --neurosymbolic-remove <domain>         # Remove domain
vibe_cli --neurosymbolic-install <url_or_path>   # Install from URL
```

---

## Command Validation

Before execution, commands are validated:

```bash
$ vibe_cli --neurosymbolic "show my gpu name"

Command Validation: 4/10 valid
Invalid commands:
  X lshw -short: Command not found (try: apt install lshw)
  X inxi -G: Command not found (try: apt install inxi)

Executing 4 valid command(s)...
```

**Validation types:**
- **Syntax**: `bash -n` for syntax checking
- **Availability**: `command -v` to verify binaries
- **Helpful errors**: Suggests install commands

---

## Learning System

When neurosymbolic matching fails and the LLM fallback succeeds, you can teach the system new commands:

```bash
$ vibe_cli --neurosymbolic "check last 20 lines journalctl"
# Falls back to LLM, executes successfully

Command succeeded! Learn this for future queries? [y/N]
y

=== Learning New Command ===
Operation Name: Check journal logs
Tool: journalctl
Template: journalctl -n 20

Saved to: ~/.config/vibe_cli/domains/linux/operations.json
```

The new operation is immediately available for future queries.

---

## Cache Management

```bash
# Clear all cached commands
vibe_cli --clear-cache

# Output
Cleared: /home/user/.local/share/vibe_cli/xxx_cli_cache.json
Cleared: /home/user/.local/share/vibe_cli/xxx_explain_cache.bin
Cleared: /home/user/.local/share/vibe_cli/xxx_rag_cache.bin
Cleared: /home/user/.cache/vibe_cli/commands.json

Cleared 4 cache file(s), 0 failed
```

### Cache Locations

| Type | Location |
|------|----------|
| Commands | `~/.local/share/vibe_cli/*_cli_cache.json` |
| Explain | `~/.local/share/vibe_cli/*_explain_cache.bin` |
| RAG | `~/.local/share/vibe_cli/*_rag_cache.bin` |
| Streaming | `~/.cache/vibe_cli/commands.json` |

---

## Architecture

```
vibe_cli/
├── domain/                    # Core business logic
│   └── domain_config/        # Neurosymbolic system
├── application/              # Use cases
│   └── services/
│       ├── rag_service.rs
│       └── neurosymbolic_service.rs
├── infrastructure/          # External implementations
│   ├── ai/                  # Ollama client
│   └── storage/             # Database
├── presentation/            # CLI interface
│   └── cli/
└── shared/                  # Common utilities
```

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

| Optimization | Description |
|--------------|-------------|
| Build | opt-level=3, LTO, codegen-units=1 |
| Async | Tokio with optimized settings |
| Memory | SmallVec, ArrayVec for efficiency |
| I/O | Memory-mapped reading |
| Parallel | Rayon for concurrent operations |
| Database | SQLite WAL mode with bincode |
| Caching | Multi-level with semantic similarity |

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
