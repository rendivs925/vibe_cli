# ReAct Agent Implementation Plan

## Overview

Implement ReAct (Reasoning + Acting) for the agentic CLI with optional neurosymbolic enhancement.

- **Default**: Pure ReAct with LLM reasoning
- **With --neurosymbolic**: Use domain operations if available, otherwise fall back to pure ReAct

---

## CLI Interface

```bash
# ReAct mode with neurosymbolic
vibe_cli --react --neurosymbolic "nginx is not running, diagnose and fix"

# Chat mode with neurosymbolic
vibe_cli --chat --neurosymbolic
# User: nginx is crashing, help me debug
# Bot uses domain operations for structured debugging

# Agent mode with neurosymbolic
vibe_cli --agent --neurosymbolic "check all services and restart failed ones"

# Query mode with neurosymbolic (default query)
vibe_cli --neurosymbolic "list processes by memory usage"
```

---

## Interactive Iteration

User can iterate on the prompt during a ReAct session:

```
ReAct session for: "nginx is not responding"

Available commands during session:
  /revise <new_goal>  - Update the goal mid-session
  /context            - Show current reasoning context
  /retry              - Retry last action
  /skip               - Skip to next step
  /abort              - End session
  /help               - Show commands

User input: /revise "actually, check if port 443 is open too"
-> Updates goal, ReAct continues with both ports 80 and 443
```

---

## Confirmation Flow

Always ask user before executing each action:

```
--- STEP 2: ACTION ---

Thought: I found nginx is not running. I need to check the logs to
         understand why it failed before restarting.

Proposed command:
  bash: journalctl -u nginx --no-pager -n 50

Execute this command? [Y/n/a(ll)]:

Options:
  Y - Execute this step
  n - Skip this step, continue to next
  a - Execute all remaining steps without asking
  r - Revise the command
  /help - Show help
```

---

## Architecture: Shared Neurosymbolic Service

The key insight: **--neurosymbolic is a reusable capability, not a mode**.

```
+---------------------------------------------------------------------+
|                         CLI Handlers                                 |
+---------------------------------------------------------------------+
|                                                                     |
|   handle_chat()  --->  Uses NeurosymbolicService if --neurosymbolic |
|   handle_agent() --->  Uses NeurosymbolicService if --neurosymbolic |
|   handle_react() --->  Uses NeurosymbolicService if --neurosymbolic |
|   handle_query() --->  Uses NeurosymbolicService if --neurosymbolic |
|                                                                     |
+---------------------------------------------------------------------+
            |
            | Uses shared service
            v
+---------------------------------------------------------------------+
|                   NeurosymbolicService                              |
+---------------------------------------------------------------------+
|                                                                     |
|   - match_operation(query) -> Option<Operation>                     |
|   - generate_commands(operation, inputs) -> Vec<String>             |
|   - apply_inference_rules(facts) -> Vec<Conclusion>                 |
|   - find_troubleshooting_pattern(symptoms) -> Option<Pattern>       |
|                                                                     |
+---------------------------------------------------------------------+
            |
            | Delegates to
            v
+---------------------------------------------------------------------+
|                    DomainRegistry (existing)                         |
+---------------------------------------------------------------------+
```

---

## Mode Selection Logic

```rust
// If --neurosymbolic is specified:
if domain operations available for task:
    use domain operations
else:
    show warning and fall back to pure mode (whatever mode user is in)

// If --neurosymbolic is NOT specified:
use pure mode (ReAct, chat, agent, or query)
```

---

## Global --neurosymbolic Flag

**File:** `presentation/src/cli/cli_app.rs`

```rust
#[derive(Parser)]
pub struct Cli {
    /// Enable neurosymbolic mode (use domain operations when available)
    #[arg(long)]
    pub neurosymbolic: bool,

    /// ReAct agent with iterative reasoning
    #[arg(long)]
    pub react: bool,

    /// Interactive chat mode
    #[arg(long)]
    pub chat: bool,

    /// Multi-step agent (one-shot planning)
    #[arg(long)]
    pub agent: bool,

    /// Max ReAct iterations (default: 30)
    #[arg(long, value_name = "N", requires = "react")]
    pub max_iterations: Option<usize>,

    /// Show full reasoning trace
    #[arg(long, requires = "react")]
    pub show_thoughts: bool,

    /// Task description
    #[arg(trailing_var_arg = true)]
    pub task: Vec<String>,
}
```

**Usage across all modes:**

