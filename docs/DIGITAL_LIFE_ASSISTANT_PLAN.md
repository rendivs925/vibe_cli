# Digital Life Assistant - ReAct Overhaul Plan

## Vision

Transform `--react` from a command generator into a **general-purpose digital assistant** that handles:

- **Coding**: Debug, refactor, analyze, write code
- **Work**: Documents, research, productivity
- **Life**: Information retrieval, learning, planning

---

## Constraints

| Component               | Choice                     | Implementation                          |
| ----------------------- | -------------------------- | --------------------------------------- |
| **Web Search**          | SearXNG only (self-hosted) | User provides URL via `SEARXNG_URL` env |
| **Vector Store**        | SQLite FTS5 + embeddings   | Already in codebase                     |
| **Code Execution**      | User confirmation always   | Prompt before every execution           |
| **Session Persistence** | Auto-save on Ctrl+C        | Signal handler saves state              |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        DIGITAL LIFE ASSISTANT                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     INPUT PROCESSING                                 │    │
│  │   1. Task Classifier (Coding/Research/FileOps/System/General)       │    │
│  │   2. Intent Parser                                                   │    │
│  │   3. Memory Retrieval (lifelong + session)                        │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     PLANNING LAYER                                  │    │
│  │   - Goal Decomposition                                               │    │
│  │   - Dynamic Workflow Generation                                      │    │
│  │   - Replan Triggers (failure/new info/user approval)                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     TOOL EXECUTION                                   │    │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │    │
│  │  │   Code   │ │   Web    │ │ Document │ │  Search  │ │  Memory  │ │    │
│  │  │          │ │          │ │          │ │          │ │          │ │    │
│  │  │ execute  │ │ search   │ │ pdf      │ │ grep     │ │ remember │ │    │
│  │  │ test     │ │ fetch    │ │ docx     │ │ rg       │ │ recall   │ │    │
│  │  │ lint     │ │ summarize│ │ xlsx     │ │ semantic │ │ consolidate │    │
│  │  │ explain  │ │ extract  │ │ extract  │ │ web      │ │ learn    │ │    │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘ │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     MEMORY SYSTEM (3-Tier)                          │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐    │    │
│  │  │ Working     │  │ Session     │  │ Lifelong                │    │    │
│  │  │ (context)   │  │ (SQLite)    │  │ (SQLite + embeddings)  │    │    │
│  │  └─────────────┘  └─────────────┘  └─────────────────────────┘    │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## New Tools to Add (30+ Tools)

### Code Tools (5)

| Tool           | Description                    |
| -------------- | ------------------------------ |
| `code_execute` | Execute code with confirmation |
| `code_test`    | Run tests                      |
| `code_lint`    | Run linters                    |
| `code_diff`    | Analyze git diff               |
| `code_explain` | Explain code                   |

### Web Tools (4)

| Tool            | Description             |
| --------------- | ----------------------- |
| `web_search`    | Search via SearXNG      |
| `web_fetch`     | Fetch URL content       |
| `web_summarize` | Summarize article       |
| `web_extract`   | Extract structured data |

### Document Tools (5)

| Tool             | Description           |
| ---------------- | --------------------- |
| `read_pdf`       | Extract PDF text      |
| `read_docx`      | Extract Word doc text |
| `read_xlsx`      | Read Excel/CSV        |
| `extract_tables` | Extract tables        |
| `doc_qa`         | Q&A on documents      |

### Enhanced Search (4)

| Tool              | Description              |
| ----------------- | ------------------------ |
| `semantic_search` | Vector similarity search |
| `grep_context`    | Grep with context        |
| `search_memory`   | Search lifelong memory   |
| `find_patterns`   | Find code patterns       |

### Memory Tools (4)

| Tool             | Description            |
| ---------------- | ---------------------- |
| `remember`       | Store fact             |
| `recall`         | Retrieve from memory   |
| `consolidate`    | Summarize to long-term |
| `learn_patterns` | Extract patterns       |

---

## Existing Tools (Already Available)

### ReAct Tools (40 tools in 8 categories)

