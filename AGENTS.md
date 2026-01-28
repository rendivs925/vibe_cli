## Overview

This document defines the **architecture, design principles, and coding guidelines** for the **Vibe CLI** project. It serves as the single source of truth for contributors, agents, and automation working in the codebase.

The project is built using **Clean Architecture**, with strong emphasis on **SOLID**, **maintainability**, and **idiomatic Rust**.

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
│   └── repositories/         # Repository interfaces
├── application/              # Use cases and application services
│   ├── use_cases/           # Business use cases
│   ├── services/            # Application services
│   ├── ports/               # Interface definitions (traits)
│   └── dto/                 # Data transfer objects
├── infrastructure/          # External implementations
│   ├── ai/                  # AI client adapters
│   ├── storage/             # Database and file storage
│   ├── file_processing/     # Document processing
│   └── config/              # Configuration loading
├── presentation/            # User interface
│   ├── cli/                 # CLI handlers
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

- **Application**
  - Depends **only on Domain**
  - Orchestrates use cases and workflows

- **Infrastructure**
  - Depends on **Domain and Application**
  - Implements interfaces (repositories, ports, adapters)

- **Presentation**
  - Depends **only on Application**
  - Handles CLI input/output and user interaction

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

## Final Notes

- Architecture rules are **not optional**
- Violations should be treated as bugs
- When in doubt, favor **clarity, safety, and simplicity**

This document applies to **humans, agents, and automation** working on Vibe CLI.
