# ReAct Agent Implementation Plan

## Current State: --neurosymbolic Already Works

The `--neurosymbolic` flag **already exists** and works with the default query mode:

```bash
vibe_cli --neurosymbolic "show my ram"       # Works today - calls handle_neurosymbolic
vibe_cli --neurosymbolic "list processes"    # Works today
```

Current behavior (already implemented):
- `cli_app.rs` routes `--neurosymbolic` to `handle_neurosymbolic()`
- Uses `NeurosymbolicService` for domain operations
- Falls back to LLM if no matching domain operation

---

## What This Plan Adds

This plan adds a **new `--react` flag** for iterative reasoning:

```bash
# NEW: ReAct mode with optional neurosymbolic
vibe_cli --react "nginx is not responding, diagnose and fix"

# NEW: ReAct + neurosymbolic (uses domain operations if available)
vibe_cli --react --neurosymbolic "debug high memory usage"
```

---

## CLI Interface

```bash
# Existing - works today
vibe_cli --neurosymbolic "show my ram"

# NEW - ReAct mode (iterative reasoning)
vibe_cli --react "find what's using all my memory"

# NEW - ReAct with neurosymbolic
vibe_cli --react --neurosymbolic "nginx is crashing, diagnose and fix"

# Chat mode (already exists)
vibe_cli --chat

# Chat with neurosymbolic
vibe_cli --chat --neurosymbolic

# Agent mode (already exists)
vibe_cli --agent "check all services"

# Agent with neurosymbolic
vibe_cli --agent --neurosymbolic "check and fix nginx"
```

---

## Interactive Iteration (New for --react)

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

## Confirmation Flow (For --react)

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

## Mode Selection Logic

```rust
// For --react mode:
if --neurosymbolic is specified:
    if domain operations available for task:
        use domain operations
    else:
        fall back to pure ReAct (show warning)
else:
    use pure ReAct

// For other modes (--chat, --agent, default query):
// Already handled by existing code - --neurosymbolic works as before
```

---

## Architecture: ReAct Uses Shared NeurosymbolicCapability

The `--react` handler uses the same `NeurosymbolicCapability` as other modes:

```
+---------------------------------------------------------------------+
|                         CLI Handlers                                 |
+---------------------------------------------------------------------+
|                                                                     |
|   handle_chat()  --->  Uses NeurosymbolicService (already exists)   |
|   handle_agent() --->  Uses NeurosymbolicService (already exists)   |
|   handle_neurosymbolic() -> Uses NeurosymbolicService (already exists) |
|   handle_react() --->  NEW: Uses NeurosymbolicCapability            |
|   handle_query() --->  Uses NeurosymbolicService (already exists)   |
|                                                                     |
+---------------------------------------------------------------------+
            |
            | Uses shared service
            v
+---------------------------------------------------------------------+
|                   NeurosymbolicCapability (NEW for ReAct)           |
+---------------------------------------------------------------------+
|                                                                     |
|   - match_operation(query) -> Option<ResolvedOperation>             |
|   - generate_commands(operation, inputs) -> Vec<GeneratedCommand>   |
|   - apply_inference_rules(facts) -> Vec<Conclusion>                 |
|                                                                     |
+---------------------------------------------------------------------+
            |
            | Delegates to
            v
+---------------------------------------------------------------------+
|                    NeurosymbolicService (existing)         |
+---------------------------------------------------------------------+
```

---

## Implementation: What Needs to Be Added

### 1. Domain Layer - ReAct Types

**New file:** `domain/src/entities/react.rs`

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

**New file:** `domain/src/repositories/react_repository.rs`

```rust
pub trait ReActRepository {
    fn save_trace(&self, trace: &ReActTrace) -> Result<()>;
    fn find_similar_traces(&self, goal: &str, limit: usize) -> Result<Vec<ReActTrace>>;
    fn get_success_rate(&self, goal_pattern: &str) -> Result<f32>;
}
```

### 2. Application Layer

**New file:** `application/src/services/react_agent_service.rs`

```rust
pub struct ReActAgentService {
    ollama_client: OllamaClient,
    repository: Box<dyn ReActRepository>,
    safety_engine: SafetyEngine,
    neuro: Option<NeurosymbolicCapability>,
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

### 3. Presentation Layer

**Update:** `presentation/src/cli/cli_app.rs`

```rust
#[derive(Parser)]
pub struct Cli {
    // ... existing flags ...

    /// ReAct agent with iterative reasoning
    #[arg(long)]
    pub react: bool,

    /// Max ReAct iterations (default: 30)
    #[arg(long, value_name = "N", requires = "react")]
    pub max_iterations: Option<usize>,

    /// Show full reasoning trace
    #[arg(long, requires = "react")]
    pub show_thoughts: bool,

    // ... existing flags ...
}

