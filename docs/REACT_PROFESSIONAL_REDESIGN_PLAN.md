# ReAct Flow Redesign - Professional Production Grade Plan

## Overview

Redesign the ReAct interactive flow to be conversational, adaptive, and safe with professional-grade context engineering, embeddings, and RAG.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PRESENTATION LAYER                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │ ReactHandler                                                            │    │
│  │  - UI rendering (analyze/suggested/output/summary)                    │    │
│  │  - User input parsing                                                  │    │
│  │  - Command confirmation flow                                           │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────────────────────┤
│                           APPLICATION LAYER                                   │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────┐   │
│  │ ReactAgentService │  │ ContextRetriever │  │ AnalysisService     │   │
│  │  - Orchestration │  │  - Context build │  │  - Intent detection │   │
│  │  - Loop control   │  │  - Source fusion │  │  - Output analysis  │   │
│  └──────────────────┘  └──────────────────┘  └──────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────────┤
│                           CONTEXT ENGINEERING LAYER                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────┐    │
│  │ SessionMemory    │  │ SemanticIndex    │  │ ExperienceLearner      │    │
│  │                 │  │                 │  │                         │    │
│  │ - Conversation  │  │ - Embeddings    │  │ - Command patterns     │    │
│  │ - Facts         │  │ - Vector search │  │ - Success/failure      │    │
│  │ - Hypotheses    │  │ - RAG retrieval │  │ - User preferences     │    │
│  │ - Constraints   │  │ - Re-ranking    │  │ - Workflows            │    │
│  └────────┬────────┘  └────────┬────────┘  └────────────┬────────────┘    │
│           │                    │                         │                 │
│           └────────────────────┼─────────────────────────┘                 │
│                                ▼                                            │
│                    ┌───────────────────────┐                               │
│                    │   ContextRetriever    │                               │
│                    │                       │                               │
│                    │  - Query analysis    │                               │
│                    │  - Source selection   │                               │
│                    │  - Context fusion     │                               │
│                    │  - Prompt building   │                               │
│                    └───────────────────────┘                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                           DOMAIN LAYER                                        │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────┐   │
│  │ ReactSession     │  │ SessionMemory     │  │ QueryIntent          │   │
│  │  - Steps history │  │  - Facts/hypotheses│ │  - Task classification│   │
│  │  - Status        │  │  - Constraints    │  │  - Tool selection   │   │
│  └──────────────────┘  └──────────────────┘  └──────────────────────┘   │
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │ Repository Traits (Ports)                                            │  │
│  │  - ReactRepository                                                  │  │
│  │  - ReactCommandRepository                                           │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│                           INFRASTRUCTURE LAYER                               │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────┐  │
│  │ InMemoryStorage   │  │ EmbeddingStorage  │  │ KnowledgeGraph       │  │
│  │  - Session state  │  │  - Vector store  │  │  - System entities   │  │
│  └──────────────────┘  └──────────────────┘  └──────────────────────┘  │
│  ┌──────────────────┐  ┌──────────────────┐                              │
│  │ ExperienceBuffer │  │ OllamaClient      │                              │
│  │  - Failure learn │  │  - LLM inference  │                              │
│  └──────────────────┘  └──────────────────┘                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Flow Structure

