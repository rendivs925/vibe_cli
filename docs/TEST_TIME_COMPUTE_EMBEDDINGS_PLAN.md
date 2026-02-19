# Test-Time Compute + Embeddings Enhancement Plan

This document outlines the comprehensive plan to enhance Vibe CLI with **test-time compute** as the default behavior, combined with **embeddings-based patterns** for improved reasoning, caching, and agent capabilities.

---

## Executive Summary

The goal is to create a more intelligent CLI that:
1. **Automatically uses test-time compute** without requiring explicit flags
2. **Combines embeddings with RAG** for better context retrieval
3. **Enables multi-agent debate** for complex reasoning
4. **Enhances the ReAct loop** with embedding-guided tool selection
5. **Adds a smart caching layer** with semantic similarity
6. **Implements self-correction** through reflection and confidence scoring

---

## Current State Analysis

| Capability | Status | Key Files |
|-----------|--------|-----------|
| **RAG** | Complete | `rag_service.rs`, `embedder.rs`, `embedding_storage.rs` |
| **ReAct Agent** | Complete | `react_agent_service.rs`, `planner.rs`, tool handlers |
| **Caching** | Basic (file-based) | `cache/mod.rs` |
| **Multi-Agent** | **NOT PRESENT** | N/A |
| **Self-Correction** | Partial (fallback/backtrack) | `neurosymbolic_service.rs`, `learning_service.rs` |
| **Test-Time Compute** | Available but not default | `test_time_scaling.rs` |

---

# Part 1: Make Test-Time Compute Default (Quick Win)

## Current Behavior
- `ScalingConfig::default()` uses `ScalingMethod::None`
- Users must explicitly pass `--scaling-method knockout` or `--scaling-method league`

## Changes Required

### File: `application/src/services/test_time_scaling.rs`

```rust
// Line 17: Change from None to Knockout
impl Default for ScalingConfig {
    fn default() -> Self {
        Self {
            method: ScalingMethod::Knockout,  // Changed from None
            num_samples: 6,
            comparisons_per_pair: 3,
            opponents_per_candidate: 5,
            early_stopping: true,  // Changed from false
            confidence_threshold: 0.85,  // Changed from 0.9
        }
    }
}
```

### File: `presentation/src/cli/cli_app.rs`

Update the scaling config construction (lines 162-169) to use sensible defaults when flags aren't provided:

```rust
let scaling_config = ScalingConfig {
    method: cli.scaling_method.into(),
    num_samples: cli.samples.unwrap_or(6),
    comparisons_per_pair: cli.comparisons.unwrap_or(3),
    opponents_per_candidate: cli.opponents.unwrap_or(5),
    early_stopping: cli.early_stop.unwrap_or(true),  // Default to true
    confidence_threshold: 0.85,
};
```

---

# Part 2: Implementation Plan for 6 Combinations

## 1. Embeddings + RAG (Retrieval Augmented Generation)

### Current State
- Chunk size: 500-2000 characters
- Top-K: Uses raw cosine similarity
- No reranking

### Best Practices
- Chunk size: 512-1024 tokens
- Top-K: 3-5 most relevant
- Rerank: Add cross-encoder reranker for better results

### Implementation

```
New Files:
├── infrastructure/src/reranker.rs           # Cross-encoder reranker
├── infrastructure/src/hybrid_search.rs     # Keyword + semantic search
└── Update rag_service.rs → integrate reranking
```

### Key Changes

1. **Add Cross-Encoder Reranker** (`reranker.rs`)
   - After initial cosine similarity retrieval, re-rank top-20 results
   - Use cross-encoder model for accurate scoring
   - Return top-5 final results

2. **Add Hybrid Search** (`hybrid_search.rs`)
   - Combine FTS5 (keyword) with vector search
   - Weighted scoring: 60% semantic + 40% keyword
   - Better recall for technical terms

3. **Optimize Chunk Size**
   - Target: 512-1024 characters per chunk
   - Overlap: 50-100 characters for context continuity

---

## 2. Embeddings + Multi-Agent Debate

### Current State
- Single-agent ReAct flow only
- No agent-to-agent communication

### Implementation

```
New Files:
├── application/src/services/multi_agent/
│   ├── mod.rs
│   ├── agent.rs              # Base agent trait
│   ├── generator_agent.rs   # Generates candidates
│   ├── critic_agent.rs       # Evaluates/flags issues
│   ├── tester_agent.rs       # Tests/validates
│   ├── consensus.rs          # Voting mechanism
│   └── debate_manager.rs    # Orchestrates debate
└── Update react_agent_service.rs → use multi-agent
```

