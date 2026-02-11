# Multi-Type Memory System Plan

## Inspired by Human Brain Memory

The system models how human memory actually works - with decay, reinforcement, emotional weighting, and pattern interference.

---

## Neuroscience Parallels

| Neuroscience Concept | Our Implementation |
|---------------------|-------------------|
| **Synaptic decay** | Importance score decreases over time |
| **Spaced repetition** | Periodic reminders at optimal intervals |
| **Working memory** | Short-term buffer (~7 items) |
| **Long-term potentiation** | Reinforcement on successful recall |
| **Emotional tagging** | Urgency/frustration = higher importance |
| **Memory interference** | Handle conflicting "correct" commands |
| **Consolidation** | Episodic memory stores complete "stories" |
| **Priming** | Pre-load relevant memories by context |

---

## Overview

Design a hierarchical memory system with **4 distinct memory types**:

1. **Short-term**: Ephemeral, working memory (like human working memory)
2. **Medium-term**: Session-persistent learnings with reinforcement
3. **Episodic**: Complete reasoning journeys (autobiographical memory)
4. **Long-term**: Unified persistent store with time-decay, importance, emotional weighting, and spaced repetition

**No duplication** - Long-term handles everything persistent.

---

## Current System Analysis

### Existing Memory Components

| Component | Target Type | Purpose |
|-----------|-------------|---------|
| ExperienceBuffer | Long-term | Failures/successes, patterns |
| KnowledgeGraph | Long-term | System entities, relationships |
| LearningService | Wrapper | RAG-style retrieval |
| CacheManager | Long-term | Commands, explain, RAG |
| ManpageCache | Long-term | Parsed man pages |
| Session entity | Medium-term | Conversation history |

### Current Gaps

1. No distinct memory tiers (all mixed together)
2. No time-decay or importance scoring
3. No episodic memory for ReAct traces
4. Session memory lost between invocations

---

## Proposed Memory Architecture

```
+---------------------------------------------------------------------+
|                        Memory System                                 |
+---------------------------------------------------------------------+
|                                                                     |
|  +------------------+  +------------------+  +--------------------+ |
|  | SHORT-TERM       |  | MEDIUM-TERM      |  | EPISODIC           | |
|  | (Working Memory) |  | (Learning Phase) |  | (Autobiographical) | |
|  | - Context        |  | - Corrections    |  | - Goal sequences   | |
|  | - Working items  |  | - Session lessons|  | - Reasoning paths  | |
|  | - Chat history   |  | - Reinforced     |  | - Emotional tags   | |
|  +------------------+  +------------------+  +--------------------+ |
|                                                                     |
|  +------------------+                                                   |
|  | LONG-TERM        |                                                   |
|  | (Unified Store)  |                                                   |
|  | - Time-decay     |                                                   |
|  | - Importance     |                                                   |
|  | - Emotional wgt  |                                                   |
|  | - Spaced rep     |                                                   |
|  +------------------+                                                   |
|                                                                     |
+---------------------------------------------------------------------+
```

---

## Memory Types

### 1. Short-Term Memory (Working Memory)

Like human working memory (~7 items):

