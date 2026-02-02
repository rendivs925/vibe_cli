# vibe_cli

<div align="center">

**Intelligent CLI assistant** that converts natural language queries into shell commands, validates them before execution, and can learn new commands from successful interactions.

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

</div>

---

## What is vibe_cli?

vibe_cli is an AI-powered command-line assistant that:

- **Understands natural language** - Ask "show my disk usage" and get `df -h`
- **Validates before executing** - Syntax checks, command availability, dangerous command blocking
- **Learns from interactions** - Teaching system builds your personal domain over time
- **Explains files and codebases** - RAG-powered documentation and code understanding
- **Multi-step task automation** - Complex workflows with safety validation

No more memorizing flags or man pages. Just describe what you need.

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

### File Explanation

```bash
vibe_cli --explain src/main.rs
vibe_cli --explain document.pdf
```

### RAG with Context

```bash
vibe_cli --rag "how does session management work?"
vibe_cli --context ./docs/
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

# With AI interpretation
vibe_cli --neurosymbolic --ai-interpret "show my gpu name"
vibe_cli --neurosymbolic --ai-interpret "check disk usage"
```

### Domain Management

```bash
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
  X lshw -short: Command not found: 'lshw' (try: apt install lshw)
  X inxi -G: Command not found: 'inxi' (try: apt install inxi)

Executing 4 valid command(s) out of 10...
Commands to execute: lspci | grep -i vga; lspci; nvidia-smi; hwinfo --short
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
Command succeeded! Learn this for future neurosymbolic queries? [y/N]

=== Learning New Command ===
Operation Name: Check journal logs
Operation ID: check_journal_logs
Description: Display recent journal entries with line count
Tool: journalctl
Template: journalctl -n 20

Save this operation to the Linux domain? [y/N]
y

Saved new operation to: /home/user/.config/vibe_cli/domains/linux/operations.json
```

The new operation is immediately available for future neurosymbolic queries. The system learns from your successful fallback commands and builds the domain dynamically.

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

### Design Patterns

- **Repository**: Data access abstraction
- **Adapter**: External system integration
- **Command**: CLI operations
- **Factory**: Service creation
- **Strategy**: Generator selection
- **Template Method**: Command resolution

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

### Rust Toolchain

```bash
rustup install stable
cargo build --release
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

## Shell Integration

### zsh

Add to `.zshrc`:

```zsh
vibe_cli_widget() {
  BUFFER="vibe_cli --chat"
  zle accept-line
}
zle -N vibe_cli_widget
bindkey '^G' vibe_cli_widget
```

Press `Ctrl-G` to start interactive session.

---

## License

MIT License - see [LICENSE](LICENSE) for details.