- **Investigation**: suggest_command, suggest_read, suggest_grep, suggest_rag, suggest_discovery
- **Analysis**: summarize, extract_errors, extract_warnings, extract_metrics, extract_patterns, compare, correlate
- **Planning**: plan_next, narrow_focus, branch, rethink, prioritize
- **Action**: apply_fix, edit_file, create_file, run_command, retry
- **Verification**: check_goal, verify_fix, verify_syntax, test_hypothesis
- **Memory**: show_facts, show_hypotheses, show_history, show_context, show_plan, compact_session
- **Resolution**: conclude_success, conclude_fail, escalate, defer
- **Interaction**: ask_clarification, ask_confirmation, explain, suggest_alternatives

### Infrastructure Tools

- **Exploration**: grep, read, rag, fd, ast
- **File Ops**: write, update, remove, replace_block
- **Editing**: sed, awk, perl, apply_patch
- **System**: shell, git, test, build, svc, pkg

### RAG & Embeddings

- Semantic search with vector similarity
- Hybrid search (FTS5 + vector)
- Session/command indexing
- Knowledge graph

---

## Implementation Phases

### Phase 1: Web & Document Tools (Week 1)

#### Dependencies to add:

```toml
# infrastructure/Cargo.toml
scraper = "0.20"      # HTML parsing
pdf-extract = "0.7"  # PDF text extraction
docx-rs = "0.4"     # Word documents
calamine = "0.26"    # Excel/CSV
```

#### Files to create:

```
infrastructure/src/tools/web/
├── mod.rs
├── search.rs        # SearXNG integration
└── fetch.rs        # reqwest + scraper

infrastructure/src/tools/documents/
├── mod.rs
├── pdf.rs          # pdf-extract
├── docx.rs         # docx-rs
└── xlsx.rs         # calmine
```

### Phase 2: Enhanced Code Tools (Week 1-2)

#### Files to modify:

- `infrastructure/src/tools/` - add code execution tools
- `domain/src/entities/react.rs` - add new tool variants

#### New capabilities:

- Sandboxed code execution (with confirmation)
- Test runner integration
- Linter integration

### Phase 3: Task Classifier & Dynamic Planner (Week 2-3)

#### Files to create:

```
application/src/services/react_agent_service/
├── classifier.rs    # Task type detection
└── planner.rs       # Dynamic workflow generation

presentation/src/cli/handlers/react/
└── planning.rs      # Plan visualization
```

#### Task Classification:

1. **Coding**: Debug, implement, refactor, review, explain
2. **Research**: Web search, document analysis, synthesis
3. **FileOps**: Read, write, edit, organize
4. **SystemAdmin**: Service management, monitoring
5. **General**: Q&A, planning, reminders

#### Dynamic Workflow:

```
Input: "Find the bug causing slow API calls"

1. DECOMPOSE
   - Identify subgoals:
     a) Find API endpoint definitions
     b) Identify performance-critical code
     c) Profile/analyze execution
     d) Identify bottleneck

2. PLAN
   Step 1: search_code("API endpoint")
   Step 2: read_file(endpoint_file)
   Step 3: analyze_performance(code)
   Step 4: identify_bottleneck()

3. EXECUTE (with reflection)
   - Execute step → Observe → Reflect
   - Replan if: tool fails, output contradicts, new info
```

### Phase 4: 3-Tier Memory System (Week 3-4)

#### Memory Architecture:

```
TIER 1: WORKING MEMORY (Context Window)
- Current conversation
- Active plan and progress
- Recent facts/hypotheses
- Tool results pending analysis

TIER 2: SESSION MEMORY (SQLite)
- Full session transcript
- All facts learned in session
- Plan variations explored
- Success/failure patterns

TIER 3: LIFELONG KNOWLEDGE (Vector Store)
- Semantic embeddings of all sessions
- Extracted entities and relationships
- Code patterns learned
- User preferences and patterns
- Cross-session solutions
```

#### Files to create:

```
infrastructure/src/memory/
├── mod.rs
├── lifelong.rs       # Vector store integration
├── consolidation.rs  # Session → lifelong
└── retrieval.rs     # Memory retrieval

# Database schema additions:
# - lifelong_knowledge (id, embedding, content, timestamp)
# - entities (id, name, type, properties)
# - relationships (from_id, to_id, relation_type)
# - learned_patterns (id, pattern, success_count)
```

### Phase 5: Auto-Save & Session Resume (Week 4)

#### Files to modify:

- `presentation/src/cli/handlers/react.rs`
- `infrastructure/src/react_storage.rs`

