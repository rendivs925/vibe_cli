# ReAct Agent Implementation Plan

## Overview

Implement ReAct (Reasoning + Acting) for the agentic CLI with optional neurosymbolic enhancement for complex tasks:

- **Simple tasks**: Pure ReAct with LLM reasoning
- **Complex tasks**: ReAct + neurosymbolic domain operations for structured, non-guessing execution

---

## CLI Interface

```bash
# Pure ReAct - LLM reasoning for simple tasks
vibe_cli --react "list processes"

# ReAct + Neurosymbolic - structured operations for complex tasks
vibe_cli --react --neurosymbolic "nginx is not running, diagnose and fix"

# With custom config
vibe_cli --react --max-iterations 50 --show-thoughts "debug high memory usage"
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

## Dual Mode Architecture

```
+---------------------------------------------------------------------+
|                         ReAct Agent                                  |
+---------------------------------------------------------------------+
|                                                                     |
|  +----------------------------------------------------------------+ |
|  |                    REACT MODE (Simple)                          | |
|  |  - LLM generates thoughts, actions from natural language        | |
|  |  - Flexible but may "guess" for complex scenarios               | |
|  |  - Best for: exploration, learning, unknown domains             | |
|  +----------------------------------------------------------------+ |
|                                                                     |
|  +----------------------------------------------------------------+ |
|  |               REACT + NEUROSYMBOLIC MODE (Complex)              | |
|  |  - ReAct reasoning + structured domain operations               | |
|  |  - Uses: operations.json, entities, inference_rules             | |
|  |  - No guessing: exact commands from verified templates          | |
|  |  - Best for: known domains, production troubleshooting          | |
|  +----------------------------------------------------------------+ |
|                                                                     |
+---------------------------------------------------------------------+
```

---

## How Neurosymbolic Enhances ReAct

### Without Neurosymbolic (Pure ReAct)

```
Thought: "I need to check nginx status. I'll try systemctl."
Action: bash: systemctl status nginx
Observation: nginx failed
Thought: "Let me check logs with journalctl."
Action: bash: journalctl -u nginx -n 50
-- May guess wrong commands, flags, or tools
```

### With Neurosymbolic (Structured)

```
Thought: "User says nginx is not running. This matches troubleshooting pattern."
Action: domain_op: check_service {service: "nginx"}
Observation: {status: "failed", error: "Address in use"}
Thought: "Error is 'Address in use'. This matches inference rule."
Action: domain_op: find_port_binding {port: 80}
Observation: {process: "nginx", pid: 1234, state: "running"}
Thought: "Nginx is actually running, systemd status is stale."
Action: domain_op: restart_service {service: "nginx"}
-- Uses verified commands from operations.json, no guessing
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
    SymbolicCheck {
        check_type: String,
        rule_id: String,
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
    domain_registry: Option<DomainRegistry>,
    config: ReActConfig,
}

impl ReActAgentService {
    pub async fn run(&mut self, goal: &str) -> Result<ReActResult> {
        let mut trace = ReActTrace::new(goal);
        let mut context = ReActContext::new(goal);
        
        for iteration in 0..self.config.max_iterations {
            // 1. Generate thought
            let thought = self.generate_thought(&context).await?;
            context.add_thought(&thought);
            
            // 2. Parse intent, check if goal achieved
            if self.is_goal_achieved(&context) {
                trace.final_result = Some(context.get_summary());
                trace.success = true;
                break;
            }
            
            // 3. Generate action (LLM or Domain Operation)
            let action = self.generate_action(&context, &thought).await?;
            
            // 4. Validation (safety + syntax)
            let validation = self.validate_action(&action).await?;
            if !validation.is_safe {
                context.add_reflection(&format!("Action blocked: {}", validation.reason));
                continue;
            }
            
            // 5. Return action for user confirmation
            return Ok(ReActResult::pending_action(action, context.clone()));
        }
        
        ReActResult::from_trace(trace)
    }
    
