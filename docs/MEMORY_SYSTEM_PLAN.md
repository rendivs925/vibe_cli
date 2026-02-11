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

Design a hierarchical memory system with distinct memory types:
- **Short-term**: Ephemeral, working memory (like human working memory)
- **Medium-term**: Session-persistent learnings with reinforcement
- **Long-term**: Persistent, reinforced memories
- **Episodic**: Complete reasoning journeys (like autobiographical memory)
- **Lifetime**: Time-decaying with importance and emotional weighting

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
5. No human-like reinforcement or interference handling

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
|  +------------------+  +------------------+                          |
|  | LONG-TERM        |  | LIFETIME         |                          |
|  | (Consolidated)   |  | (Ebbinghaus)     |                          |
|  | - Command cache  |  | - Decay curve    |                          |
|  | - System KG      |  | - Spaced rep     |                          |
|  | - Reinforced     |  | - Emotional wgt  |                          |
|  +------------------+  +------------------+                          |
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

### 3. Long-Term Memory (Consolidated)

Fully consolidated memories:

- **Duration**: Persistent (until decay threshold)
- **Capacity**: Unlimited
- **Access**: SQLite with pattern indices
- **Persistence**: ~/.config/vibe_cli/memory/

**Contents:**
- Successful commands
- Query patterns
- System knowledge (KnowledgeGraph)
- Manpage cache
- **Access count**: How many times successfully used

**Human Parallel:**
```
"This is second nature now - I don't even have to think about it."
```

### 4. Episodic Memory (Autobiographical)

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
    goal: String,
    steps: Vec<EpisodeStep>,     // What happened
    outcome: EpisodeOutcome,     // Success/failure
    emotional_score: f32,        // -1.0 (frustrated) to 1.0 (satisfied)
    reasoning_trace: String,     // Full "story"
}
```

**Human Parallel:**
```
"I remember when I fixed that nginx issue - I had to check the logs first..."
```

### 5. Lifetime Memory (Ebbinghaus Curve)

Time-decaying with exact forgetting curve:

```
Forgetting Curve: R = e^(-t/S)
Where:
  R = retention (0-1)
  t = time since learning
  S = stability (strength of memory)
```

- **Duration**: Time-decaying with decay algorithm
- **Capacity**: Dynamic (old items decay out)
- **Access**: SQLite with decay calculation
- **Persistence**: memories.db

**Decay Formula:**
```rust
fn retention(memory: &Memory) -> f32 {
    let age_hours = (Utc::now() - memory.last_accessed).num_hours() as f32;
    let stability = memory.stability_score(); // Built through reinforcement
    (-age_hours / stability).exp()
}
```

**Reinforcement (Spaced Repetition):**
```rust
fn recall_memory(memory: &mut MemoryItem) {
    // Each successful recall strengthens the memory
    memory.stability += REINFORCEMENT_BOOST;
    memory.last_accessed = Utc::now();
    memory.access_count += 1;
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

fn resolve_interference(context: &InterferenceContext) -> MemoryItem {
    match context.resolution_strategy {
        InterferenceStrategy::MostSuccessful => {
            context.memories
                .iter()
                .max_by(|a, b| a.success_rate.partial_cmp(&b.success_rate).unwrap())
                .unwrap()
                .clone()
        }
        // ...
    }
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
        self.interval_hours = 1; // Review soon
    } else if recall_quality < 4.0 {
        self.interval_hours *= 1.2; // Slight increase
    } else {
        self.interval_hours *= self.ease_factor; // Significant increase
    }

    self.next_review = Utc::now() + Duration::hours(self.interval_hours);
}
```

**CLI Integration:**
```bash
# Manual spaced repetition check
vibe_cli --memory-review

# Output might suggest:
# "You haven't used 'journalctl -u nginx' in 30 days.
#  Last success rate: 95%. Review?"
```

### D. Priming

Pre-load relevant memories based on current context:

```rust
fn prime_memory(context: &Context) -> Vec<MemoryItem> {
    let mut primed = Vec::new();

    // Check current goal/topic
    if let Some(goal) = &context.current_goal {
        // Prime memories related to goal
        primed.extend(lifetime_memory.search_by_keywords(goal));
    }

    // Check system context
    let system_type = detect_os_type();
    primed.extend(lifetime_memory.get_system_specific(system_type));

    // Check recent patterns
    primed.extend(episodic_memory.get_recent_episodes(5));

    // Limit to ~5 primed memories
    primed.truncate(5);

    primed
}
```

**Example:**
```
User starts session with: "nginx is crashing"

