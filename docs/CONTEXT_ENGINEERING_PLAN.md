# Context Engineering Overhaul Plan

## Overview

Transform the current context system to follow the **Context Engineering** template - a structured approach that helps the LLM identify Source of Truth, understand Relationships, and follow Operational Logic without "drifting."

---

## Current State Analysis

### What's Working
- Session memory with facts, hypotheses, constraints, insights
- Context builder with token budget management
- Context retriever with semantic search
- Learning context from past sessions
- Role-based context entries (Goal, User, Constraint, Action, Observation, Summary)

### Issues to Fix

| Issue | Current | Target |
|-------|---------|--------|
| **Session Summary** | None | Project, Environment, Temporal_Anchor |
| **Context Delimiters** | Raw text blocks | XML-style `<doc>` tags |
| **Citations** | None | [REF-XX] notation |
| **Task Placement** | Middle of prompt | END of prompt (recency bias) |
| **Guardrails** | Scattered rules | Explicit operational rules |
| **Metadata** | Limited | Date, source, code, summary |
| **Context Window** | No visibility | Token counting + auto-compact |

---

## The New Context Engineering Template

```
# [[ GLOBAL_INTERFACE ]]

### ## SESSION_SUMMARY
- **Project**: [Project Name or CWD]
- **Environment**: [Production, Development, Research]
- **Temporal_Anchor**: [2026-02-18 14:30:00 UTC]

---

### ## CONTEXT_VAULT
<doc id="REF-01" label="session_history">
[Full session transcript...]
</doc>

<doc id="REF-02" label="latest_output">
[Most recent command output]
</doc>

<doc id="REF-03" label="extracted_facts">
[Fact 1: key=value]
[Fact 2: key=value]
</doc>

<doc id="REF-04" label="hypotheses">
[Hypothesis 1: description (confidence: 0.8)]
</doc>

<doc id="REF-05" label="constraints">
[Constraint 1: key=value]
</doc>

<doc id="REF-06" label="learning_context">
[Past failures to avoid]
</doc>

<doc id="REF-07" label="code_context">
[Relevant code snippets from RAG]
</doc>

<doc id="REF-08" label="context_window">
- Estimated Tokens: 1420
- Window Limit: 8192
- Compact At: 7168
- Utilization: 17.3%
- Status: ok
</doc>

---

### ## OPERATIONAL_GUARDRAILS
1. **Groundedness**: Base all answers strictly on the CONTEXT_VAULT. Do NOT hallucinate.
2. **Traceability**: Cite sources using [REF-XX] notation for every claim.
3. **Recency Priority**: [REF-02] (latest_output) overrides all prior references.
4. **Delta-Only**: For code changes, provide only the diff, not full rewrites.
5. **Loop Prevention**: Do NOT repeat commands from history without new justification.

---

### ## TASK_ORCHESTRATION
**PRIMARY_GOAL**: [The user's task]
**STEP**: [Current step number / total]
**FINAL_COMMAND**: "Execute the next action."

```

---

## Key Differences: Before vs After

### Before (Current)
```
You are a systems debugging assistant using ReAct loop.

## Current Task
debug high memory usage

## Context Window
[...truncated context...]

## Session History - MOST RECENT LAST
Step 1: Analyzed...
Step 2: Ran ps aux...

## Latest Output - ALWAYS USE THIS
PID    USER      COMMAND
1234   nginx     worker process

## Extracted Facts
memory_usage: 95%

## STRICT BEHAVIORAL RULES
- Latest Output Supremacy Rule
- Evidence-Based Reasoning
[...more rules...]

## Instructions
- Use FACTS from the latest output
- Consider user constraints

Output format:
ANALYZE: <reasoning>
```

