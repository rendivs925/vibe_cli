# Multi-Type Memory System Plan

## Overview

Design and implement a hierarchical memory system with distinct memory types:
- **Short-term**: Ephemeral, session-scoped working memory
- **Medium-term**: Session-persistent learnings and corrections
- **Long-term**: Persistent across sessions, time-decaying
- **Episodic**: Complete ReAct traces and reasoning journeys
- **Lifetime**: Time-decaying with importance-based retention

---

## Current System Analysis

### Existing Memory Components

| Component | Type | Storage | Purpose |
|-----------|------|---------|---------|
| ExperienceBuffer | Long-term | SQLite | Failures/successes, patterns |
| KnowledgeGraph | Long-term | SQLite | System entities, relationships |
| LearningService | Wrapper | - | RAG-style retrieval |
| CacheManager | Cache | bincode+gzip | Commands, explain, RAG |
| ManpageCache | Cache | SQLite | Parsed man pages |
| Session entity | Ephemeral | In-memory | Conversation history |

### Current Gaps

1. No distinct memory tiers (all mixed together)
2. No time-decay or importance scoring
3. No episodic memory for ReAct traces
4. Session memory lost between invocations
5. No lifetime-based memory decay

---

## Proposed Memory Architecture

```
+---------------------------------------------------------------------+
|                        Memory System                                 |
+---------------------------------------------------------------------+
|                                                                     |
|  +------------------+  +------------------+  +--------------------+ |
|  | SHORT-TERM       |  | MEDIUM-TERM      |  | EPISODIC           | |
|  | (Ephemeral)      |  | (Session)        |  | (ReAct Traces)     | |
|  | - Context        |  | - Corrections    |  | - Goal sequences   | |
|  | - Working memory |  | - Session lessons|  | - Reasoning paths  | |
|  | - Chat history   |  | - User prefs     |  | - Decision points  | |
|  +------------------+  +------------------+  +--------------------+ |
|                                                                     |
|  +------------------+  +------------------+                          |
|  | LONG-TERM        |  | LIFETIME         |                          |
|  | (Persistent)     |  | (Time-decaying)  |                          |
|  | - Command cache  |  | - Success rates  |                          |
|  | - System KG      |  | - Pattern decay  |                          |
|  | - Manpages       |  | - Importance     |                          |
|  +------------------+  +------------------+                          |
|                                                                     |
+---------------------------------------------------------------------+
```

---

## Memory Types

### 1. Short-Term Memory

- Duration: Single CLI invocation
- Capacity: ~50 items
- Access: O(1) hash map
- Persistence: None (cleared on exit)

Contents:
- Current context variables
- Working memory for reasoning
- Chat history
- Active ReAct goal

### 2. Medium-Term Memory

- Duration: Session (multiple invocations, same user)
- Capacity: ~500 items
- Access: SQLite with session_id index
- Persistence: JSON file per session

Contents:
- User corrections ("Use X instead of Y")
- Session lessons
- User preferences
- Tool discoveries

### 3. Long-Term Memory

- Duration: Persistent (forever)
- Capacity: Unlimited
- Access: SQLite with pattern indices
- Persistence: ~/.config/vibe_cli/memory/

Contents:
- Successful commands
- Query patterns
- System knowledge (KnowledgeGraph)
- Manpage cache

### 4. Episodic Memory

- Duration: Persistent with optional TTL
- Capacity: ~1000 episodes
- Access: SQLite with goal/pattern indices
- Persistence: episodes.db

Contents:
- Complete ReAct episodes (goal, steps, outcome)
- Reasoning traces
- Decision points

### 5. Lifetime Memory

- Duration: Time-decaying (configurable)
- Capacity: Dynamic
- Access: SQLite with decay algorithm
- Persistence: memories.db with decay

Decay formula:
```
importance_score = base_score * e^(-decay_rate * age_days)
```

---

## Storage Structure

```
~/.config/vibe_cli/memory/
├── short_term/           # Ephemeral
├── medium_term/          # Session-scoped
│   └── sessions/
│       ├── session_1234567890.json
├── long_term/            # Persistent
│   ├── experience.db
│   ├── knowledge_graph.db
│   └── manpage_cache.db
├── episodic/             # ReAct traces
│   └── episodes.db
└── lifetime/             # Time-decaying
    └── memories.db
```

---

## Implementation Plan

### Phase 1: Memory Infrastructure

1. `domain/src/entities/memory.rs` - Memory type definitions
2. `domain/src/repositories/memory_repository.rs` - Repository interfaces
3. `infrastructure/src/memory/short_term_store.rs`
4. `infrastructure/src/memory/medium_term_store.rs`

### Phase 2: Memory Manager

`application/src/services/memory_manager.rs`:
- Central coordinator for all memory types
- Store/recall across memory types
- Decay cycle management

### Phase 3: Integration

- Update `LearningService` to use new memory system
- Update `ReActAgentService` to store episodes
- Migrate existing data

### Phase 4: CLI Integration

```bash
vibe_cli --memory-stats           # Show statistics
vibe_cli --memory-clear all       # Clear all memory
vibe_cli --memory-decay           # Trigger decay cycle

# In --chat mode
/ch记忆 short                      # Show short-term
/ch记忆 clear                     # Clear memories
```

---

## Importance Scoring

| Event | Initial Score | Type |
|-------|---------------|------|
| Critical failure (5+ times) | 5.0 | Lifetime |
| User correction | 4.0 | Medium-term, Lifetime |
| Successful command | 2.0 | Lifetime |
| Failed command | 3.0 | Lifetime |
| User preference | 3.5 | Medium-term |
| ReAct episode success | 3.0 | Episodic |
| ReAct episode failure | 4.0 | Episodic |

---

## Memory Decay Algorithm

```rust
fn apply_decay(memory: &mut MemoryItem) {
    let age_days = (Utc::now() - memory.created_at).num_days() as f64;
    let decay_factor = (-DECAY_RATE * age_days).exp();
    memory.importance *= decay_factor;

    if memory.importance < MIN_IMPORTANCE {
        memory.mark_for_deletion();
    }
}
```

---

## Implementation Checklist

| Phase | Task |
|-------|------|
| 1 | Memory types and repositories |
| 1 | Short-term and medium-term stores |
| 2 | Memory manager service |
| 2 | Long-term, episodic, lifetime stores |
| 3 | Integrate with LearningService, ReAct |
| 4 | CLI flags and chat commands |

---

## Migration from Existing

1. ExperienceBuffer -> Long-term + Lifetime (add importance scores)
2. KnowledgeGraph -> Long-term (no change)
3. Session entity -> Medium-term (serialize to JSON)
4. CacheManager -> Long-term (migrate to memory dir)
5. ManpageCache -> Long-term (no change)