Primed memories loaded:
- Last nginx episode (3 weeks ago)
- Common nginx commands (systemctl, nginx -t)
- User corrections about nginx (use journalctl)
- System-specific nginx behavior (Ubuntu vs CentOS)
```

---

## Storage Structure

```
~/.config/vibe_cli/memory/
├── short_term/           # Ephemeral (in-memory)
├── medium_term/          # Session-scoped (JSON)
│   └── sessions/
│       ├── session_1234567890.json
├── long_term/            # Persistent (SQLite)
│   ├── experience.db
│   ├── knowledge_graph.db
│   └── manpage_cache.db
├── episodic/             # ReAct traces
│   └── episodes.db
├── lifetime/             # Time-decaying
│   └── memories.db
└── spaced_repetition/    # Review schedules
    └── schedules.db
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

## Decay Algorithm (Ebbinghaus)

```rust
const MIN_RETENTION: f32 = 0.1;
const INITIAL_STABILITY: f32 = 24.0; // Hours

struct LifetimeMemory {
    base_importance: f32,
    stability: f32,           // Hours of retention at this importance
    emotional_multiplier: f32,
    reinforcement_count: u32,
    last_accessed: DateTime<Utc>,
}

impl LifetimeMemory {
    fn current_retention(&self) -> f32 {
        let age_hours = (Utc::now() - self.last_accessed).num_hours() as f32;
        let adjusted_stability = self.stability * self.emotional_multiplier;
        (-age_hours / adjusted_stability).exp()
    }

    fn should_reinforce(&self) -> bool {
        self.current_retention() < REINFORCEMENT_THRESHOLD
    }

    fn apply_decay(&mut self) {
        if self.current_retention() < MIN_RETENTION {
            self.mark_for_deletion();
        }
    }

    fn reinforce(&mut self) {
        // Each reinforcement increases stability (longer retention)
        self.stability *= STABILITY_MULTIPLIER; // e.g., 1.2x
        self.stability = self.stability.min(MAX_STABILITY); // Cap at 8760 hours (1 year)
        self.reinforcement_count += 1;
        self.last_accessed = Utc::now();
    }
}
```

---

## Implementation Plan

### Phase 1: Memory Types & Infrastructure

1. `domain/src/entities/memory.rs` - Memory type definitions with emotional tags
2. `domain/src/repositories/memory_repository.rs` - Repository interfaces
3. `infrastructure/src/memory/short_term_store.rs` - Working memory
4. `infrastructure/src/memory/medium_term_store.rs` - Session memory with reinforcement

### Phase 2: Long-Term & Lifetime

1. `infrastructure/src/memory/long_term_store.rs` - Refactor existing
2. `infrastructure/src/memory/lifetime_store.rs` - Decay algorithm
3. `infrastructure/src/memory/episodic_store.rs` - Episodes with emotional tags

### Phase 3: Advanced Features

1. `application/src/services/memory_manager.rs` - Central coordinator
2. Spaced repetition scheduler
3. Interference resolution engine
4. Priming system

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
vibe_cli --memory-export episodic # Export episodes

# Chat mode commands
/ch记忆 short                      # Show short-term memory
/ch记忆 medium                    # Show medium-term memory
/ch记忆 lifetime                  # Show lifetime with decay
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
| Long-term | Consolidated memory | Permanent knowledge |
| Episodic | Autobiographical memory | Learn from past experiences |
| Lifetime | Ebbinghaus forgetting | Adapt to change |
| Emotional | Amygdala tagging | Remember important events |
| Spaced repetition | Optimal learning intervals | Efficient retention |
| Interference | Memory conflicts | Handle ambiguity |
| Priming | Context activation | Faster responses |

---

## Migration from Existing

1. ExperienceBuffer -> Lifetime (add stability, emotional tags)
2. KnowledgeGraph -> Long-term (no change)
3. Session entity -> Medium-term (serialize to JSON with reinforcement)
4. CacheManager -> Long-term (migrate to memory dir)
5. ManpageCache -> Long-term (no change)

---

## Why This Matters

The system becomes:
- **Adaptive**: Changes with user behavior over time
- **Efficient**: Forgets unused patterns automatically
- **Personal**: Learns user preferences and frustrations
- **Learning**: Stores complete reasoning journeys
- **Human-like**: Feels natural because it works like human memory