### After (Context Engineering)
```
# [[ GLOBAL_INTERFACE ]]

### ## SESSION_SUMMARY
- **Project**: /home/rendi/projects/vibe_cli
- **Environment**: Development
- **Temporal_Anchor**: 2026-02-18 14:30:00 UTC

---

### ## CONTEXT_VAULT
<doc id="REF-01" label="session_history">
Step 1: Analyzed...
Step 2: Ran ps aux...
</doc>

<doc id="REF-02" label="latest_output">
PID    USER      COMMAND
1234   nginx     worker process
</doc>

<doc id="REF-03" label="extracted_facts">
- memory_usage: 95% [from REF-02]
- process_count: 247 [from REF-02]
</doc>

<doc id="REF-04" label="hypotheses">
- "nginx worker consuming excessive memory" (confidence: 0.85) [based on REF-02]
</doc>

<doc id="REF-05" label="constraints">
- mode: read_only
</doc>

<doc id="REF-06" label="learning_context">
- Avoid: systemctl restart (failed in session #abc123)
</doc>

<doc id="REF-07" label="context_window">
- Estimated Tokens: 1420
- Window Limit: 8192
- Compact At: 7168
- Utilization: 17.3%
- Status: ok
</doc>

---

### ## OPERATIONAL_GUARDRAILS
1. **Groundedness**: Base all answers strictly on the CONTEXT_VAULT.
2. **Traceability**: Cite sources using [REF-XX] notation.
3. **Recency Priority**: [REF-02] overrides all prior references.
4. **Loop Prevention**: Do NOT repeat commands without new justification.

---

### ## TASK_ORCHESTRATION
**PRIMARY_GOAL**: Debug high memory usage
**STEP**: 3 / 10
**FINAL_COMMAND**: "Analyze and propose next action."

ANALYZE: [reasoning with citations]
```

---

## Implementation Plan

### Phase 1: New Context Types (Week 1)

#### 1.1 Create Context Document Types

**New file:** `domain/src/entities/context_document.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextDocumentType {
    SessionHistory,
    LatestOutput,
    ExtractedFacts,
    Hypotheses,
    Constraints,
    LearningContext,
    CodeContext,
    KnowledgeBase,
    Plan,
    Summary,
    Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDocument {
    pub id: String,  // REF-01, REF-02, etc.
    pub doc_type: ContextDocumentType,
    pub label: String,  // Human-readable label
    pub content: String,
    pub source_ref: Option<String>,  // Where this came from
    pub timestamp: DateTime<Utc>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl ContextDocument {
    pub fn new(id: String, doc_type: ContextDocumentType, label: &str, content: String) -> Self {
        Self {
            id,
            doc_type,
            label: label.to_string(),
            content,
            source_ref: None,
            timestamp: Utc::now(),
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source_ref = Some(source.to_string());
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn to_markdown(&self) -> String {
        let mut output = format!("<doc id=\"{}\" label=\"{}\">\n", self.id, self.label);
        output.push_str(&self.content);
        output.push_str("\n</doc>\n");
        output
    }
}
```

#### 1.2 Update Session Summary

**New file:** `domain/src/entities/session_summary.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Production,
    Development,
    Research,
    Staging,
}

impl Environment {
    pub fn from_env() -> Self {
        if std::env::var("PRODUCTION").is_ok() {
            Environment::Production
        } else {
            Environment::Development
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub project: String,
    pub environment: Environment,
    pub temporal_anchor: DateTime<Utc>,
    pub session_id: String,
    pub iteration: u32,
    pub max_iterations: u32,
    pub task_type: Option<String>,
}

impl SessionSummary {
    pub fn new(task: &str) -> Self {
        Self {
            project: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .to_string_lossy()
                .to_string(),
            environment: Environment::from_env(),
            temporal_anchor: Utc::now(),
            session_id: uuid::Uuid::new_v4().to_string(),
            iteration: 1,
            max_iterations: 10,
            task_type: None,
        }
    }

    pub fn to_markdown(&self) -> String {
        format!(
            r#"### ## SESSION_SUMMARY
- **Project**: {}
- **Environment**: {:?}
- **Temporal_Anchor**: {}
- **Session ID**: {}
- **Progress**: {}/{}

---

"#,
            self.project,
            self.environment,
            self.temporal_anchor.format("%Y-%m-%d %H:%M:%S UTC"),
            self.session_id,
            self.iteration,
            self.max_iterations
        )
    }
}
```

---

### Phase 2: Context Vault Builder (Week 1)

**New file:** `application/src/services/context_vault.rs`

