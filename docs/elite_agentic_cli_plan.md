# Elite Agentic CLI: Comprehensive Implementation Plan

## Vision
A persistent, omniscient, self-correcting coding co-pilot that lives in your terminal.
It watches your project live, fixes errors before you report them, remembers everything, and suggests improvements — all while feeling instantaneous.

## Core Architecture (Rust-First, Ultra-Performance)

### Component Implementation Choices
- **Concurrency**: Tokio (async runtime) + Rayon (parallelism) + Crossbeam/Flume (lock-free channels)
- **Background Engine**: Supervised long-lived tasks: notify (file watcher), LSP client, test/log tailer
- **Persistence**: Primary: sled (embedded ACID DB) → ~/.ai-agent/data/<project-hash>.sled
- **Large Codebase RAG**: ripgrep (keyword) + optional Qdrant/hnswlib-rs (semantic) + Tree-sitter cached ASTs
- **Memory Optimization**: Arc<str>, Cow<str>, Bytes, object pooling, small-string opts (compact_str)
- **Error Handling**: anyhow + thiserror + tracing (with tokio-console support)

## Session Management (Multiple Named Sessions)

### User-Facing Commands
```bash
ai --session "auth-refactor" --build "implement JWT login"  # Create/switch session
ai --list-sessions                                         # Show all sessions
ai --session "auth-refactor" --continue                    # Resume specific session
ai --delete-session "old-experiment"                       # Remove session
```

### Storage Strategy
- **One sled DB per project**: ~/.ai-agent/data/<project-hash>.sled
- **Prefixed keys for isolation**:
  - `session:list` → Vec<SessionMetadata>
  - `session:metadata:<name>` → SessionMetadata (last_used, goal_summary, change_count)
  - `session:history:<name>` → Vec<Message>
  - `session:applied:<name>:<id>` → AppliedDiff
  - `session:undo:<name>` → Vec<UndoEntry>
  - `session:background:<name>` → Background state snapshot

### Session Metadata Structure
```rust
pub struct SessionMetadata {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
    pub goal_summary: String,
    pub change_count: u32,
    pub is_active: bool,
}
```

## 6-Phase Implementation Roadmap

### Phase 1: Foundation — Session Persistence & Multi-Session Support (Weeks 1–2)
**Goal**: Fix hallucinations through mandatory file reading and robust multi-session persistence

**Deliverables:**
- Multiple Session Management: Named session creation/switching with `--session "name"` flag
- Session Commands: `--list-sessions`, `--delete-session`, `--continue` for specific sessions
- Project-based Persistence: BLAKE3 project hashing with sled DB per project
- Prefixed Key Storage: Session isolation via `session:<name>:<data>` keys in sled
- Session Metadata: Track last_used, goal_summary, change_count per session
- Mandatory Pre-reading: All mentioned files read before any planning
- Dynamic File Extraction: Regex + semantic analysis for file discovery
- Default Session: Auto-create/use "main" session when no `--session` specified
- Session Backup: Optional JSON export on operations

**Key Rust Crates:**
```rust
sled = "0.34"                    # Embedded ACID database
blake3 = "1.5"                   # Project hashing
tokio-stream = "0.1"            # Async stream utilities
compact_str = "0.7"             # Memory-efficient strings
serde_json = "1.0"              # JSON backup/export
chrono = "0.4"                  # Session timestamps
```

**Test Commands:**
```bash
# Multiple sessions
ai --session "auth-refactor" --build "implement JWT"
ai --session "dark-mode" --build "add theme toggle"
ai --list-sessions               # Shows all sessions with metadata
ai --session "auth-refactor" --continue
ai --delete-session "old-experiment"

# Default session
ai --build "fix bug"             # Uses "main" session
ai --session "main" --continue   # Explicit main session

# Context awareness with sessions
ai --session "auth-refactor" --build "fix login"  # Reads auth files first
```

### Phase 2: True CRUD & Safe Editing (Weeks 3–4)
**Goal**: Replace hallucinations with true, targeted file modifications

**Deliverables:**
- AI generates `REPLACE`, `INSERT`, `DELETE` operations instead of full rewrites
- Atomic file operations with timestamped backups
- Git integration (`git add`, `git commit` on apply)
- Per-Session Undo: `--undo` command scoped to current session
- File operation validation and conflict detection
- Session-Specific Change Tracking: Applied diffs stored per session