```bash
# All modes can use --neurosymbolic
vibe_cli --neurosymbolic "list processes"                    # Query mode
vibe_cli --agent --neurosymbolic "check and fix nginx"       # Agent mode
vibe_cli --chat --neurosymbolic                              # Chat mode
vibe_cli --react --neurosymbolic "debug high cpu"            # ReAct mode
```

---

## Reusable Neurosymbolic Service

**File:** `application/src/services/neurosymbolic_service.rs` (enhanced)

```rust
/// Reusable neurosymbolic capability for all CLI modes
pub struct NeurosymbolicCapability {
    registry: Option<DomainRegistry>,
    command_generator: CommandGenerator,
    inference_engine: InferenceEngine,
}

impl NeurosymbolicCapability {
    /// Create new capability (no-op if no domains loaded)
    pub fn new() -> Self {
        Self {
            registry: DomainRegistry::load().ok(),
            command_generator: CommandGenerator::new(),
            inference_engine: InferenceEngine::new(),
        }
    }

    /// Check if domain operations are available
    pub fn is_available(&self) -> bool {
        self.registry.is_some()
    }

    /// Match query to domain operation
    pub fn match_operation(&self, query: &str) -> Option<ResolvedOperation> {
        self.registry.as_ref()?.match_operation(query)
    }

    /// Generate commands from operation
    pub fn generate_commands(
        &self,
        op_id: &str,
        inputs: &HashMap<String, Value>,
    ) -> Result<Vec<GeneratedCommand>> {
        let registry = self.registry.as_ref().context("No domain registry")?;
        let operation = registry.get_operation(op_id)?.1;
        Ok(registry.command_generator().generate(operation, inputs))
    }

    /// Apply inference rules to observed facts
    pub fn apply_inference_rules(&self, facts: &[Fact]) -> Vec<Conclusion> {
        self.registry.as_ref().map_or(Vec::new(), |r| {
            r.get_inference_rules()
                .iter()
                .filter(|rule| rule.evaluate(facts))
                .map(|rule| rule.conclusion.clone())
                .collect()
        })
    }

    /// Find troubleshooting pattern for symptoms
    pub fn find_pattern(&self, symptoms: &[Symptom]) -> Option<TroubleshootingPattern> {
        self.registry.as_ref()?.find_pattern(symptoms)
    }
}
```

---

## How Each Mode Uses Neurosymbolic

### 1. Query Mode (default)

```rust
// In handle_query()
if cli.neurosymbolic {
    if let Some(op) = self.neuro.match_operation(query) {
        let commands = self.neuro.generate_commands(&op.op_id, &op.inputs)?;
        // Present commands to user, ask confirmation, execute
    }
}
```

### 2. Agent Mode (one-shot planning)

```rust
// In handle_agent()
if cli.neurosymbolic {
    // When generating plan steps, try domain operations first
    for step in plan.steps {
        if let Some(op) = self.neuro.match_operation(&step.description) {
            step.commands = self.neuro.generate_commands(&op.op_id, &op.inputs)?;
        }
    }
}
```

### 3. Chat Mode

```rust
// In handle_chat()
if cli.neurosymbolic {
    // Parse user intent, suggest domain operations
    if let Some(op) = self.neuro.match_operation(&user_input) {
        bot.reply(format!("I can help with that using domain operation: {}", op.op_id));
        // Offer to execute the operation
    }
}
```

### 4. ReAct Mode (iterative)

```rust
// In handle_react() - NEW
if cli.neurosymbolic {
    // In each iteration, try domain operations before LLM
    if let Some(op) = self.neuro.match_operation(&context.goal) {
        return Action::DomainOperation(op);
    }
}
// Fall back to LLM-generated action
self.generate_llm_action(context).await
```

---

## Architecture Components

### Domain Layer

**File:** `domain/src/entities/react.rs`