#### New capabilities:

- Signal handler for Ctrl+C
- Session state persistence
- `/resume` command
- `/list-sessions` command

```rust
// Signal handler
ctrlc::set_handler(move || {
    save_current_session();
    println!("\n[Session auto-saved]");
    std::process::exit(0);
});
```

### Phase 6: Integration & Testing (Week 5)

- Full integration test
- End-to-end workflow test
- Memory consolidation test
- Tool discovery test

---

## Session Commands

| Command            | Description                       |
| ------------------ | --------------------------------- |
| `/help`            | Show all commands                 |
| `/context`         | Show reasoning context            |
| `/facts`           | Show learned facts                |
| `/hypotheses`      | Show hypotheses                   |
| `/plan`            | Show current plan                 |
| `/memory <query>`  | Search lifelong memory            |
| `/remember <fact>` | Remember fact                     |
| `/forget <fact>`   | Forget fact                       |
| `/autonomy <mode>` | Set autonomy (manual/guided/auto) |
| `/save`            | Save session explicitly           |
| `/resume [id]`     | Resume session                    |
| `/sessions`        | List saved sessions               |
| `/stats`           | Show session statistics           |

---

## Configuration

```bash
# User sets in .env
SEARXNG_URL=http://localhost:8085

# Storage location
~/.config/vibe_cli/
├── memory.db           # SQLite (session + lifelong)
├── sessions/          # Session transcripts
└── embeddings/        # Embedded vectors (blob)
```

---

## Files Summary

| Category           | Files   | New/Modified |
| ------------------ | ------- | ------------ |
| **Web Tools**      | 3 files | New          |
| **Document Tools** | 4 files | New          |
| **Code Tools**     | 3 files | Modified     |
| **Memory System**  | 4 files | New          |
| **Planning**       | 3 files | New          |
| **CLI Handler**    | 1 file  | Modified     |
| **Agent Service**  | 1 file  | Modified     |

**Total: ~18 files**

---

## Dependencies

```toml
# infrastructure/Cargo.toml (additions)
scraper = "0.20"       # HTML parsing
pdf-extract = "0.7"   # PDF text extraction
docx-rs = "0.4"      # Word documents
calamine = "0.26"      # Excel/CSV
ctrlc = "3.0"         # Signal handling
```

---

## Best Practices Applied

Based on 2025-2026 agentic workflow research:

1. **Plan-Then-Execute**: Separate planning from execution with replan gates
2. **Reflection Loop**: Use deterministic checks, not just self-critique
3. **Spectrum of Control**: User chooses autonomy level
4. **Evaluation-Driven**: Track metrics (success rate, iterations, interventions)
5. **Context Engineering**: Curate relevant info at each step
6. **Memory Tiers**: Short/medium/long-term with consolidation
7. **Tool Discovery**: Dynamic tool selection based on task
8. **Diff-First**: Review every change before execution

---

## Success Metrics

| Metric                 | Target                           |
| ---------------------- | -------------------------------- |
| Task completion rate   | > 80%                            |
| Average iterations     | < 10                             |
| User intervention rate | < 30%                            |
| Memory recall accuracy | > 70%                            |
| Cross-session learning | Measurable improvement over time |

---

## Current vs Target

| Aspect          | Current             | Target                            |
| --------------- | ------------------- | --------------------------------- |
| **Task Type**   | Command generation  | Multi-domain (code/research/docs) |
| **Planning**    | Linear step-by-step | Dynamic with replanning           |
| **Tools**       | 40 static tools     | 60+ discoverable tools            |
| **Memory**      | Session only        | 3-tier lifelong                   |
| **Web**         | Not implemented     | SearXNG search + fetch            |
| **Documents**   | Not implemented     | PDF/DOCX/XLSX                     |
| **Persistence** | Manual              | Auto-save on interrupt            |
| **Control**     | Manual only         | Spectrum (manual/guided/auto)     |

---

## References

- [The Agentic AI Handbook](https://www.nibzard.com/agentic-handbook)
- [Mastering Agentic Workflows - 20 Principles](https://opentyphoon.ai/blog/en/agentic-workflows-principles)
- [SearXNG](https://github.com/searxng/searxng) - Self-hosted metasearch engine

---

_Last Updated: 2026-02-18_
