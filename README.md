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

## Quick Start

```bash
# Build and install
cargo build --release
sudo mv target/release/vibe_cli /usr/local/bin/vibe_cli

# Initialize neurosymbolic domain
vibe_cli --neurosymbolic-init

# Query examples (uses standard LLM by default)
vibe_cli "list processes"
vibe_cli "show my gpu name"
vibe_cli "find all .rs files larger than 1MB"

# Use --neurosymbolic for config-driven command generation
vibe_cli --neurosymbolic "list processes"
```

---

## Commands

### Standard Query

Natural language to shell command conversion:

```bash
vibe_cli "find all .rs files larger than 1MB"
vibe_cli "check ssh status"
```

### Interactive Chat

```bash
vibe_cli --chat
```

Enters an interactive chat session where you can have a conversation about system tasks.

### Multi-Step Agent

```bash
vibe_cli --agent "collect system health: disk, cpu, memory"
```

Plans and executes complex multi-step tasks with safety validation at each step.

### ReAct Loop (Interactive)

```bash
vibe_cli --react "nginx is slow"
vibe_cli --react --neurosymbolic "nginx is not running"
```

Runs a conversational, iterative loop with **dynamic tool selection**:

```text
ANALYZE → TOOL SELECTION → TOOL EXECUTION → OUTPUT → repeat
```

The ReAct system now uses **35 specialized tools** across 8 categories to intelligently handle different task types:

| Category | Tools |
|----------|-------|
| **Investigation** | suggest_command, suggest_read, suggest_grep, suggest_rag, suggest_discovery |
| **Analysis** | summarize, extract_errors, extract_warnings, extract_metrics, extract_patterns, compare, correlate |
| **Planning** | plan_next, narrow_focus, branch, rethink, prioritize |
| **Action** | apply_fix, edit_file, create_file, run_command, retry |
| **Verification** | check_goal, verify_fix, verify_syntax, test_hypothesis |
| **Memory** | show_facts, show_hypotheses, show_history, show_context, show_plan, compact_session |
| **Resolution** | conclude_success, conclude_fail, escalate, defer |
| **Interaction** | ask_clarification, ask_confirmation, explain, suggest_alternatives |

### Tool Modes

Configure tool behavior via `ReactConfig`:

- **Legacy** - Always use suggest_command (backward compatible)
- **Mixed** - Use tool selection with fallback to suggest_command (default)
- **Full** - Full dynamic tool system, no fallback

Built-in session commands:
- `/help` - Show commands
- `/context` - Show recent reasoning history
- `/facts` - Show extracted facts
- `/hypotheses` - Show current hypotheses
- `/compact` - Summarize older steps
- `/reset` - Clear facts and hypotheses
- `/skip` - Skip current suggestion
- `/abort` - End session

Safety prompts adapt to command risk:

```text
Allow? y/n>
WARNING Will modify. Confirm? y/n>
DANGER Will modify system. Confirm? y/n>
```

### Explain Files

```bash
vibe_cli --explain /path/to/config.py
vibe_cli --explain src/main.rs
```

Explains code files with syntax highlighting. Supported formats:
- `.rs` (Rust), `.py` (Python), `.md` (Markdown)
- `.toml`, `.json`, `.graphql`
- `.pdf`, `.docx`

### RAG-Based Queries

```bash
# Query with codebase context
vibe_cli --rag "how do I configure systemd services?"

# Load context from a specific path
vibe_cli --context /path/to/project
```

Uses embeddings to understand your codebase and provide contextual answers.

### AI Interpretation

```bash
# Get readable summaries of command output
vibe_cli --ai-interpret "list processes"
```

Command execution streams with AI chunk summaries and a concise final summary.

### Clear Cache

```bash
# Clear all cached commands
vibe_cli --clear-cache

# Clear only the RAG answer cache
vibe_cli --clear-rag-cache

# Clear the RAG embeddings index for this project
vibe_cli --clear-embeddings
```

Removes cached commands from `~/.local/share/vibe_cli/`.

---

## Neurosymbolic System

The neurosymbolic system provides intelligent, config-driven command generation through JSON domain configurations.

### Enable Neurosymbolic Mode

By default, vibe_cli uses standard LLM query mode. Use the `--neurosymbolic` flag to enable config-driven command generation:

```bash
# Explicitly enable neurosymbolic mode
vibe_cli --neurosymbolic "list processes"
vibe_cli --neurosymbolic "check memory usage"
vibe_cli --neurosymbolic "nginx is not running"
vibe_cli --neurosymbolic "show my gpu name"
vibe_cli --neurosymbolic "check last 20 lines journalctl"
```

### Initialize

```bash
vibe_cli --neurosymbolic-init
```

Sets up the domain configuration directory at `~/.config/vibe_cli/domains/`.

### Query