### Architecture

```
              ┌──────────────┐
              │   Query     │
              └──────┬───────┘
                     │
          ┌──────────┼──────────┐
          ▼          ▼          ▼
     ┌────────┐ ┌────────┐ ┌────────┐
     │ Qwen A │ │ Qwen B │ │ Qwen C │
     │ (Gen)  │ │ (Crit) │ │ (Test) │
     └───┬────┘ └───┬────┘ └───┬────┘
         │           │           │
         └───────────┼───────────┘
                     │
                     ▼
              ┌──────────────┐
              │   Consensus  │
              │   (Voting)   │
              └──────┬───────┘
                     │
                     ▼
               Final Answer
```

### Agent Roles

1. **Generator Agent**: Produces N candidate solutions
2. **Critic Agent**: Evaluates each, identifies flaws, suggests improvements
3. **Tester Agent**: Validates against constraints, checks safety
4. **Consensus Module**: Aggregates votes, selects best answer

### With Embeddings
- Each agent retrieves relevant memories
- Agents cite past solutions
- More informed debate

---

## 3. Embeddings + Tool Execution Loop (ReAct)

### Current State
- ReAct loop with basic context retrieval
- Limited memory of past tool executions

### Implementation

```
Updates:
├── application/src/services/react_agent_service.rs
│   └── Add: embed tool results for future context
├── application/src/services/react_tools/handlers/memory.rs
│   └── Add: "remember" tool stores embeddings
└── infrastructure/src/session_indexing_service.rs
    └── Already exists - integrate into ReAct loop
```

### Key Changes

1. **After tool execution**, embed the result → store for future retrieval
2. **Tool selection**: embed current state → find similar past states → use that tool
3. **Creates memory of "what was tried"** to avoid repeating失败的工具调用

### ReAct Loop with Embeddings

```
┌─────────────────────────────────────────────────────────────┐
│                    ReAct LOOP                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Query ─► Embed ─► Retrieve ─► Add to context          │
│                           │                                  │
│                           ▼                                  │
│  2. Qwen: "I'll use Read tool"                             │
│                           │                                  │
│                           ▼                                  │
│  3. Execute tool ─► Get result                              │
│                           │                                  │
│                           ▼                                  │
│  4. Embed result ─► Store in semantic index ─► Add to context│
│                           │                                  │
│                           ▼                                  │
│  5. Repeat until done                                      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Embeddings + Caching Layer

### Current State
- Basic file-based cache with JSON storage
- Semantic similarity matching (threshold: 0.7)
- TTL: 7 days

### Implementation

```
New Files:
├── infrastructure/src/embedding_cache.rs    # Vector cache layer
└── Update cache/mod.rs → integrate embeddings
```

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    SMART CACHE                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  User Query ──► Embed ──► Search Cache                     │
│                      │                                      │
│            ┌───────┴───────┐                               │
│            ▼               ▼                               │
│      [HIT]              [MISS]                              │
│       │                   │                                 │
│       ▼                   ▼                                 │
│  Return cached      Call Qwen +                             │
│  response          Store in cache                           │
│                           │                                 │
│                           ▼                                 │
│                    Embed response +                          │
│                    Store in Vector DB                        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Key Changes

1. **Cache queries as embeddings** in SQLite
2. **On query**: compute embedding → cosine similarity → hit/miss
3. **TTL-based eviction** with semantic deduplication
4. **Cache analytics**: track hit rates, popular queries

### Use Cases
- Same question → instant answer
- Similar bug → reuse fix
- Common patterns → pre-cached

---

## 5. Embeddings + Self-Correction Loop

### Current State
- Basic fallback + backtracking in neurosymbolic service
- Learning service tracks failed commands
- Limited explicit reflection

### Implementation

```
New Files:
├── application/src/services/
│   ├── reflection_service.rs     # Self-reflection
│   └── confidence_scorer.rs      # Confidence scoring
└── Updates:
    └── react_agent_service.rs → add reflection step