```rust
use domain::entities::context_document::{ContextDocument, ContextDocumentType};

pub struct ContextVault {
    documents: Vec<ContextDocument>,
    ref_counter: u32,
}

impl ContextVault {
    pub fn new() -> Self {
        Self { documents: Vec::new(), ref_counter: 0 }
    }

    pub fn add(&mut self, doc_type: ContextDocumentType, label: &str, content: String) -> String {
        self.ref_counter += 1;
        let id = format!("REF-{:02}", self.ref_counter);
        let doc = ContextDocument::new(id.clone(), doc_type, label, content);
        self.documents.push(doc);
        id
    }

    pub fn add_with_source(&mut self, doc_type: ContextDocumentType, label: &str, content: String, source: &str) -> String {
        let id = self.add(doc_type, label, content);
        if let Some(doc) = self.documents.last_mut() {
            doc.source_ref = Some(source.to_string());
        }
        id
    }

    pub fn get(&self, id: &str) -> Option<&ContextDocument> {
        self.documents.iter().find(|d| d.id == id)
    }

    pub fn update(&mut self, id: &str, content: String) {
        if let Some(doc) = self.documents.iter_mut().find(|d| d.id == id) {
            doc.content = content;
            doc.timestamp = chrono::Utc::now();
        }
    }

    pub fn render(&self) -> String {
        self.documents.iter().map(|d| d.to_markdown()).collect::<Vec<_>>().join("\n")
    }
}
```

---

### Phase 3: Operational Guardrails (Week 1)

**New file:** `application/src/services/operational_guardrails.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalGuardrails {
    pub groundedness: bool,
    pub traceability: bool,
    pub recency_priority: bool,
    pub delta_only: bool,
    pub loop_prevention: bool,
}

impl Default for OperationalGuardrails {
    fn default() -> Self {
        Self { groundedness: true, traceability: true, recency_priority: true, delta_only: false, loop_prevention: true }
    }
}

impl OperationalGuardrails {
    pub fn to_markdown(&self) -> String {
        let mut rules = Vec::new();
        if self.groundedness { rules.push("**Groundedness**: Base all answers strictly on the CONTEXT_VAULT. Do NOT hallucinate.".to_string()); }
        if self.traceability { rules.push("**Traceability**: Cite sources using [REF-XX] notation for every claim.".to_string()); }
        if self.recency_priority { rules.push("**Recency Priority**: Most recent [REF-02] (latest_output) overrides all prior references.".to_string()); }
        if self.delta_only { rules.push("**Delta-Only**: For code changes, provide only the diff, not full rewrites.".to_string()); }
        if self.loop_prevention { rules.push("**Loop Prevention**: Do NOT repeat commands from history without new justification.".to_string()); }

        format!(
            "### ## OPERATIONAL_GUARDRAILS\n{}\n\n---\n\n",
            rules.iter().enumerate().map(|(i, r)| format!("{}. {}", i + 1, r)).collect::<Vec<_>>().join("\n")
        )
    }
}
```

---

### Phase 4: Task Orchestration (Week 1-2)

**New file:** `application/src/services/task_orchestration.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOrchestration {
    pub primary_goal: String,
    pub step: u32,
    pub total_steps: u32,
    pub task_type: String,
    pub final_command: String,
}

impl TaskOrchestration {
    pub fn new(goal: &str, step: u32, total_steps: u32) -> Self {
        Self { primary_goal: goal.to_string(), step, total_steps, task_type: "Analyze".to_string(), final_command: "Execute the next action.".to_string() }
    }

    pub fn with_type(mut self, task_type: &str) -> Self {
        self.task_type = task_type.to_string();
        self
    }

    pub fn to_markdown(&self) -> String {
        format!(
            "### ## TASK_ORCHESTRATION\n**PRIMARY_GOAL**: {}\n**STEP**: {} / {}\n**TASK_TYPE**: {}\n**FINAL_COMMAND**: \"{}\"\n\n",
            self.primary_goal, self.step, self.total_steps, self.task_type, self.final_command
        )
    }
}
```

---

### Phase 5: Complete Context Engineer (Week 2)

**New file:** `application/src/services/context_engineer.rs`