### Clean Conversational Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                     CONVERSATIONAL REACT                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  USER: "nginx is slow"                                          │
│                                                                  │
│  START                                                             │
│  Goal: nginx is slow                                            │
│                                                                  │
│  ── ANALYZE ─────────────────────────────────────────────────    │
│  Intent: Debug performance                                        │
│  Tools: [process, network, logs]                                  │
│                                                                  │
│  ── SUGGESTED ──────────────────────────────────────────────    │
│  uptime && free -h                                                │
│                                                                  │
│  Allow? y/n> y                                                 │
│                                                                  │
│  ── OUTPUT ──────────────────────────────────────────────────    │
│  Load: 3.42 | Mem: 28GB/31GB (90%)                             │
│                                                                  │
│  ── ANALYZE ─────────────────────────────────────────────────    │
│  → Memory at 90%. Likely cause of slowness.                       │
│    Next: check nginx processes                                    │
│                                                                  │
│  ── SUGGESTED ──────────────────────────────────────────────    │
│  ps aux | grep nginx                                             │
│                                                                  │
│  Allow? y/n> y                                                 │
│                                                                  │
│  ── OUTPUT ──────────────────────────────────────────────────    │
│  nginx worker: 45% CPU, 8% MEM                                  │
│                                                                  │
│  ── ANALYZE ─────────────────────────────────────────────────    │
│  → Runaway worker (45% CPU). New code deployed 2h ago?           │
│    Next: restart nginx                                            │
│                                                                  │
│  ── SUGGESTED ──────────────────────────────────────────────    │
│  sudo systemctl restart nginx                                     │
│                                                                  │
│  ⚠ Will modify system. Confirm? y/n> yes                        │
│                                                                  │
│  ── OUTPUT ──────────────────────────────────────────────────    │
│  Active: active (running)                                         │
│                                                                  │
│  ── VERIFY ──────────────────────────────────────────────────    │
│  curl http://localhost/ → 200 OK (45ms)                          │
│                                                                  │
│  ── COMPLETE ────────────────────────────────────────────────    │
│  ✓ Fixed: Restarted nginx (15s → 45ms)                          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## UI Refinements

### Labels

| Old | New |
|-----|-----|
| `THOUGHT` | `ANALYZE` |
| `ACTION` | `SUGGESTED` |
| `OBSERVATION` | `OUTPUT` |

### Confirmation Prompts

| Command Type | Prompt |
|--------------|--------|
| Read-only | `→ Allow? y/n>` (Enter works) |
| Write | `⚠ Will modify. Confirm? y/n>` |
| Destructive | `⚠ DANGER. Confirm? y/n>` |

---

## Command Safety Classification

### Classification Rules

```rust
enum CommandSafety {
    ReadOnly,
    Write,
    Destructive,
}

fn classify_command(command: &str) -> CommandSafety {
    // Read-only patterns
    let read_only = [
        "read ", "grep ", "fd ", "rag ", "ls", "pwd", "cat", "head", "tail",
        "ps", "top", "free", "df", "du", "uptime", "curl -s", "curl -S",
        "systemctl status", "service status", "ss ", "netstat",
        "git status", "git diff", "git log", "git show",
    ];
    
    // Destructive patterns
    let destructive = [
        "rm ", "rmdir", "dd ", "mkfs", "fdisk", "parted",
        "sudo systemctl start", "sudo systemctl stop", "sudo systemctl restart",
        "kill ", "pkill ", "killall",
        "reboot", "shutdown", "halt", "poweroff",
        "git push", "git force",
    ];
    
    // Check patterns...
}
```

### Confirmation Rules

| Command Type | Prompt | Empty Input |
|--------------|--------|--------------|
| Read-only | `Allow? y/n>` | Execute |
| Write | `⚠ Will modify. Confirm? y/n>` | Require y/yes |
| Destructive | `⚠ DANGER. Confirm? y/n>` | Require y/yes |

---

## User Intent Types

```rust
enum UserIntent {
    Approve,              // y, yes,, enter (read-only only)
    Reject,               // n, no ok
    Redirect(String),     // "let's try X instead", "actually..."
    Question(String),     // "what does this do?", "how does X work?"
    Clarify(String),     // "it's production", "no errors"
    Skip,                 // /skip
    Abort,                // /abort
    Help,                 // /help
    Context,              // /context
    Compact,              // /compact
}
```

### Input Handling

| Input | Action |
|-------|--------|
| `y`, `yes` | Execute (required for non-read-only) |
| `Enter` | Execute (read-only only) |
| `n`, `no` | Free input mode |
| `/abort` | Exit session |
| `/help` | Show available commands |
| `/skip` | Skip step |
| `/context` | Show reasoning history |
| `/compact` | Summarize old steps |
| `/facts` | Show extracted facts |
| `/hypotheses` | Show current hypotheses |
| `/reset` | Clear facts/hypotheses |
| Any text | User direction/question |