    async fn generate_action(&self, context: &ReActContext) -> Result<Action> {
        // If neurosymbolic enabled, try domain operations first
        if self.config.use_neurosymbolic {
            if let Some(operation) = self.domain_registry.match_operation(&context.goal) {
                let commands = self.generate_from_operation(&operation, &context)?;
                return Ok(Action::DomainOperation {
                    op_id: operation.op_id,
                    parameters: operation.parameters,
                    generated_commands: commands,
                });
            }
        }
        
        // Fall back to LLM-generated action
        self.generate_llm_action(context).await
    }
    
    async fn apply_inference_rules(&self, context: &ReActContext) -> Result<Vec<Conclusion>> {
        let registry = self.domain_registry.as_ref()?;
        let mut conclusions = Vec::new();
        
        for rule in registry.get_inference_rules() {
            if rule.evaluate(&context.observed_facts) {
                conclusions.push(rule.conclusion.clone());
            }
        }
        
        Ok(conclusions)
    }
}
```

**File:** `application/src/services/complexity_detector.rs`

```rust
pub enum TaskComplexity {
    Simple,      // Pure ReAct
    Moderate,    // ReAct with fallback to symbolic
    Complex,     // ReAct + Neurosymbolic recommended
}

impl ComplexityDetector {
    fn estimate_complexity(&self, task: &str) -> Result<TaskComplexity> {
        let task_lower = task.to_lowercase();
        
        // Check for known services
        let known_services = ["nginx", "apache", "mysql", "postgres", "redis", "docker"];
        let has_known_service = known_services.iter().any(|s| task_lower.contains(s));
        
        // Check for known actions
        let known_actions = ["restart", "status", "check", "diagnose", "fix", "debug"];
        let has_known_action = known_actions.iter().any(|a| task_lower.contains(a));
        
        // Check for troubleshooting keywords
        let trouble_keywords = ["not running", "failed", "error", "crash", "memory", "cpu"];
        let is_troubleshooting = trouble_keywords.iter().any(|k| task_lower.contains(k));
        
        Ok(match (has_known_service, has_known_action, is_troubleshooting) {
            (true, true, true) => TaskComplexity::Complex,
            (true, _, _) => TaskComplexity::Moderate,
            (_, true, _) => TaskComplexity::Moderate,
            _ => TaskComplexity::Simple,
        })
    }
}
```

### Presentation Layer

**File:** `presentation/src/cli/handlers/react.rs`

```rust
impl CliHandlers {
    pub async fn handle_react(
        &mut self,
        task: &str,
        use_neurosymbolic: bool,
        config: ReActConfig,
    ) -> Result<()> {
        // Detect task complexity
        let complexity = self.estimate_complexity(task).await?;
        
        let use_symbolic = if use_neurosymbolic {
            true
        } else if complexity.is_complex() && self.has_enabled_domains() {
            println!("This task matches known domain operations.");
            println!("Enable neurosymbolic mode for structured execution?");
            ask_confirmation("Use verified commands from domain config?", false)?
        } else {
            false
        };
        
        let mut service = ReActAgentService::new(
            self.ollama_client()?,
            self.domain_registry(),
            use_symbolic,
            config,
        )?;
        
        let mut session = ReActSession::new(task, use_symbolic);
        
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
                            config.auto_execute_all = true;
                        }
                    }
                }
                ReActStepResult::SymbolicSuggestion(suggestion) => {
                    session.prompt_symbolic_choice(&suggestion).await?;
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

**File:** `presentation/src/cli/cli_app.rs` (additions)

```rust
#[derive(Parser)]
pub struct Cli {
    /// ReAct agent with iterative reasoning (vs one-shot planning)
    #[arg(long)]
    pub react: bool,
    
    /// Enable neurosymbolic mode for complex tasks
    #[arg(long, requires = "react")]
    pub neurosymbolic: bool,
    
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

### Infrastructure Layer

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
    fn save_trace(&self, trace: &ReActTrace) -> Result<()> {
        let steps_json = serde_json::to_string(&trace.steps)?;
        
        db.execute(
            "INSERT INTO react_traces VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                trace.id.to_string(),
                trace.original_goal,
                trace.current_goal,
                steps_json,
                trace.final_result,
                trace.success as i32,
                trace.total_iterations,
                chrono::Utc::now().timestamp(),
            ],
        )?;
        
        Ok(())
    }
    
    fn find_similar_traces(&self, goal: &str, limit: usize) -> Result<Vec<ReActTrace>> {
        // Use embedding similarity or text matching
        // Return similar successful traces
    }
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
| Enable neurosymbolic for complex task | YES - suggested if complexity detected |
| Choose between symbolic recommendations | YES - when multiple options |
| Revise goal mid-session | YES - `/revise` command |
| Retry skipped step | YES - `/retry` command |
| Apply learning from trace | YES - at end of session |
| Edit/revise generated command | YES - `r` option |

---

## Sample Output

```
$ vibe_cli --react --neurosymbolic "nginx is crashing randomly"

--- DETECTED: Complex task matching Linux domain ---

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
Recommendations:
  1. Check worker_processes directive
  2. Check file descriptor limits
  3. Check configuration syntax

--- STEP 4: ACTION (Domain Operation: validate_config) ---
Operation: validate_nginx_config
Generated command: nginx -t

Execute? [Y/n/r/g/a/x]: Y

--- OBSERVATION ---
Syntax OK
Configuration test successful

--- STEP 5: THOUGHT ---
Config is valid. Worker crash could be resource limits. 
Let me check file descriptors and worker configuration.

--- STEP 5: ACTION ---
Generated: bash: ulimit -n && ps aux | grep nginx

Execute? [Y/n/r/g/a/x]: Y

--- OBSERVATION ---
ulimit -n: 1024
nginx workers: 4

--- STEP 6: SYMBOLIC INFERENCE ---
Rule: fd_limit_too_low
Conditions:
  - ulimit < 4096
  - nginx workers > 1
Conclusion: File descriptor limit too low for 4 workers

Recommendation: Increase worker_rlimit_nofile or decrease workers

--- STEP 7: ACTION (Domain Operation: apply_fix) ---
Operation: fix_fd_limit {service: "nginx"}
Generated commands:
  1. echo "nginx soft nofile 4096" >> /etc/security/limits.conf
  2. Edit nginx.conf: worker_rlimit_nofile 4096

Execute? [Y/n/r/g/a/x]: 

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
| 2 | Agent service | `application/src/services/react_agent_service.rs`, `application/src/services/thought_parser.rs` |
| 3 | Complexity detector | `application/src/services/complexity_detector.rs` |
| 4 | Storage | `infrastructure/src/react_storage.rs` |
| 5 | CLI handler | `presentation/src/cli/handlers/react.rs` |
| 6 | CLI app update | `presentation/src/cli/cli_app.rs` |
| 7 | Integration | Connect OllamaClient, SafetyEngine, LearningService, DomainRegistry |
| 8 | Tests | Unit + integration tests for both modes |

---

## Integration Points

| Component | Role in ReAct |
|-----------|---------------|
| `DomainRegistry` | Match task to operations, generate commands |
| `CommandGenerator` | Generate from templates, score alternatives |
| `InferenceEngine` | Apply rules to observations |
| `TroubleshootingPatterns` | Match symptoms to solutions |
| `SafetyEngine` | Validate all actions |
| `ExperienceBuffer` | Learn from ReAct traces |
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
|     | Parse thought -> determine tool (LLM or Domain Operation)  |  |
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

## Complexity Detection Rules

| Conditions | Complexity | Recommended Mode |
|------------|------------|------------------|
| Known service + known action + troubleshooting | Complex | ReAct + Neurosymbolic |
| Known service + known action | Moderate | ReAct (suggest symbolic) |
| Known action only | Moderate | ReAct (suggest symbolic) |
| Unknown service/action | Simple | Pure ReAct |

---

## Known Patterns for Detection

**Known Services:**
- nginx, apache, mysql, postgres, redis, docker

**Known Actions:**
- restart, status, check, diagnose, fix, debug

**Troubleshooting Keywords:**
- not running, failed, error, crash, memory, cpu, slow, hanging
