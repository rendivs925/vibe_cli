## Overview

This document defines the **architecture, design principles, and coding guidelines** for the **Vibe CLI** project. It serves as the single source of truth for contributors, agents, and automation working in the codebase.

The project is built using **Clean Architecture**, with strong emphasis on **SOLID**, **maintainability**, and **idiomatic Rust**.

---

## Neurosymbolic Domain System

Vibe CLI includes a **config-driven neurosymbolic reasoning system** that enables intelligent command generation through domain configurations.

### Domain Configuration Structure

```
├── domains/
    └── linux/
        ├── domain.json              # Domain manifest
        ├── operations.json          # Available operations (15 operations)
        ├── relationships.json       # Entity relationships (8 relationships)
        ├── inference_rules.json     # Symbolic reasoning rules (10 rules)
        ├── troubleshooting.json     # Troubleshooting patterns (5 patterns)
        └── entities/
            ├── process.json         # Process entity
            ├── file.json            # File entity
            ├── service.json         # Service entity
            ├── network_connection.json
            ├── user.json            # User entity
            ├── filesystem.json      # Filesystem entity
            └── memory.json          # Memory entity
```

### Domain Manifest (domain.json)

```json
{
    "domain": "linux",
    "version": "2.0.0",
    "description": "Complete Linux system administration domain",
    "depends_on": [],
    "priority": 10,
    "enabled": true,
    "tags": ["linux", "process", "filesystem", "network", "services", "users"]
}
```

### Operations (operations.json)

Operations define available actions with generators for command generation:

```json
{
    "op_id": "list_processes",
    "name": "List processes",
    "description": "List running processes with detailed information",
    "input_schema": {
        "filter": {"type": "string", "optional": true},
        "sort": {"type": "string", "optional": true}
    },
    "generators": [
        {"name": "ps_standard", "tool": "ps", "template": "ps aux", "when": []},
        {"name": "ps_sort_cpu", "tool": "ps", "template": "ps aux --sort=-%cpu", "when": []}
    ],
    "examples": [
        {"description": "Find nginx processes", "inputs": {"filter": "nginx"}}
    ]
}
```

### Entities (entities/*.json)

Entities define domain objects with properties and derived properties:

```json
{
    "name": "Process",
    "description": "A running process on the system",
    "core_properties": [
        {"name": "pid", "type": "integer", "meaning": "Process ID"},
        {"name": "cpu", "type": "number", "meaning": "CPU usage percentage"},
        {"name": "state", "type": "string", "meaning": "Process state (R/S/D/Z)"}
    ],
    "derived_properties": [
        {"name": "is_zombie", "expression": "state == 'Z'"},
        {"name": "is_running", "expression": "state == 'R'"}
    ]
}
```

### Inference Rules (inference_rules.json)

Rules for symbolic reasoning and diagnosis:

```json
{
    "rule_id": "zombie_detect",
    "name": "Zombie Detection",
    "if": [{"entity": "Process", "prop": "state", "equals": "Z"}],
    "then": [
        {"conclude": "zombie_process", "confidence": 0.99, "recommendation": "Kill the parent process"}
    ]
}
```

### Troubleshooting Patterns (troubleshooting.json)

Patterns for diagnosing common issues:

```json
{
    "pattern_id": "high_cpu",
    "name": "High CPU Usage",
    "symptoms": [{"metric": "cpu", "observation": "high cpu"}],
    "likely_causes": [{"cause": "runaway_process", "probability": 0.6}],
    "checks": [{"step": "Find CPU hog", "command": "top -bn1 | head -20"}],
    "actions": [{"action": "kill_process", "methods": ["kill", "pkill"]}]
}
```

---

## CLI Commands

### Basic Usage

```bash
# Simple query (uses LLM for command generation)
vibe_cli "list processes"

# Interactive chat mode
vibe_cli --chat

# Multi-step agent mode
vibe_cli --agent "check nginx status and restart if needed"

# RAG-based query with context
vibe_cli --rag "how do I configure systemd services?"

# Explain a file
vibe_cli --explain /path/to/config.py
```

### Neurosymbolic Commands

```bash
# Initialize complete Linux domain
vibe_cli --neurosymbolic-init

# Query with symbolic reasoning
vibe_cli --neurosymbolic "list processes"
vibe_cli --neurosymbolic "nginx is not running"
vibe_cli --neurosymbolic "disk is full"
vibe_cli --neurosymbolic "show my gpu name"

# Domain management
vibe_cli --neurosymbolic-list                    # List installed domains
vibe_cli --neurosymbolic-add <name>              # Add new domain
vibe_cli --neurosymbolic-edit <domain>           # Edit domain in $EDITOR
vibe_cli --neurosymbolic-remove <domain>         # Remove domain
vibe_cli --neurosymbolic-install <url_or_path>   # Install domain from URL/path
```

---

## Clean Architecture Design

### Architecture Overview

Vibe CLI follows Clean Architecture principles with **clear separation of concerns** and **dependency inversion**. Business logic is isolated from infrastructure and presentation concerns, enabling testability, flexibility, and long-term maintainability.