```rust
pub enum ReActStepType {
    Thought,
    Action,
    Observation,
    Reflection,
    FinalAnswer,
}

pub struct ReActStep {
    pub step_number: usize,
    pub step_type: ReActStepType,
    pub content: String,
    pub confidence: f32,
    pub was_successful: Option<bool>,
}

pub struct ReActTrace {
    pub id: Uuid,
    pub original_goal: String,
    pub current_goal: String,
    pub steps: Vec<ReActStep>,
    pub final_result: Option<String>,
    pub success: bool,
    pub total_iterations: usize,
}

pub enum Tool {
    Bash { command: String },
    DomainOperation {
        op_id: String,
        parameters: HashMap<String, Value>,
        generated_commands: Vec<String>,
    },
}

pub struct ReActConfig {
    pub max_iterations: usize,
    pub show_thoughts: bool,
    pub auto_execute_all: bool,
    pub use_neurosymbolic: bool,
    pub confidence_threshold: f32,
}

impl Default for ReActConfig {
    fn default() -> Self {
        Self {
            max_iterations: 30,
            show_thoughts: true,
            auto_execute_all: false,
            use_neurosymbolic: false,
            confidence_threshold: 0.7,
        }
    }
}
```

**File:** `domain/src/repositories/react_repository.rs`

```rust
pub trait ReActRepository {
    fn save_trace(&self, trace: &ReActTrace) -> Result<()>;
    fn find_similar_traces(&self, goal: &str, limit: usize) -> Result<Vec<ReActTrace>>;
    fn get_success_rate(&self, goal_pattern: &str) -> Result<f32>;
}
```

### Application Layer

**File:** `application/src/services/react_agent_service.rs`

```rust
pub struct ReActAgentService {
    ollama_client: OllamaClient,
    repository: Box<dyn ReActRepository>,
    safety_engine: SafetyEngine,
    neuro: Option<NeurosymbolicCapability>,  // Shared capability
    config: ReActConfig,
}

impl ReActAgentService {
    pub async fn run(&mut self, goal: &str) -> Result<ReActResult> {
        let mut trace = ReActTrace::new(goal);
        let mut context = ReActContext::new(goal);

        for iteration in 0..self.config.max_iterations {
            let thought = self.generate_thought(&context).await?;
            context.add_thought(&thought);

            if self.is_goal_achieved(&context) {
                trace.final_result = Some(context.get_summary());
                trace.success = true;
                break;
            }

            // Try neurosymbolic first if enabled
            let action = if self.config.use_neurosymbolic {
                self.generate_action(&context, &thought).await?
            } else {
                self.generate_llm_action(&context).await?
            };

            let validation = self.validate_action(&action).await?;
            if !validation.is_safe {
                context.add_reflection(&format!("Action blocked: {}", validation.reason));
                continue;
            }

            return Ok(ReActResult::pending_action(action, context.clone()));
        }

        ReActResult::from_trace(trace)
    }

    async fn generate_action(&self, context: &ReActContext, thought: &str) -> Result<Action> {
        if let Some(ref neuro) = self.neuro {
            if let Some(operation) = neuro.match_operation(&context.goal) {
                let commands = neuro.generate_commands(&operation.op_id, &operation.inputs)?;
                return Ok(Action::DomainOperation {
                    op_id: operation.op_id,
                    parameters: operation.inputs,
                    generated_commands: commands.into_iter().map(|c| c.command).collect(),
                });
            }
        }
        self.generate_llm_action(context).await
    }
}
```

### Presentation Layer

**File:** `presentation/src/cli/handlers/react.rs`

```rust
impl CliHandlers {
    pub async fn handle_react(&mut self, task: &str, cli: &Cli) -> Result<()> {
        let config = ReActConfig {
            max_iterations: cli.max_iterations.unwrap_or(30),
            show_thoughts: cli.show_thoughts,
            use_neurosymbolic: cli.neurosymbolic,
            ..Default::default()
        };

        let neuro = if cli.neurosymbolic {
            Some(NeurosymbolicCapability::new())
        } else {
            None
        };

        let mut service = ReActAgentService::new(
            self.ollama_client()?,
            neuro,
            config,
        )?;

        // Warning if neurosymbolic requested but not available
        if cli.neurosymbolic && !service.has_domain_operations() {
            println!("[Warning: No domain operations available, using pure ReAct]");
        }

        // ReAct loop with user interaction...
    }
}
```

**File:** `presentation/src/cli/handlers/mod.rs` (enhance existing handlers)