---

## Context Engineering

### SessionMemory Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemory {
    pub session_id: String,
    pub goal: String,
    pub created_at: DateTime<Utc>,
    
    // Extracted during session
    pub constraints: Vec<Constraint>,
    pub facts: Vec<Fact>,
    pub hypotheses: Vec<Hypothesis>,
    pub key_insights: Vec<Insight>,
    
    // For semantic retrieval
    pub embedding_id: Option<String>,
    pub semantic_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub key: String,           // "environment"
    pub value: String,         // "production"
    pub source: String,        // "user input"
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub key: String,           // "memory_usage"
    pub value: String,         // "90%"
    pub source_command: String,
    pub source_step: usize,
    pub verified: bool,
    pub embedding_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub description: String,
    pub confidence: f32,
    pub supporting_facts: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub text: String,
    pub importance: f32,       // 0-1
    pub created_at: DateTime<Utc>,
}
```

### QueryIntent Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryIntent {
    pub task_type: TaskType,
    pub target: Option<String>,
    pub constraints: Vec<String>,
    pub tool_categories: Vec<ToolCategory>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Debug,      // diagnose issues
    Explore,    // discover/understand
    Fix,        // repair/resolve
    Explain,    // document/describe
    Monitor,    // watch/track
    Configure,  // setup/change
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    Process,
    Network,
    Filesystem,
    Service,
    Logs,
    Package,
    Git,
    Build,
    Shell,
}
```

---

## Context Sources & Retrieval

| Source | When to Use | Retrieval Method |
|--------|-------------|------------------|
| Current session | Always | Linear (recent 6 steps) |
| Session facts | Analysis phase | Direct lookup |
| Past sessions | Similar query | Semantic search |
| Command history | Tool selection | Pattern matching |
| Failures | Error handling | Experience buffer |
| Knowledge graph | System state | Entity lookup |
| User preferences | Any | Learned patterns |

---

## Prompt Engineering Template

```rust
fn build_react_prompt(
    query: &str,
    context: &RetrievedContext,
) -> String {
    format!(
        r#"You are a systems debugging assistant using ReAct reasoning.

## Current Task
{query}

## Session History (recent)
{session_history}

## Extracted Facts
{facts}

## Current Hypotheses
{hypotheses}

## Relevant Past Experiences
{experiences}

## System Context (from knowledge graph)
{knowledge_context}

## Instructions
- Use facts to support reasoning
- Avoid commands that failed before
- Consider user constraints: {constraints}
- Suggest next diagnostic step

## Output format
ANALYZE: <reasoning about current state>
SUGGESTED: <command to execute>
"#
    )
}
```

---

## Session Commands

| Command | Description |
|---------|-------------|
| `/context` | Show goal, constraints, facts, recent steps |
| `/facts` | Show extracted facts only |
| `/hypotheses` | Show current hypotheses |
| `/compact` | Summarize old steps into single summary |
| `/reset` | Clear facts/hypotheses, keep goal |
| `/abort` | End session |
| `/help` | Show available commands |

---

## Context Display Examples

### `/context` Output

```
╭─ CONTEXT ─────────────────────────────────────────────────────╮
│ Goal: nginx is slow                                             │
│ Constraints: production, no errors                              │
│                                                               │
│ Facts (4):                                              │
│   • memory: 90% (uptime)                                     │
│   • disk: 100% (df)                                         │
│   • nginx cpu: 45% (ps aux)                                 │
│   • response: 15s (curl)                                     │
│                                                               │
│ Hypotheses (1):                                               │
│   • Runaway nginx worker (confidence: 85%)                  │
│                                                               │
│ Recent (3):                                                   │
│   1. uptime → 3.42 load                                     │
│   2. ps aux → nginx 45% cpu                                  │
│   3. curl → 15s response                                   │
│                                                               │
│ Steps: 3                                                     │
╰───────────────────────────────────────────────────────────────╯
```

### `/compact` Output

