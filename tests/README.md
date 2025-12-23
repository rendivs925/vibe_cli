# Vibe CLI Comprehensive Test Suite

This directory contains comprehensive end-to-end tests for the Vibe CLI project, testing all major features with real-world user inputs and expected outputs.

## Test Structure

### Test Categories

1. **Basic Functionality Tests** (`comprehensive_tests.rs`)
   - Command generation
   - Agent mode with complex tasks
   - File explanation functionality
   - Interactive chat mode
   - Caching mechanisms
   - Error handling and edge cases

2. **Performance Tests** (`performance_tests.rs`)
   - Command generation benchmarks
   - RAG indexing performance
   - Large file processing
   - Concurrent request handling
   - Memory usage validation
   - Cache performance analysis

3. **Integration Tests** (`integration_tests.rs`)
   - External service integration (Ollama)
   - Filesystem operations
   - Database integration
   - Web functionality
   - System command execution
   - Resource limit testing

4. **Core Infrastructure Tests** (`lib.rs`)
   - Sandbox safety validation
   - Dangerous pattern detection
   - Confirmation manager functionality
   - Execution limits
   - Path validation
   - Command whitelisting

## Running Tests

### Quick Test Run
```bash
# Run all tests
cargo test --package tests

# Run specific test file
cargo test --package tests --lib comprehensive_tests
cargo test --package tests --lib performance_tests
cargo test --package tests --lib integration_tests
```

### Using the Test Runner
```bash
# Run comprehensive test suite with output
./test_runner.sh

# Run specific category
./test_runner.sh "Basic Functionality"
./test_runner.sh "Performance"
./test_runner.sh "Integration"
```

### Individual Test Examples
```bash
# Test basic command generation
cargo test --package tests --lib test_basic_command_generation -- --nocapture

# Test agent mode
cargo test --package tests --lib test_agent_mode_complex_task -- --nocapture

# Test RAG functionality
cargo test --package tests --lib test_rag_functionality -- --nocapture

# Test performance benchmarks
cargo test --package tests --lib performance_tests::benchmark_command_generation -- --nocapture

# Test security features
cargo test --package tests --lib test_sandbox_safety -- --nocapture
```

## Test Scenarios

### Real-World User Scenarios

1. **Development Workflow**
   - Setting up new projects
   - Building and compiling code
   - Running tests
   - Debugging issues

2. **File Management**
   - Explaining code files
   - Processing documentation
   - Handling multiple file formats (PDF, DOCX, source code)

3. **System Administration**
   - Monitoring system resources
   - Managing services
   - Configuration tasks

4. **AI-Assisted Tasks**
   - Multi-step agent operations
   - Context-aware querying
   - Interactive problem solving

### Expected Outputs

Each test validates:
- **Correct command generation** for various user queries
- **Proper error handling** for invalid inputs
- **Security compliance** with sandbox restrictions
- **Performance benchmarks** within acceptable limits
- **Integration reliability** with external services

## Test Data

### Sample Files Created During Tests
- Rust projects with `Cargo.toml` and `src/main.rs`
- JavaScript/TypeScript files
- Python modules
- Configuration files
- Documentation in various formats

### Temporary Directories
All tests use temporary directories that are automatically cleaned up:
```rust
let temp_dir = TempDir::new().expect("Failed to create temp dir");
```

## Performance Benchmarks

### Expected Performance Metrics
- **Command Generation**: < 10 seconds per query
- **RAG Indexing**: < 45 seconds for 20+ files
- **Large File Processing**: < 30 seconds for 1000+ functions
- **Concurrent Requests**: < 60 seconds for 5 parallel operations
- **Cache Performance**: Significantly faster on repeated queries

## Security Testing

### Dangerous Operations Tested
- System file modification (`/etc/*`, `/dev/*`)
- Device access (`/dev/sda`, `/dev/mem`)
- Fork bombs and infinite loops
- Code injection attempts
- Privilege escalation

### Safe Operations Validated
- File listing and searching
- Development tool usage (`cargo`, `git`, `npm`)
- System monitoring (`ps`, `df`, `top`)
- File creation and editing

## Integration Requirements

### Optional Dependencies
- **Ollama Server**: For AI model integration tests
- **Network Access**: For web search functionality
- **File Permissions**: For file operation tests

### Graceful Degradation
Tests verify that the CLI:
- Handles missing services gracefully
- Provides fallbacks when external dependencies are unavailable
- Maintains functionality in limited environments
- Provides clear error messages

## Continuous Integration

### CI/CD Considerations
- Tests marked with `#[ignore]` when external dependencies are required
- Timeout handling for long-running operations
- Platform-specific behavior validation
- Resource usage monitoring

### Environment Variables
```bash
# Set test environment
export RUST_TEST_THREADS=1
export RUST_LOG=debug

# Configure Ollama (if available)
export OLLAMA_HOST=http://localhost:11434
```

## Test Output Analysis

### Success Indicators
- All tests compile without errors
- Performance benchmarks within limits
- Security validations passing
- Integration tests completing successfully

### Expected Warnings
- Missing Ollama server (graceful handling)
- Network unavailability (fallback behavior)
- Permission issues (safe operation restrictions)

### Failure Investigation
1. Check compilation errors: `cargo build --verbose`
2. Verify dependencies: `cargo tree`
3. Check test logs with `--nocapture`
4. Validate environment setup

## Contributing Tests

### Adding New Tests
1. Follow the existing naming conventions
2. Use temporary directories for file operations
3. Include proper error handling validation
4. Add performance expectations where applicable
5. Document the test scenario clearly

### Test Best Practices
- Use descriptive test function names
- Include assertions for both success and failure cases
- Test edge cases and error conditions
- Validate performance characteristics
- Ensure cleanup of test resources

## Coverage Analysis

### Feature Areas Tested
- ✅ Command generation and execution
- ✅ Multi-step agent workflows  
- ✅ File explanation and processing
- ✅ RAG (Retrieval-Augmented Generation)
- ✅ Interactive chat interface
- ✅ Caching mechanisms
- ✅ Security sandboxing
- ✅ Error handling
- ✅ Performance benchmarks
- ✅ Integration with external services

### Test Coverage Goals
- All public API endpoints
- Error paths and edge cases
- Security boundary conditions
- Performance regression prevention
- User workflow validation

This comprehensive test suite ensures the Vibe CLI is production-ready, secure, performant, and provides excellent user experience across various real-world scenarios.