```rust
impl CliHandlers {
    /// Enhanced query handler with neurosymbolic support
    pub async fn handle_query(&mut self, query: &str, cli: &Cli) -> Result<()> {
        if cli.neurosymbolic {
            if let Some(operation) = self.neuro()?.match_operation(query) {
                // Use domain operation
                return self.execute_domain_operation(query, &operation).await;
            }
        }
        // Fall back to standard query handling
        self.handle_query_standard(query).await
    }

    /// Enhanced agent handler with neurosymbolic support
    pub async fn handle_agent(&mut self, task: &str, cli: &Cli) -> Result<()> {
        if cli.neurosymbolic {
            // When generating plan, use domain operations where possible
            self.generate_plan_with_neurosymbolic(task).await
        } else {
            self.generate_plan_llm(task).await
        }
    }

    /// Enhanced chat handler with neurosymbolic support
    pub async fn handle_chat(&mut self, cli: &Cli) -> Result<()> {
        if cli.neurosymbolic {
            // Enable domain operation suggestions in chat
            self.chat_with_neurosymbolic().await
        } else {
            self.chat_standard().await
        }
    }

    /// Helper to get shared neurosymbolic capability
    fn neuro(&self) -> Result<NeurosymbolicCapability> {
        NeurosymbolicCapability::new()
            .ok_or_else(|| anyhow!("Failed to initialize neurosymbolic capability"))
    }
}
```

---

## Storage

**File:** `infrastructure/src/react_storage.rs`

```rust
pub struct ReActStorage {
    db: rusqlite::Connection,
}

impl ReActStorage {
    pub fn new(path: PathBuf) -> Result<Self> {
        let db = rusqlite::Connection::open(path)?;
        db.execute("
            CREATE TABLE IF NOT EXISTS react_traces (
                id TEXT PRIMARY KEY,
                original_goal TEXT NOT NULL,
                current_goal TEXT NOT NULL,
                steps TEXT NOT NULL,
                final_result TEXT,
                success INTEGER NOT NULL,
                total_iterations INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )
        ")?;
        Ok(Self { db })
    }
}

impl ReActRepository for ReActStorage {
    fn save_trace(&self, trace: &ReActTrace) -> Result<()> { /* ... */ }
    fn find_similar_traces(&self, goal: &str, limit: usize) -> Result<Vec<ReActTrace>> { /* ... */ }
}
```

---

## Session Commands

| Command | Description |
|---------|-------------|
| `/revise "new goal"` | Update the goal mid-session |
| `/context` | Show current reasoning context |
| `/retry` | Retry last failed action |
| `/skip` | Skip to next step |
| `/abort` | End session |
| `/help` | Show available commands |

---

## Confirmation Summary

| Action | User Asked? |
|--------|-------------|
| Execute each command | YES - every step |
| Revise goal mid-session | YES - `/revise` command |
| Retry skipped step | YES - `/retry` command |
| Apply learning from trace | YES - at end of session |
| Edit/revise generated command | YES - `r` option |

---

## Sample Output

```
$ vibe_cli --react --neurosymbolic "nginx is crashing randomly"

--- STEP 1: THOUGHT ---
User reports nginx crashing randomly. This matches troubleshooting
pattern "service_unstable". I'll use structured diagnostics.

--- STEP 2: ACTION (Domain Operation: check_service) ---
Operation: check_service {service: "nginx"}
Generated commands:
  1. systemctl status nginx
  2. journalctl -u nginx --no-pager -n 100
  3. cat /var/log/nginx/error.log | tail -50

Execute? [Y/n/r/g/a/x]: Y

--- OBSERVATION ---
Active: active (running)
Last crash: 2 hours ago
Error log shows: "worker process exited unexpectedly"

--- STEP 3: SYMBOLIC INFERENCE ---
Rule: worker_crash
Conditions:
  - service active but workers crashing
Conclusion: Worker process restart loop, likely configuration issue

--- GOAL ACHIEVED ---
Root cause: File descriptor limit too low
Fix applied: worker_rlimit_nofile increased to 4096
Iterations: 7
Mode: ReAct + Neurosymbolic
```

---

## Implementation Checklist

| Phase | Task | Files |
|-------|------|-------|
| 1 | Create NeurosymbolicCapability | `application/src/services/neurosymbolic_capability.rs` |
| 2 | Domain types | `domain/src/entities/react.rs`, `domain/src/repositories/react_repository.rs` |
| 3 | ReAct agent service | `application/src/services/react_agent_service.rs` |
| 4 | ReAct CLI handler | `presentation/src/cli/handlers/react.rs` |
| 5 | Update CLI app | `presentation/src/cli/cli_app.rs` |
| 6 | Update existing handlers | `presentation/src/cli/handlers/mod.rs` |
| 7 | Storage | `infrastructure/src/react_storage.rs` |
| 8 | Integration | Connect OllamaClient, SafetyEngine, DomainRegistry |
| 9 | Tests | Unit + integration tests |