```
━━━ COMPACT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Compacting 50 steps → 1 summary

Summary:
"Investigated nginx performance. Found memory at 90%, nginx 
worker at 45% CPU. Response time 15s. Identified root cause: 
runaway worker process. Applied fix: restarted nginx. Result: 
response time 45ms."

→ Context compactified. Use /context to view.
```

---

## Storage Schema

```sql
-- Sessions table (enhanced)
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    goal TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    constraints_json TEXT,
    facts_json TEXT,
    hypotheses_json TEXT,
    insights_json TEXT,
    embedding_id TEXT,
    semantic_tags TEXT,
    compacted_summary TEXT
);

-- Session embeddings for semantic search
CREATE TABLE session_embeddings (
    session_id TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,
    indexed_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

-- Command history with embeddings
CREATE TABLE command_history (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    command TEXT NOT NULL,
    output_text TEXT,
    exit_code INTEGER,
    executed_at TEXT NOT NULL,
    embedding BLOB,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

-- Experience patterns (learned)
CREATE TABLE experience_patterns (
    id INTEGER PRIMARY KEY,
    pattern_type TEXT NOT NULL,
    pattern_text TEXT NOT NULL,
    success_count INTEGER DEFAULT 0,
    failure_count INTEGER DEFAULT 0,
    last_accessed TEXT NOT NULL,
    embedding BLOB
);
```

---

## End-to-End Example

### Session 1: Debugging

```
$ vibe_cli --react "nginx is responding slowly"

━━━ START ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Goal: nginx is responding slowly
Mode: ReAct

━━━ ANALYZE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Intent: Debug (performance issue)
Tools: [process, network, logs]
Constraints: (none detected)

> Actually this is production and no errors in logs

Added constraint: production
Added constraint: no errors in logs

━━━ SUGGESTED ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
uptime && free -h && df -h

→ Allow? y/n> 

(Enter = execute for read-only)

━━━ OUTPUT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Load: 3.42 | Mem: 28GB/31GB (90%) | Disk: 100%

━━━ ANALYZE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
→ Extracted fact: memory=90%
→ Extracted fact: disk=100%
→ High memory pressure detected.
  Next: Check nginx process state

━━━ SUGGESTED ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
ps aux --sort=-cpu | head -20

→ Allow? y/n> y

━━━ OUTPUT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
USER   PID %CPU %MEM COMMAND
root   892 98.0  2.1 nginx: master
www-d 1234 45.2  8.2 nginx: worker
www-d 1235 42.1  7.9 nginx: worker

━━━ ANALYZE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
→ Extracted fact: nginx worker cpu=45%
→ Extracted fact: nginx worker cpu=42%
→ Runaway nginx workers (45% + 42% CPU).
  Hypothesis: Worker stuck in loop (confidence: 85%)
  Next: Test response time

> wait what does 45% cpu mean?

━━━ EXPLAIN ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
45% CPU means the nginx worker process is consuming 45% of 
available CPU resources. For idle web server, this is abnormal.

━━━ SUGGESTED ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
curl -w "\nTime: %{time_total}s\n" -o /dev/null http://localhost/

→ Allow? y/n> 

━━━ OUTPUT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Time: 15.234s

━━━ ANALYZE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
→ Extracted fact: response_time=15.234s
→ Confirmed: Response is extremely slow (15s). 
  Next: Restart nginx to fix immediately

━━━ SUGGESTED ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
sudo systemctl restart nginx

⚠ Will modify system. Confirm? y/n> yes

━━━ OUTPUT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
nginx stopped
nginx started

━━━ VERIFY ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
curl -w "\nTime: %{time_total}s\n" -o /dev/null http://localhost/

→ Allow? y/n> 

━━━ OUTPUT ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Time: 0.045s

━━━ COMPLETE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ Root cause: Runaway nginx workers (45%/42% CPU)
✓ Fix applied: Restarted nginx
✓ Result: Response time 15s → 45ms
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Session saved to semantic index for future retrieval.
```

### Session 2: Using Context Commands