- **Duration**: Single CLI invocation
- **Capacity**: ~7 items (Miller's Law)
- **Access**: O(1) hash map
- **Persistence**: None (cleared on exit)

**Contents:**
- Current context variables
- Active ReAct goal and sub-goals
- Recent chat messages
- Working calculations

**Human Parallel:**
```
"When I'm thinking about a problem, I hold relevant facts in mind."
```

### 2. Medium-Term Memory (Learning Phase)

Memories being consolidated through repetition:

- **Duration**: Session (multiple invocations)
- **Capacity**: ~500 items
- **Access**: SQLite with reinforcement counter
- **Persistence**: Session JSON files

**Contents:**
- User corrections ("Use X instead of Y")
- Session lessons
- User preferences
- Tool discoveries
- **Reinforcement counter**: How many times recalled

**Human Parallel:**
```
"I'm learning this pattern - the more I use it, the stronger it gets."
```

### 3. Episodic Memory (Autobiographical)

Complete "stories" of past experiences:

- **Duration**: Persistent (with emotional weighting)
- **Capacity**: ~1000 episodes
- **Access**: SQLite with goal/success indices
- **Persistence**: episodes.db

**Contents:**
- Complete ReAct episodes
- Reasoning traces
- Decision points
- **Emotional tag**: Frustration level, satisfaction

**Structure:**
```rust
struct Episode {
    id: Uuid,
    goal: String,                   // Original user goal
    steps: Vec<EpisodeStep>,        // What happened
    outcome: EpisodeOutcome,        // Success/failure
    emotional_score: f32,           // -1.0 (frustrated) to 1.0 (satisfied)
    reasoning_trace: String,        // Full "story"
    timestamp: DateTime<Utc>,
}
```

**Human Parallel:**
```
"I remember when I fixed that nginx issue - I had to check the logs first..."
```

### 4. Long-Term Memory (Unified Persistent Store)

All persistent memories unified with:

- **Duration**: Time-decaying with importance
- **Capacity**: Dynamic (old/decayed items removed)
- **Access**: SQLite with decay calculation
- **Persistence**: ~/.config/vibe_cli/memory/long_term.db

**Features:**
- Time-decay (Ebbinghaus curve)
- Importance scoring
- Emotional weighting
- Spaced repetition

**Contents:**
- Successful commands with access counts
- Query patterns with success rates
- System knowledge (KnowledgeGraph)
- Manpage cache
- User preferences (promoted from medium-term)

**Decay Formula (Ebbinghaus):**
```rust
fn retention(memory: &Memory) -> f32 {
    let age_hours = (Utc::now() - memory.last_accessed).num_hours() as f32;
    let stability = memory.stability_score(); // Built through reinforcement
    (-age_hours / stability).exp()
}
```

**Human Parallel:**
```
"The more I practice something, the longer I remember it. But if I don't use it,
 I gradually forget - first the details, then the whole thing."
```

---

## Advanced Brain-Inspired Features

### A. Emotional/Urgency Weighting

Emotions strengthen memories:

```rust
enum EmotionalState {
    Neutral,      // +0.0
    Satisfied,    // +0.5
    Frustrated,   // +1.0 (stronger memory)
    Urgent,       // +1.5 (very strong)
    Confused,     // +0.3 (weaker, unclear)
}

struct EmotionallyWeightedMemory {
    base_importance: f32,
    emotional_multiplier: f32,
    urgency_level: u8,     // 1-5 scale
}
```

**Examples:**
| Scenario | Emotional State | Multiplier |
|----------|-----------------|------------|
| User corrected 5x same mistake | Frustrated | 2.0x |
| Critical command failed | Urgent | 2.5x |
| Clean successful solution | Satisfied | 1.5x |
| Confused user, unclear what worked | Confused | 0.7x |

### B. Memory Interference Handling

When multiple "correct" approaches exist:

```rust
struct InterferenceContext {
    memories: Vec<ConflictingMemory>,
    resolution_strategy: InterferenceStrategy,
}

enum InterferenceStrategy {
    MostRecent,           // Use the newest memory
    MostSuccessful,       // Use highest success rate
    MostAccessed,         // Use most frequently used
    UserPreference,       // Ask user
    Contextual,           // Use based on current context
}
```

**Example:**
```
User asks: "list files"
Memories:
  A. "ls -la" (used 10x, success rate 90%)
  B. "find . -type f" (used 5x, success rate 80%)
  C. "ls -lh" (used 2x, success rate 100%)

Strategy: Contextual (based on "list files" -> use "ls" family)
Result: "ls -la" (most appropriate context)
```

### C. Spaced Repetition Reminders

Suggest forgotten but important memories at optimal intervals:

```rust
struct SpacedRepetitionSchedule {
    memory_id: Uuid,
    next_review: DateTime<Utc>,
    interval_hours: u64,
    ease_factor: f32,
}

fn calculate_next_review(&self, recall_quality: f32) {
    // Based on SM-2 algorithm (used by Anki)
    self.ease_factor = self.ease_factor + (0.1 - (5.0 - recall_quality) * (0.08 + (5.0 - recall_quality) * 0.02));

    if recall_quality < 3.0 {
        self.interval_hours = 1;
    } else if recall_quality < 4.0 {
        self.interval_hours *= 1.2;
    } else {
        self.interval_hours *= self.ease_factor;
    }

    self.next_review = Utc::now() + Duration::hours(self.interval_hours);
}
```

### D. Priming

Pre-load relevant memories based on current context:

```rust
fn prime_memory(context: &Context) -> Vec<MemoryItem> {
    let mut primed = Vec::new();

    // Check current goal/topic
    if let Some(goal) = &context.current_goal {
        primed.extend(long_term_memory.search_by_keywords(goal));
    }

    // Check system context
    let system_type = detect_os_type();
    primed.extend(long_term_memory.get_system_specific(system_type));

    // Check recent patterns
    primed.extend(episodic_memory.get_recent_episodes(5));

    primed.truncate(5);
    primed
}
```

---

## Storage Structure

```
~/.config/vibe_cli/memory/
├── short_term/           # Ephemeral (in-memory only)
├── medium_term/          # Session-scoped (JSON)
│   └── sessions/
│       ├── session_1234567890.json
├── episodic/             # ReAct traces (SQLite)
│   └── episodes.db
└── long_term/            # Unified persistent (SQLite)
    ├── long_term.db      # Commands, patterns, preferences
    ├── knowledge_graph.db # System entities
    ├── manpage_cache.db  # Parsed man pages
    └── spaced_repetition.db # Review schedules
```

---

## Importance Scoring

| Event | Initial Score | Emotional Modifier | Stability |
|-------|---------------|-------------------|-----------|
| Critical failure (5+ times) | 5.0 | Frustrated (2.0x) | High |
| User correction | 4.0 | Neutral | Medium |
| Successful command | 2.0 | Satisfied (1.5x) | Medium |
| Failed command | 3.0 | Confused (0.7x) | Low |
| User preference set | 3.5 | Neutral | Medium |
| Urgent issue | 5.0 | Urgent (2.5x) | High |
| ReAct episode success | 3.0 | Satisfied | High |
| ReAct episode failure | 4.0 | Frustrated | High |

---

## Reinforcement (Long-Term Potentiation)

```rust
fn recall_memory(memory: &mut MemoryItem) {
    // Each successful recall strengthens the memory
    memory.stability += REINFORCEMENT_BOOST; // e.g., 2.0 hours
    memory.last_accessed = Utc::now();
    memory.access_count += 1;

    // Cap stability (max 1 year = 8760 hours)
    memory.stability = memory.stability.min(8760.0);
}

fn should_decay(memory: &MemoryItem) -> bool {
    let retention = memory.retention();
    retention < MIN_RETENTION_THRESHOLD // e.g., 0.1 (10%)
}
```

---

## Implementation Plan

### Phase 1: Memory Types & Infrastructure

1. `domain/src/entities/memory.rs` - Memory type definitions
2. `domain/src/repositories/memory_repository.rs` - Repository interfaces
3. `infrastructure/src/memory/short_term_store.rs` - Working memory
4. `infrastructure/src/memory/medium_term_store.rs` - Session memory

### Phase 2: Long-Term & Episodic

1. `infrastructure/src/memory/long_term_store.rs` - Unified persistent store
2. `infrastructure/src/memory/episodic_store.rs` - Episodes with emotional tags
3. Spaced repetition scheduler

### Phase 3: Advanced Features

1. `application/src/services/memory_manager.rs` - Central coordinator
2. Interference resolution engine
3. Priming system

### Phase 4: Integration & CLI

1. Update `LearningService` to use new memory system
2. Update `ReActAgentService` to store episodes
3. CLI flags for memory management
4. Chat commands for memory interaction

---

## CLI Integration

```bash
# Memory management
vibe_cli --memory-stats           # Show all memory statistics
vibe_cli --memory-clear all       # Clear all memory
vibe_cli --memory-decay           # Trigger decay cycle
vibe_cli --memory-review          # Spaced repetition review

# Chat mode commands
/ch记忆 short                      # Show short-term memory
/ch记忆 medium                    # Show medium-term memory
/ch记忆 episodic                   # Show episodes
/ch记忆 long-term                 # Show long-term with decay
/ch记忆 clear                     # Clear memories
/ch记忆 interference              # Show conflicting memories
/ch记忆 prime                     # Show currently primed memories
```

---

## Human Brain Summary

| Feature | Human Parallel | Benefit |
|---------|---------------|---------|
| Short-term | Working memory | Focused context |
| Medium-term | Learning phase | Gradual consolidation |
| Episodic | Autobiographical memory | Learn from past experiences |
| Long-term | Unified persistent | All knowledge in one place |
| Time-decay | Ebbinghaus forgetting | Adapt to change |
| Emotional | Amygdala tagging | Remember important events |
| Spaced repetition | Optimal learning | Efficient retention |
| Interference | Memory conflicts | Handle ambiguity |
| Priming | Context activation | Faster responses |

---

## Migration from Existing

1. ExperienceBuffer -> Long-term (add stability, emotional tags)
2. KnowledgeGraph -> Long-term (no change)
3. Session entity -> Medium-term (serialize to JSON)
4. CacheManager -> Long-term (migrate to memory dir)
5. ManpageCache -> Long-term (no change)

---

## Why 4 Types (Not 5)

**Simplified from earlier version:**

| Old (Redundant) | New (Simplified) |
|-----------------|------------------|
| Long-term + Lifetime | Long-term (unified) |
| Same semantics, different names | One store handles decay, importance, emotional |

**Benefits:**
- No duplication
- Simpler mental model
- Easier implementation
- Less configuration