By default, vibe_cli uses standard LLM query mode. Use `--neurosymbolic` for config-driven command generation:

```bash
# Standard LLM query (default)
vibe_cli "list processes"
vibe_cli "show my gpu name"

# Explicit neurosymbolic mode (config-driven)
vibe_cli --neurosymbolic "list processes"
vibe_cli --neurosymbolic "check memory usage"
vibe_cli --neurosymbolic "nginx is not running"
vibe_cli --neurosymbolic "check last 20 lines journalctl"
```

Queries are matched against domain operations and validated before execution.

### Domain Management

List installed domains:

```bash
vibe_cli --neurosymbolic-list
```

Add a new domain from template:

```bash
vibe_cli --neurosymbolic-add linux
```

Edit an existing domain in your editor:

```bash
vibe_cli --neurosymbolic-edit linux
```

Remove a domain:

```bash
vibe_cli --neurosymbolic-remove linux
```

Install a domain from local path or URL:

```bash
vibe_cli --neurosymbolic-install /path/to/domain
vibe_cli --neurosymbolic-install https://example.com/linux-domain.zip
```

---

## Command Validation

Before execution, neurosymbolic commands pass through:

1. **Safety Engine**: Blocks dangerous commands (`rm -rf /`, `mkfs`, `dd`)
2. **Manpage Validation**: Flags are checked against local man pages
3. **Risk Scoring**: Computes risk profile and suggests mitigations

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

### ReAct Validation

ReAct suggestions are validated before prompting:
- Syntax check via `bash -n`
- Binary availability via `command -v`

### Safety Rules

Hard rules block catastrophic operations:

| Pattern | Action |
|---------|--------|
| `rm -rf /` | Block |
| `mkfs.*` | Block |
| `dd if=/dev/zero` | Block |
| `:(){:\|:&}` | Block (fork bomb) |

Soft rules issue warnings for risky operations.

---

## Learning System

When neurosymbolic matching fails and the LLM fallback succeeds:

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

New operations are available immediately after saving.

ReAct uses an experience buffer stored in `~/.config/vibe_cli/experience.db` to avoid repeating failed commands.

---

## Cache Management

| Type | Location | Format | Compression |
|------|----------|--------|-------------|
| Commands | `~/.local/share/vibe_cli/*_cli_cache.bin` | bincode | gz (>1KB) |
| Explain | `~/.local/share/vibe_cli/*_explain_cache.bin` | bincode | gz (>1KB) |
| RAG | `~/.local/share/vibe_cli/*_rag_cache.bin` | bincode | gz (>1KB) |

### Cache Features

- **Memory-mapped I/O**: Enabled by default for all cache operations
- **Automatic compression**: Files > 1KB use flate2 gzip compression
- **Binary serialization**: Pure bincode (3-5x faster than JSON)
- **Semantic similarity**: Fuzzy matching for command retrieval

Clear cache:

```bash
vibe_cli --clear-cache
vibe_cli --clear-rag-cache
vibe_cli --clear-embeddings
```

---

## Troubleshooting

### Command not found

Ensure Ollama is running:

```bash
ollama serve
```

### Slow responses

Reduce model size for faster responses:

```bash
ollama pull qwen2.5-coder:1.5b
```

### Domain not loading

Check domain config syntax:

```bash
cat ~/.config/vibe_cli/domains/linux/domain.json | jq .
```

### Commands not being cached

Verify cache directory exists:

```bash
ls -la ~/.local/share/vibe_cli/
```

---

## Architecture

```
vibe_cli/
├── domain/                    # Core business logic
│   ├── entities/             # Business entities (ReactTool, ToolResult, etc.)
│   ├── value_objects/        # Value objects
│   ├── services/             # Domain services
│   └── domain_config/        # Neurosymbolic system
├── application/              # Use cases
│   └── services/            # Application services
│       ├── react_tools/     # Dynamic Tool System
│       │   ├── registry.rs      # ToolRegistry, ReactToolHandler
│       │   ├── handlers/         # 35 tool implementations
│       │   └── prompts/          # Tool-specific prompts
│       └── ...
├── infrastructure/          # External implementations
│   ├── ai/                  # Ollama client
│   ├── storage/             # SQLite storage
│   └── tools/               # CLI tools (read, grep, sed, etc.)
├── presentation/            # CLI interface
│   └── cli/                 # Command handlers
└── shared/                  # Common utilities
```

### ReAct Tool System Components

- **ToolRegistry**: Central registry for all 35 ReAct tools
- **ReactToolHandler**: Async trait for tool execution
- **ReactConfig**: Configuration with ToolMode (Legacy/Mixed/Full)
- **ToolResult**: Enriched results with facts, hypotheses, and next tool suggestions

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

## License

MIT License - see [LICENSE](LICENSE) for details.