```
$ vibe_cli --react "debug the database connection issue"

[... 25 steps later ...]

> /context

╭─ CONTEXT ────────────────────────────────────────────────╮
│ Goal: debug the database connection issue               │
│ Constraints: (none)                                       │
│                                                        │
│ Facts (6):                                              │
│   • memory: 82% (uptime)                                │
│   • postgres connections: 100/100                        │
│   • disk: 95%                                           │
│   • postgres cpu: 120%                                   │
│   • last error: too many connections                    │
│                                                        │
│ Hypotheses (2):                                          │
│   • Connection pool exhausted (confidence: 92%)        │
│   • Connection leak in app (confidence: 70%)           │
│                                                        │
│ Recent (3):                                             │
│   1. psql -c "SELECT * FROM pg_stat_activity"          │
│   2. grep max_connections /etc/postgresql/             │
│   3. top -bn1 | grep postgres                           │
│                                                        │
│ Steps: 25                                               │
╰─────────────────────────────────────────────────────────╯
```

---

## Implementation Phases

| Phase | Tasks | Files |
|-------|-------|-------|
| **1** | Domain entities | `domain/entities/react.rs` |
| **2** | AnalysisService | `application/services/react_analysis_service.rs` |
| **3** | ContextRetriever | `application/services/context_retriever.rs` |
| **4** | SemanticIndex | `infrastructure/src/semantic_index.rs` |
| **5** | ExperienceLearner | `infrastructure/src/experience_learner.rs` |
| **6** | Update ReactAgentService | `application/services/react_agent_service.rs` |
| **7** | Update ReactHandler | `presentation/cli/handlers/react.rs` |
| **8** | Persistence layer | `infrastructure/src/react_storage.rs` |

---

## Current Infrastructure

| Component | Status | Purpose |
|-----------|--------|---------|
| `EmbeddingStorage` | ✅ Exists | Vector storage (SQLite) |
| `Embedding` | ✅ Exists | Value object with cosine similarity |
| `SearchEngine` | ✅ Exists | Similarity search |
| `ExperienceBuffer` | ✅ Exists | Failure learning |
| `KnowledgeGraph` | ✅ Exists | System entity tracking |
| `LearningService` | ✅ Exists | Failure pattern learning |
| `RagService` | ⚠️ Basic | Uses ripgrep, not semantic |
| `NeurosymbolicService` | ✅ Exists | Command generation |

---

## Gaps to Address

| Gap | Current | Needed |
|-----|---------|--------|
| Session memory | In-memory only | Persistent with embeddings |
| Semantic search | ripgrep-based | True vector embeddings |
| Cross-session learning | Failures only | Full context history |
| Tool output memory | Lost after session | Indexed & searchable |
| Context retrieval | Linear history | Semantic similarity |
| User patterns | Not tracked | Learned over time |

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Separate AnalysisService | Single responsibility, testable |
| SessionMemory not persisted as separate | Runtime only, reduces storage complexity |
| Facts extracted via AI | More flexible than regex patterns |
| User-triggered compact | Matches Claude Code behavior |
| Phase enum in result | Clean UI branching |
| Embedding-based retrieval | Enables semantic search across sessions |

---

## Summary

- **Clean UI**: Simplified labels (ANALYZE, SUGGESTED, OUTPUT)
- **Command Safety**: Read-only auto-executes, destructive requires explicit confirmation
- **Natural Language**: User can redirect, question, clarify at any phase
- **Context Engineering**: Facts, hypotheses, constraints extracted and tracked
- **Semantic Search**: Past sessions indexed and retrievable via embeddings
- **Experience Learning**: Patterns learned from success/failure
- **Professional Features**: /context, /compact, /facts, /hypotheses commands
- **Persistence**: Sessions stored with embeddings for future retrieval

---

## Questions

1. **Embedding model**: Use existing Ollama or external API?
2. **Retention policy**: How long to keep sessions? (default: 30 days)
3. **Privacy**: Allow semantic search across all sessions or user-specific only?
4. **Index updates**: Real-time or batch indexing?
