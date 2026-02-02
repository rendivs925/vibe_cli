# vibe_cli

AI-powered CLI assistant with **RAG capabilities** and **neurosymbolic reasoning**. Built with Clean Architecture for safety, extensibility, and performance.

## Features

### Core Capabilities

- **Natural Language → Shell Command**: Convert descriptions to safe shell commands
- **Ultra-Safe Mode (Default)**: Blocks dangerous commands (`rm -rf /`, `mkfs`, `dd` on disks, etc.)
- **Retrieval-Augmented Generation (RAG)**: Context-aware responses using codebase embeddings
- **Multi-Step Agent Mode**: Complex task planning with safety validation
- **Neurosymbolic Reasoning**: Config-driven command generation with domain configs
- **AI Output Interpretation**: Get readable summaries of command results with `--ai-interpret`

### Neurosymbolic Domain System

Intelligent command generation through JSON-based domain configurations:

- **15 Linux operations**: process, memory, disk, network, services, files, users, permissions, Docker, hardware info
- **7 entities**: Process, File, Service, NetworkConnection, User, Filesystem, MemoryInfo
- **8 relationships**: hierarchical, ownership, containment, usage, binding
- **10 inference rules**: zombie detection, high CPU/memory, disk full, service failure
- **5 troubleshooting patterns**: high CPU, high memory, disk full, service down, network issues
- **Fuzzy matching + synonyms**: Better accuracy for natural language queries
- **Priority keyword matching**: Hardware queries get boosted confidence (95%)

### File Support

- **Explain**: Rust (.rs), Markdown (.md), TOML (.toml), JSON (.json), GraphQL, PDFs, DOCX
- **RAG Indexing**: Same formats plus text files, binary files rejected

## Quick Start

```bash
# Build
cargo build --release

# Initialize neurosymbolic domain (recommended first step)
vibe_cli --neurosymbolic-init

# Use neurosymbolic reasoning
vibe_cli --neurosymbolic "list processes"
vibe_cli --neurosymbolic "nginx is not running"
vibe_cli --neurosymbolic "disk is full"

# Standard query
vibe_cli "find all .rs files larger than 1MB"
```

## Neurosymbolic Commands

```bash
# Initialize complete Linux domain
vibe_cli --neurosymbolic-init

# Query with symbolic reasoning
vibe_cli --neurosymbolic "list processes"
vibe_cli --neurosymbolic "check memory usage"
vibe_cli --neurosymbolic "nginx is not running"
vibe_cli --neurosymbolic "show my gpu name"

# With AI interpretation (get readable summaries)
vibe_cli --neurosymbolic --ai-interpret "show my gpu name"
vibe_cli --neurosymbolic --ai-interpret "check disk usage"

# Domain management
vibe_cli --neurosymbolic-list                    # List installed domains
vibe_cli --neurosymbolic-add <name>              # Add new domain
vibe_cli --neurosymbolic-edit <domain>           # Edit domain in $EDITOR
vibe_cli --neurosymbolic-remove <domain>         # Remove domain
vibe_cli --neurosymbolic-install <url_or_path>   # Install domain from URL/path
```

## Domain Configuration

Domain configs are stored in `~/.config/vibe_cli/domains/`:

```
~/.config/vibe_cli/
├── domains/
│   └── linux/
│       ├── domain.json              # Domain manifest
│       ├── operations.json          # 15 operations
│       ├── entities/                # 7 entities
│       ├── relationships.json       # 8 relationships
│       ├── inference_rules.json     # 10 inference rules
│       └── troubleshooting.json     # 5 patterns
└── shared_entities/
    └── port.json                    # Shared entity templates
```

### Example Operation

```json
{
    "op_id": "list_processes",
    "name": "List processes",
    "description": "List running processes",
    "generators": [
        {"name": "ps_standard", "tool": "ps", "template": "ps aux", "when": []},
        {"name": "ps_sort_cpu", "tool": "ps", "template": "ps aux --sort=-%cpu", "when": []}
    ]
}
```

### Example Entity

```json
{
    "name": "Process",
    "core_properties": [
        {"name": "pid", "type": "integer", "meaning": "Process ID"},
        {"name": "cpu", "type": "number", "meaning": "CPU usage"}
    ],
    "derived_properties": [
        {"name": "is_zombie", "expression": "state == 'Z'"}
    ]
}
```

## Standard CLI Commands

### Basic Query

```bash
vibe_cli find all .rs files larger than 1MB
vibe_cli check ssh status
```

### Interactive Chat

```bash
vibe_cli --chat
```

### Agent Mode (Multi-Step)

```bash
vibe_cli --agent "collect system health: disk, cpu, memory"
```

### File Explanation

```bash
vibe_cli --explain src/main.rs
vibe_cli --explain document.pdf
vibe_cli --explain file.docx
```

### RAG with Context

```bash
vibe_cli --rag "how does session management work?"
vibe_cli --context ./docs/
```

### AI Output Interpretation

Get readable, AI-powered summaries of command results:

```bash
# With standard query
vibe_cli --ai-interpret "list processes"
vibe_cli --ai-interpret "show disk usage"

# With neurosymbolic mode (executes multiple commands, then summarizes all at once)
vibe_cli --neurosymbolic --ai-interpret "show my gpu name"
vibe_cli --neurosymbolic --ai-interpret "check memory usage"
```

The AI executes all relevant commands first, then interprets the combined output into a single, comprehensive summary. This eliminates duplicate interpretations and provides a cleaner user experience.

## Architecture

Clean Architecture with clear separation:

```
vibe_cli/
├── domain/                    # Core business logic
│   └── domain_config/        # Neurosymbolic domain system
├── application/              # Use cases and services
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

### Key Design Patterns

- **Repository Pattern**: Data access abstraction
- **Adapter Pattern**: External system integration
- **Command Pattern**: CLI operations
- **Factory Pattern**: Service creation
- **Strategy Pattern**: Generator selection with scoring
- **Template Method**: Command template resolution

## Requirements

- Rust toolchain (cargo, rustc)
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

## Configuration

### Environment Variables

```env
OLLAMA_BASE_URL=http://localhost:11434
BASE_MODEL=qwen2.5-coder:3b
```

### Data Storage

- **Domain configs**: `~/.config/vibe_cli/domains/`
- **Embeddings DB**: `~/.local/share/vibe_cli/embeddings.db`
- **Caches**: `~/.local/share/vibe_cli/` (bincode optimized)

## Performance

- **Release Build**: opt-level=3, LTO, codegen-units=1
- **Async Runtime**: Tokio with optimized settings
- **Memory**: SmallVec, ArrayVec for efficiency
- **File I/O**: Memory-mapped reading
- **Parallel Processing**: Rayon for concurrent operations
- **Database**: SQLite WAL mode with bincode
- **Caching**: Multi-level with semantic similarity

## Development

```bash
# Run tests (use RUST_TEST_THREADS=1 for domain tests)
RUST_TEST_THREADS=1 cargo test --package domain
cargo test --workspace

# Check code
cargo check --package domain
cargo check --package application
cargo check --package presentation

# Build release
cargo build --release
```

## Installation

```bash
# Build
cargo build --release

# Install system-wide
sudo mv target/release/vibe_cli /usr/local/bin/vibe_cli
```

## Optional zsh Keybinding

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