### Layer Structure

```
vibe_cli/
├── domain/                    # Core business logic (no external dependencies)
│   ├── entities/             # Business entities
│   ├── value_objects/        # Value objects
│   ├── services/             # Domain services
│   ├── repositories/         # Repository interfaces
│   └── domain_config/        # Neurosymbolic domain configuration
├── application/              # Use cases and application services
│   ├── use_cases/           # Business use cases
│   ├── services/            # Application services
│   │   ├── rag_service.rs           # RAG-based queries
│   │   └── neurosymbolic_service.rs # Symbolic reasoning
│   ├── ports/               # Interface definitions (traits)
│   └── dto/                 # Data transfer objects
├── infrastructure/          # External implementations
│   ├── ai/                  # AI client adapters (Ollama)
│   ├── storage/             # Database and file storage
│   ├── file_processing/     # Document processing
│   └── config/              # Configuration loading
├── presentation/            # User interface
│   ├── cli/                 # CLI handlers
│   │   ├── handlers.rs      # Command handlers
│   │   ├── main.rs          # CLI struct and app
│   │   ├── cache.rs         # Command caching
│   │   └── streaming.rs     # Streaming output
│   ├── views/               # Display formatting
│   └── controllers/         # Request orchestration
└── shared/                  # Common utilities
    ├── error/               # Error handling
    ├── primitives/          # Basic types
    └── utils/               # Utility functions
```

---

### Dependency Rules

Strict dependency direction must be maintained:

- **Domain**
  - Has **no dependencies** on other layers
  - Contains pure business rules only
  - `domain_config/` module is self-contained

- **Application**
  - Depends **only on Domain**
  - Orchestrates use cases and workflows
  - `NeurosymbolicService` uses `DomainRegistry`

- **Infrastructure**
  - Depends on **Domain and Application**
  - Implements interfaces (repositories, ports, adapters)

- **Presentation**
  - Depends **only on Application**
  - Handles CLI input/output and user interaction
  - Uses `NeurosymbolicService` for config-driven queries

- **Shared**
  - May be used by all layers
  - Must contain only **primitives and generic utilities**

Dependencies must always point **inward**.

---

### Key Design Patterns

The following patterns are used intentionally across the codebase:

1. **Repository Pattern**
   Abstracts data access and persistence logic

2. **Adapter Pattern**
   Integrates external systems (AI APIs, storage, file processing)

3. **Command Pattern**
   Encapsulates CLI commands and operations

4. **Factory Pattern**
   Centralizes service and dependency creation

5. **Builder Pattern**
   Constructs complex objects step-by-step with clarity

6. **Strategy Pattern** (Neurosymbolic)
   Generator selection with scoring-based fallback

7. **Template Method** (Neurosymbolic)
   Command template resolution with variable substitution

---

### SOLID Principles

The architecture enforces SOLID principles at every layer:

- **Single Responsibility**
  Each module, service, and struct has one clear purpose

- **Open–Closed**
  Extend behavior via new implementations, not modification

- **Liskov Substitution**
  All implementations must be safely interchangeable

- **Interface Segregation**
  Interfaces are small, focused, and client-specific

- **Dependency Inversion**
  High-level logic depends on abstractions, not concretions

---

## Coding Guidelines

### Core Principles

- **Clean Code**
  Write readable, maintainable code with clear intent

- **DRY (Don't Repeat Yourself)**
  Eliminate duplication through proper abstraction

- **SOLID**
  Apply all five principles consistently

- **YAGNI (You Aren't Gonna Need It)**
  Implement only what is necessary today

- **KISS (Keep It Simple, Stupid)**
  Prefer straightforward solutions over clever ones

- **Self-Explanatory Code**
  Code should explain itself without excessive comments

- **Balanced Conciseness**
  Avoid both over-verbosity and cryptic shorthand

- **Safety First**
  Prevent panics, undefined behavior, and security issues

- **Performance Awareness**
  Optimize for real-time voice and CLI responsiveness

- **Idiomatic Rust**
  Follow official Rust conventions and community best practices

---

### Code Structure Rules

- Keep modules and files between **200–300 lines of code**
- Exceed limits **only** for clear architectural reasons
- Use **guard clauses** to avoid deeply nested conditionals
- Prefer **composition over inheritance**
- Follow existing **project patterns and naming conventions**

---

### Testing Commands

```bash
# Run all tests
cargo test

# Run domain tests (use RUST_TEST_THREADS=1 to avoid race conditions)
RUST_TEST_THREADS=1 cargo test --package domain

# Check code
cargo check --package domain
cargo check --package application
cargo check --package presentation

# Build release
cargo build --release
```

---

## Final Notes

- Architecture rules are **not optional**
- Violations should be treated as bugs
- When in doubt, favor **clarity, safety, and simplicity**
- Neurosymbolic domain configs use **only `~/.config/vibe_cli`** (no `~/.local/share`)
- Tests must use `env::var("HOME")` for portable paths

This document applies to **humans, agents, and automation** working on Vibe CLI.