```

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  SELF-CORRECTION                            │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Qwen generates ─► Check against embeddings                 │
│       │                  │                                  │
│       │                  ▼                                  │
│       │           [Does this match                          │
│       │            past solutions?]                          │
│       │                  │                                  │
│       ▼                  ▼                                  │
│  ┌─────────┐      ┌──────────┐                             │
│  │Matches  │      │ No match │                             │
│  │ patterns │      │ → Flag   │                             │
│  └────┬────┘      └────┬─────┘                             │
│       │                 │                                    │
│       └────────┬────────┘                                    │
│                ▼                                             │
│         Refine if needed                                     │
│                │                                             │
│                ▼                                             │
│          Final Answer                                        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Key Changes

1. **Reflection Step**: After generating response, ask "Does this match past solutions?"
2. **Confidence Scoring**: Embed response → compare to knowledge base → flag low confidence
3. **If low confidence**: Re-generate with feedback
4. **Citation Verification**: Ensure all claims have evidence

---

## 6. Complete Architecture (All Combined)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      ULTIMATE QWEN SYSTEM                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                      EMBEDDING LAYER                               │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────┐   │  │
│  │  │  Query     │  │  Memory   │  │   Code    │  │ Cache  │   │  │
│  │  │  Encoder   │  │   Index    │  │   Index   │  │ Index  │   │  │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────┘   │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    RETRIEVAL LAYER                                 │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐               │  │
│  │  │  Memory    │  │  Similar   │  │  Rerank   │               │  │
│  │  │  Search    │  │  Files     │  │  (cross)  │               │  │
│  │  └────────────┘  └────────────┘  └────────────┘               │  │
│  │  ┌────────────────────────────────────────────┐                │  │
│  │  │  Hybrid Search (FTS5 + Vector)            │                │  │
│  │  └────────────────────────────────────────────┘                │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    AGENT LAYER (Multi-Qwen)                     │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐               │  │
│  │  │  Planner   │  │  Executor  │  │  Critic   │               │  │
│  │  └────────────┘  └────────────┘  └────────────┘               │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    TOOL EXECUTION                                │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐               │  │
│  │  │   Shell    │  │   Read     │  │   Write    │               │  │
│  │  └────────────┘  └────────────┘  └────────────┘               │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                    │                                    │
│                                    ▼                                    │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                    CACHE LAYER                                   │  │
│  │  ┌──────────────────────────────────────────────────────────┐   │  │
│  │  │  Vector Cache  +  Semantic Deduplication + TTL        │   │  │
│  │  └──────────────────────────────────────────────────────────┘   │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

# Implementation Roadmap

| Phase | Task | Effort | Files to Modify |
|-------|------|--------|-----------------|
| **1** | Make test-time compute default | Low | `test_time_scaling.rs`, `cli_app.rs` |
| **2** | Embeddings + Cache Layer | Medium | New: `embedding_cache.rs`, update `cache/mod.rs` |
| **3** | RAG Enhancements (rerank + hybrid) | Medium | New: `reranker.rs`, `hybrid_search.rs`, update `rag_service.rs` |
| **4** | Embeddings + ReAct Loop | Medium | Update `react_agent_service.rs`, `session_indexing_service.rs` |
| **5** | Multi-Agent Debate | High | New: `multi_agent/` directory |
| **6** | Self-Correction Loop | Medium | New: `reflection_service.rs`, `confidence_scorer.rs` |
| **7** | Complete Integration | Medium | Wire everything together |

---

# Recommended Priority Order

| Priority | Combination | Impact | Effort |
|----------|------------|--------|--------|
| 1 | Test-Time Compute Default | High | Low |
| 2 | Embeddings + RAG | High | Medium |
| 3 | Embeddings + Cache | High | Low |
| 4 | Embeddings + ReAct | High | Medium |
| 5 | Multi-Agent | Very High | High |
| 6 | Self-Correction | High | Medium |

---

# Key Dependencies

- **Ollama Client**: Already exists for embeddings and LLM calls
- **SQLite**: Already used for storage (embeddings, sessions, cache)
- **Semantic Index**: Already exists in `session_indexing_service.rs`
- **ReAct Agent**: Already exists with tool registry
- **Learning Service**: Already tracks failed commands

---

# Backward Compatibility

All changes should maintain backward compatibility:
1. Existing CLI flags should continue to work
2. Default behavior changes are additive
3. New features can be disabled via config if needed

---

# Testing Strategy

1. **Unit Tests**: Test individual components (reranker, hybrid search)
2. **Integration Tests**: Test the full pipeline
3. **A/B Testing**: Compare default vs enhanced behavior
4. **User Feedback**: Gather feedback on response quality

---

*Document created: 2026-02-20*
*Last updated: 2026-02-20*
