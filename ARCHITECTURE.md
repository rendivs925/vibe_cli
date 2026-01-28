# Clean Architecture Design for Vibe CLI

## Architecture Overview

This project follows Clean Architecture principles with clear separation of concerns and dependency inversion.

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

### Dependency Rules

- **Domain**: No dependencies on other layers
- **Application**: Depends on Domain only
- **Infrastructure**: Depends on Domain and Application (implements interfaces)
- **Presentation**: Depends on Application only
- **Shared**: Can be used by all layers (primitives only)

### Key Design Patterns

1. **Repository Pattern**: For data access abstraction
2. **Adapter Pattern**: For external system integration
3. **Command Pattern**: For CLI operations
4. **Factory Pattern**: For service creation
5. **Builder Pattern**: For complex object construction

### SOLID Principles Implementation

- **S**: Each service has a single responsibility
- **O**: Use cases are open for extension, closed for modification
- **L**: Infrastructure adapters can substitute interfaces
- **I**: Interfaces are focused and client-specific
- **D**: Dependencies point inward toward the domain