**Key Rust Crates:**
```rust
git2 = "0.18"                    # Git integration
diff = "0.1"                     # Text diffing utilities
tempfile = "3.8"                 # Atomic temp file operations
notify = "6.1"                   # File watching foundation
uuid = "1.6"                     # Change ID generation
```

### Phase 3: Real-Time Streaming UX (Weeks 4–5)
**Goal**: Make the agent feel alive with incremental progress and previews

**Deliverables:**
- 8-stage incremental planning with live progress indicators
- Chunked diff previews with syntax highlighting
- Single final confirmation with `[y/N/edit/revise/suggest]` options
- Session-Aware Status: Show current session in progress display
- Real-time background status updates
- Streaming error handling and recovery

**Key Rust Crates:**
```rust
crossterm = "0.27"              # Terminal control (colors, cursor)
syntect = "5.1"                 # Syntax highlighting
futures = "0.3"                 # Stream utilities
tokio-util = "0.7"              # Additional async utilities
indicatif = "0.17"              # Progress bars (optional)
```

### Phase 4: Background Intelligence (Weeks 6–8)
**Goal**: Persistent awareness of project state through background monitoring

**Deliverables:**
- File watcher with debounced change notifications
- LSP client integration (rust-analyzer, typescript-language-server)
- Test/build watcher with real-time results
- Log tailer for server/application logs
- Per-Session Background State: Background services scoped to active session
- Background event broadcasting system

**Key Rust Crates:**
```rust
async-lsp = "0.1"               # LSP protocol client
tokio-process = "0.2"           # Process management
watchexec = "3.0"               # File watching (alternative to notify)
tower-lsp = "0.20"              # LSP tower integration
flume = "0.11"                  # High-performance channels
```

### Phase 5: Autonomous Self-Correction (Weeks 9–10)
**Goal**: Agent fixes errors before you even report them

**Deliverables:**
- Error detection from test failures and log entries
- Stack trace parsing with `syn` crate
- Per-Session Auto-Fix: Autonomous fix loop scoped to current session (max 3 attempts)
- Temporary patch application and re-testing
- Failure recovery and user escalation
- Session-Aware Error Context: Fixes based on session's change history

**Key Rust Crates:**
```rust
syn = "2.0"                      # Rust AST parsing for stack traces
nom = "7.1"                      # Parser combinator for logs
regex = "1.10"                   # Pattern matching for errors
tokio-retry = "0.3"              # Retry logic for fixes
```

### Phase 6: Proactive Mastery (Weeks 11–12)
**Goal**: Agent suggests improvements proactively

**Deliverables:**
- Post-apply analysis with LSP and Tree-sitter
- Proactive suggestion system (`ai --suggest`)
- Large codebase RAG optimization (ripgrep + cached ASTs)
- Performance profiling and optimization
- Cross-Session Learning: Suggestions based on patterns across sessions
- Final polish and comprehensive testing

**Key Rust Crates:**
```rust
ripgrep = "13.0"                 # Fast text search
tree-sitter = "0.20"             # AST caching and queries
hnswlib-rs = "0.1"               # Vector similarity (optional)
criterion = "0.5"                # Benchmarking
```

## Performance Targets (2025 Elite Tier)

- Startup + session load: <100ms
- RAG search (100k files): <250ms
- File read + diff preview: instant
- Background overhead: <5% CPU

## Implementation Priority

**Phase 1 First**: Session persistence is foundational - fixes 80% of current hallucinations and enables multi-turn conversations.

## Use Cases for Multiple Sessions

- **Parallel Development**: Work on auth while experimenting with dark mode
- **Experimentation Sandbox**: Try risky refactors in isolated sessions
- **Long-running Tasks**: Resume database migrations weeks later
- **Teaching/Demos**: Separate tutorial sessions
- **Personal Workflow**: "main" for stable work, "spike" for experiments

## Architecture Benefits

- **Isolation**: Independent state per session
- **Safety**: No cross-contamination between features
- **Flexibility**: Switch contexts instantly
- **History**: Long-term project evolution tracking
- **Performance**: Efficient sled storage with prefixed keys

---

**Status**: Plan Finalized and Saved
**Ready for Phase 1 Implementation**
**Timeline**: 11 weeks total
**Quality**: Production-ready blueprint</content>
<parameter name="filePath">/home/rendi/projects/vibe_cli/docs/elite_agentic_cli_plan.md