## Coding Guidelines

### Principles

- **Clean Code**: Write readable, maintainable code with clear intent
- **DRY (Don't Repeat Yourself)**: Eliminate duplication through abstraction
- **SOLID**: Single responsibility, Open-closed, Liskov substitution, Interface segregation, Dependency inversion
- **YAGNI (You Aren't Gonna Need It)**: Implement only what's necessary
- **KISS (Keep It Simple, Stupid)**: Prefer simple solutions over complex ones
- **Self-Explanatory Code**: Write code that explains itself without excessive comments
- **Balanced Conciseness**: Code should be neither too verbose nor too abbreviated
- **Safety First**: Always write safe code that prevents common errors and vulnerabilities
- **Ultra High Performance**: Optimize for extreme performance using advanced techniques
- **Idiomatic Code**: Follow Rust conventions and best practices for the language

### Code Structure

- Limit modules/files to 200-300 lines of code (LOC)
- Exceed this limit only with clear architectural purpose
- Use guard clauses to avoid deeply nested conditions
- Follow existing patterns and conventions in the codebase

### Commands

- Lint: `cargo clippy`
- Typecheck/Build: `cargo check` / `cargo build`
- Test: `cargo test`