impl CliApp {
    pub async fn run(&mut self, cli: Cli) -> Result<()> {
        // ... existing handlers ...

        // NEW: Handle --react flag
        if cli.react {
            return self.handlers.handle_react(&args_str, &cli).await;
        }

        // ... existing fallback ...
    }
}
```

**New file:** `presentation/src/cli/handlers/react.rs`

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

        if cli.neurosymbolic && !service.has_domain_operations() {
            println!("[Warning: No domain operations available, using pure ReAct]");
        }

        let mut session = ReActSession::new(task);

        loop {
            match service.run_step(&session.context).await? {
                ReActStepResult::Thought(thought) => {
                    session.display_thought(&thought);
                }
                ReActStepResult::Action(action) => {
                    if action.is_domain_operation() {
                        println!("[Domain Operation: {}]", action.op_id);
                    }

                    let decision = session.prompt_action(&action).await?;
                    match decision {
                        ActionDecision::Execute => {
                            let output = self.execute_action(&action).await?;
                            session.display_observation(&output);
                            session.context.add_observation(&output);

                            let reflection = service.generate_reflection(&session.context).await?;
                            session.display_reflection(&reflection);
                            session.context.add_reflection(&reflection);
                        }
                        ActionDecision::Skip => {
                            session.context.add_note("User skipped this step");
                        }
                        ActionDecision::Revise(prompt) => {
                            let revised = service.revise_action(&action, &prompt).await?;
                            session.pending_action = Some(revised);
                        }
                        ActionDecision::ReviseGoal(new_goal) => {
                            session.context.update_goal(&new_goal);
                        }
                        ActionDecision::Abort => {
                            println!("Session aborted by user.");
                            return Ok(());
                        }
                        ActionDecision::AutoAll => {
                            // Auto-execute remaining steps
                        }
                    }
                }
                ReActStepResult::Done(result) => {
                    session.display_result(&result);
                    return Ok(());
                }
            }

            if let Some(cmd) = session.check_session_command().await? {
                match cmd {
                    SessionCommand::ReviseGoal(new_goal) => {
                        session.context.update_goal(&new_goal);
                    }
                    SessionCommand::ShowContext => {
                        session.display_context();
                    }
                    SessionCommand::Retry => {
                        session.retry_last_action();
                    }
                    SessionCommand::Abort => {
                        println!("Session aborted.");
                        return Ok(());
                    }
                    SessionCommand::Help => {
                        session.display_help();
                    }
                }
            }
        }
    }

    async fn prompt_action(&self, action: &Action) -> Result<ActionDecision> {
        println!("\n--- STEP {}: ACTION ---\n", action.step_number);
        println!("Thought: {}", action.thought);
        println!("\nProposed command:");
        println!("  {}", action.command.yellow());

        println!("\nOptions:");
        println!("  [Y] Execute this command");
        println!("  [n] Skip this step");
        println!("  [r] Revise the command");
        println!("  [g] Revise the goal");
        println!("  [a] Execute all remaining steps automatically");
        println!("  [x] Abort session");

        loop {
            let input = shared::utils::prompt("Execute? [Y/n/r/g/a/x]: ")?;
            match input.trim().to_lowercase().as_str() {
                "y" | "" => return Ok(ActionDecision::Execute),
                "n" => return Ok(ActionDecision::Skip),
                "r" => {
                    let revision = shared::utils::prompt("How should the command be revised? ")?;
                    return Ok(ActionDecision::Revise(revision));
                }
                "g" => {
                    let new_goal = shared::utils::prompt("New goal: ")?;
                    return Ok(ActionDecision::ReviseGoal(new_goal));
                }
                "a" => return Ok(ActionDecision::AutoAll),
                "x" => return Ok(ActionDecision::Abort),
                _ => println!("Invalid option. Use Y/n/r/g/a/x"),
            }
        }
    }
}
```

### 4. Infrastructure Layer

**New file:** `infrastructure/src/react_storage.rs`

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
| 1 | Domain types | `domain/src/entities/react.rs`, `domain/src/repositories/react_repository.rs` |
| 2 | ReAct agent service | `application/src/services/react_agent_service.rs` |
| 3 | ReAct CLI handler | `presentation/src/cli/handlers/react.rs` |
| 4 | Update CLI app | `presentation/src/cli/cli_app.rs` |
| 5 | Storage | `infrastructure/src/react_storage.rs` |
| 6 | Integration | Connect OllamaClient, SafetyEngine, DomainRegistry |
| 7 | Tests | Unit + integration tests |

---

## Integration Points

| Component | Role |
|-----------|------|
| `NeurosymbolicCapability` | Reusable service (shared with other modes) |
| `DomainRegistry` | Match task to operations, generate commands |
| `CommandGenerator` | Generate from templates |
| `InferenceEngine` | Apply rules to observations |
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
|     | Try NeurosymbolicCapability if --neurosymbolic             |  |
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

## Existing Code Used (No Duplication)

| Existing Component | How Used by ReAct |
|--------------------|-------------------|
| `NeurosymbolicService` | Source of domain operations |
| `DomainRegistry` | Match operations, generate commands |
| `SafetyEngine` | Validate actions |
| `OllamaClient` | Generate thoughts, reflections |
| `ExperienceBuffer` | Learn from ReAct traces |
| `CommandGenerator` | Generate commands from templates |

---

## Future: Neurosymbolic Enhancement

Type-safe domain definitions, operation traits, derive macros.

See: [docs/NEUROSYMBOLIC_ENHANCEMENT.md](./NEUROSYMBOLIC_ENHANCEMENT.md)