```rust
use crate::services::context_vault::ContextVault;
use crate::services::operational_guardrails::OperationalGuardrails;
use crate::services::task_orchestration::TaskOrchestration;
use domain::entities::context_document::ContextDocumentType;
use domain::entities::session_summary::SessionSummary;
use domain::entities::react_memory::{Fact, Hypothesis};

pub struct ContextEngineer {
    session_summary: SessionSummary,
    context_vault: ContextVault,
    guardrails: OperationalGuardrails,
}

impl ContextEngineer {
    pub fn new(task: &str) -> Self {
        Self { session_summary: SessionSummary::new(task), context_vault: ContextVault::new(), guardrails: OperationalGuardrails::default() }
    }

    pub fn with_iteration(mut self, current: u32, max: u32) -> Self { self.session_summary.iteration = current; self.session_summary.max_iterations = max; self }
    pub fn with_task_type(mut self, task_type: &str) -> Self { self.session_summary.task_type = Some(task_type.to_string()); self }

    pub fn add_session_history(&mut self, content: &str) -> String { self.context_vault.add(ContextDocumentType::SessionHistory, "session_history", content.to_string()) }
    pub fn add_latest_output(&mut self, content: &str, source_command: &str) -> String { self.context_vault.add_with_source(ContextDocumentType::LatestOutput, "latest_output", content.to_string(), source_command) }
    pub fn add_facts(&mut self, facts: &[Fact]) -> String {
        let content = facts.iter().map(|f| format!("- {}: {} [source: {}]", f.key, f.value, f.source_command)).collect::<Vec<_>>().join("\n");
        self.context_vault.add(ContextDocumentType::ExtractedFacts, "extracted_facts", content)
    }
    pub fn add_hypotheses(&mut self, hypotheses: &[Hypothesis]) -> String {
        let content = hypotheses.iter().map(|h| format!("- \"{}\" (confidence: {})", h.description, h.confidence)).collect::<Vec<_>>().join("\n");
        self.context_vault.add(ContextDocumentType::Hypotheses, "hypotheses", content)
    }
    pub fn add_constraints(&mut self, content: &str) -> String { self.context_vault.add(ContextDocumentType::Constraints, "constraints", content.to_string()) }
    pub fn add_learning_context(&mut self, content: &str) -> String { self.context_vault.add(ContextDocumentType::LearningContext, "learning_context", content.to_string()) }
    pub fn add_code_context(&mut self, content: &str, source: &str) -> String { self.context_vault.add_with_source(ContextDocumentType::CodeContext, "code_context", content.to_string(), source) }
    pub fn with_guardrails(mut self, guardrails: OperationalGuardrails) -> Self { self.guardrails = guardrails; self }

    pub fn render(&self, task: &str, step: u32) -> String {
        let orchestration = TaskOrchestration::new(task, step, self.session_summary.max_iterations);
        let mut output = String::new();
        output.push_str("# [[ GLOBAL_INTERFACE ]]\n\n");
        output.push_str(&self.session_summary.to_markdown());
        output.push_str(&self.context_vault.render());
        output.push_str(&self.guardrails.to_markdown());
        output.push_str(&orchestration.to_markdown());
        output
    }
}
```

---

## File Changes Summary

| File | Action |
|------|--------|
| `domain/src/entities/context_document.rs` | **NEW** - Context document types |
| `domain/src/entities/session_summary.rs` | **NEW** - Session summary |
| `application/src/services/context_vault.rs` | **NEW** - Context vault builder |
| `application/src/services/operational_guardrails.rs` | **NEW** - Guardrails |
| `application/src/services/task_orchestration.rs` | **NEW** - Task orchestration |
| `application/src/services/context_engineer.rs` | **NEW** - Main context engineer |
| `application/src/services/react_prompt_service.rs` | **MODIFY** - Use ContextEngineer |

---

## Benefits

| Benefit | Description |
|---------|-------------|
| **Clear Source of Truth** | Every piece of context has a REF-XX ID |
| **Traceability** | LLM must cite sources using [REF-XX] |
| **Recency Bias Fixed** | Task at END where LLM focuses most |
| **Metadata** | Timestamps, source refs, confidence scores |
| **Groundedness** | Explicit rule: don't hallucinate |
| **Loop Prevention** | Explicit rule: don't repeat without justification |

---

*Last Updated: 2026-02-18*