---

## Integration Points

| Component | Role |
|-----------|------|
| `NeurosymbolicCapability` | Reusable service used by all modes |
| `DomainRegistry` | Match task to operations, generate commands |
| `CommandGenerator` | Generate from templates, score alternatives |
| `InferenceEngine` | Apply rules to observations |
| `TroubleshootingPatterns` | Match symptoms to solutions |
| `SafetyEngine` | Validate all actions |
| `OllamaClient` | Generate thoughts, reflections |
| `ReActStorage` | Persist reasoning traces |

---

## ReAct Loop

```
+---------------------------------------------------------------------+
|                     ReAct Iteration                                  |
+---------------------------------------------------------------------+
|                                                                     |
|  1. THINK                                                           |
|     +------------------------------------------------------------+  |
|     | Context: goal + history + extracted facts                  |  |
|     | LLM generates: "I need to check X first..."                |  |
|     +------------------------------------------------------------+  |
|                                                                     |
|  2. ACTION                                                          |
|     +------------------------------------------------------------+  |
|     | Try NeurosymbolicCapability first if --neurosymbolic       |  |
|     | If match: use domain operation                             |  |
|     | Else: LLM generates action                                 |  |
|     | Validate: safety check, syntax check                       |  |
|     +------------------------------------------------------------+  |
|                                                                     |
|  3. OBSERVE                                                         |
|     +------------------------------------------------------------+  |
|     | Execute command, capture stdout/stderr                     |  |
|     | Parse output into facts                                    |  |
|     +------------------------------------------------------------+  |
|                                                                     |
|  4. REFLECT                                                         |
|     +------------------------------------------------------------+  |
|     | "Good, found the issue. Next I should..."                  |  |
|     | Update confidence, check if goal achieved                  |  |
|     +------------------------------------------------------------+  |
|                                                                     |
|  5. Loop until: goal achieved OR max iterations OR user interrupt   |
|                                                                     |
+---------------------------------------------------------------------+
```

---

# Future: Neurosymbolic Enhancement Plan

Make neurosymbolic system more reusable, DRY, and type-safe.

## Current Issues

1. **Plain JSON configurations** - operations.json, entities.json, inference_rules.json
2. **Code duplication** - similar patterns across domains
3. **No type safety** - parsing JSON at runtime, no compile-time validation
4. **Hard to extend** - adding new operation types requires code changes

## Goals

1. **Type-safe domain definitions** using Rust structs with serde
2. **Code generation** from domain definitions (optional)
3. **Reusable operation templates** - compose operations from smaller units
4. **Better separation** - domain logic from infrastructure
5. **Validation at load time** - fail fast on invalid configs

## Proposed Changes

### 1. Typed Operation Definitions

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Operation {
    pub op_id: String,
    pub name: String,
    pub description: String,
    pub intent: String,
    #[serde(default)]
    pub input_schema: HashMap<String, InputField>,
    pub generators: Vec<Generator>,
    pub examples: Vec<OperationExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Generator {
    pub name: String,
    pub tool: String,
    pub template: String,
    #[serde(default)]
    pub when: Vec<Condition>,
    #[serde(default)]
    pub preference_score: f32,
}
```

### 2. Operation Traits for Reusability

```rust
pub trait Operation {
    type Input: Validate + Default;
    type Output: ParseOutput;

    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn generate_commands(&self, input: &Self::Input) -> Result<Vec<GeneratedCommand>>;
    fn parse_output(&self, raw: &str) -> Self::Output;
}

pub struct CompositeOperation {
    operations: Vec<Box<dyn Operation<Input = (), Output = ()>>>,
}
```

### 3. Migration Strategy

```
Phase 1: Type-safe wrappers
- Keep JSON configs
- Add Rust structs with full validation
- Fail at load time on invalid configs

Phase 2: Code generation (optional)
- Generate Rust structs from JSON schemas
- Add derive macros for common patterns
- Allow pure Rust domain definitions

Phase 3: Optional Rust-native domains
- Allow defining domains entirely in Rust
- Use trait bounds for type-safe operations
- Enable compile-time validation
```

---

## Benefits of Shared Architecture

1. **No duplication** - One NeurosymbolicCapability used by all modes
2. **Consistent behavior** - Same domain operations across query/agent/chat/react
3. **Easy extension** - Add new capabilities to one place
4. **Maintainable** - Single source of truth for neurosymbolic logic
5. **Testable** - Test the capability independently
