# Neurosymbolic Domain Configuration System

## Overview

A **domain-centric, config-driven symbolic reasoning system** for Vibe CLI. Each domain (Linux, Docker, Kubernetes, etc.) is defined as a collection of JSON files containing entities, relationships, operations, and inference rules.

## Directory Structure

```
~/.config/vibe_cli/
├── domains/
│   └── ultimate_linux/
│       ├── domain.json              # Main manifest
│       ├── entities/
│       │   ├── process.json
│       │   ├── file.json
│       │   ├── user.json
│       │   ├── service.json
│       │   └── network.json
│       ├── relationships.json
│       ├── operations.json
│       ├── inference_rules.json
│       ├── troubleshooting.json
│       └── reasoning_templates.json
└── shared_entities/
    ├── process.json
    ├── file.json
    └── user.json
```

## Core Components

### 1. Domain Types (`src/config/domain_types.rs`)

```rust
pub struct Domain {
    pub domain: String,
    pub version: String,
    pub description: String,
    pub entities: HashMap<String, Entity>,
    pub relationships: Vec<Relationship>,
    pub operations: Vec<Operation>,
    pub inference_rules: Vec<InferenceRule>,
    pub troubleshooting_patterns: Vec<TroubleshootingPattern>,
    pub reasoning_templates: Vec<ReasoningTemplate>,
}

pub struct Entity {
    pub name: String,
    pub description: String,
    pub core_properties: Vec<Property>,
}

pub struct Property {
    pub name: String,
    pub type_: String,
    pub meaning: String,
    pub example: Option<Value>,
    pub allowed_values: Option<Vec<String>>,
}

pub struct Operation {
    pub op_id: String,
    pub name: String,
    pub description: String,
    pub input_schema: HashMap<String, InputSpec>,
    pub generators: Vec<Generator>,
    pub output_schema: Option<OutputSchema>,
}

pub struct Generator {
    pub name: String,
    pub tool: String,
    pub template: String,
    pub when: Vec<RequiredInput>,
    pub preference_score: f32,
}

pub struct InferenceRule {
    pub rule_id: String,
    pub if_: Vec<RuleCondition>,
    pub then: Vec<RuleConclusion>,
}
```

### 2. Command Generator (`src/domain/command_generator.rs`)

Dynamic command generation with scoring-based generator selection:

```rust
impl CommandGenerator {
    pub fn generate(&self, operation: &Operation, inputs: &HashMap<String, Value>) -> Vec<Command> {
        // 1. Score each generator by input completeness
        // 2. Select best matching generator
        // 3. Resolve template variables
        // 4. Build final command (direct exec, no shell)
    }
    
    fn score_generator(&self, gen: &Generator, inputs: &HashMap<String, Value>) -> f32 {
        // Required inputs present: +1.0 each
        // Optional inputs present: +0.5 each
        // Preference score: +configurable
    }
}
```

### 3. Domain Registry (`src/config/domain_registry.rs`)

```rust
impl DomainRegistry {
    pub fn load_all() -> Result<Self> {
        // Load prebuilt domains from ~/.local/share/vibe_cli/domains/
        // Load user overrides from ~/.config/vibe_cli/domains/
        // Deep merge (user takes precedence)
    }
    
    pub fn query_intent(&self, intent: &str) -> Vec<&Domain> {
        // Return domains sorted by confidence for this intent
    }
}
```

### 4. Output Parser (`src/domain/output_parser.rs`)

```rust
impl OutputParser {
    pub fn parse(&self, output: &str, schema: &OutputSchema) -> Vec<HashMap<String, Value>> {
        // Parse command output using operation's output_schema
        // Support delimited (CSV, TSV), JSON, key-value formats
    }
}
```

## Command Generation Flow

```
1. Intent → Operation (match intent to op_id)
         ↓
2. Select Generator (score by input completeness)
         ↓
3. Resolve Template (replace {{var}} with inputs)
         ↓
4. Execute Command (direct exec, no shell)
         ↓
5. Parse Output (using operation's output_schema)
         ↓
6. Return Structured Result
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Generator Selection | Score-based (B) | Smarter than first-match |
| Output Parsing | Schema-defined per operation (A) | Type safety, validation |
| Fallback | Try next generator (B) | Robustness |
| Shell vs Direct | Direct exec (A) | Security, no injection risk |
| Config Format | JSON | Your existing spec |
| Entities | Shared + domain-specific | DRY principle |

## Usage Examples

### Example 1: List Processes

```rust
// User: "List nginx processes"
let intent = analyzer.extract_intent("List nginx processes");
// → intent.operation = "list_processes"
// → intent.inputs = { "filter": "nginx" }

let cmd = generator.generate(&list_processes_op, &intent.inputs);
// → "ps -eo pid,ppid,cmd,%cpu,%mem,state | grep nginx"

let result = executor.run("ps -eo pid,ppid,cmd,%cpu,%mem,state");
// Parse using output_schema...

let processes = parser.parse_list_processes(output);
// → [{ pid: 1234, cmdline: "nginx...", cpu_percent: 2.5 }, ...]
```

### Example 2: Diagnose High CPU

```rust
// User: "Why is my server slow?"
let observations = collector.collect(vec![
    "list_processes",
    "get_system_load",
    "check_memory_usage"
]);

let conclusions = engine.apply_rules(&observations);
// → Rule: high_cpu
//    If: process.cpu_percent > 90
//    Then: conclude("cpu_hot_process", confidence=0.8)

let plan = troubleshooter.create_plan(&conclusions);
// → Hypothesis: "Process 1234 (nginx) consuming 95% CPU"
// → Checks: ["strace -p 1234", "top -H -p 1234"]
```

### Example 3: Safe Process Termination

```rust
// User: "Gracefully stop nginx"
let safety = safety_engine.check(&Termination {
    target: process.pid,
    signal: "SIGTERM"
});
// → Checks sensitive paths, permissions
// → Result: safe_to_proceed

let cmd = generator.generate(&send_signal_term, &{ "pid": 1234 });
// → "kill -TERM 1234"
```

## Benefits

1. **Maintainability** - Change rules/operations without recompiling
2. **Extensibility** - Add new domains without code changes
3. **User Customization** - Override prebuilt behavior via JSON
4. **Testability** - Test reasoning with mock domains
5. **Domain Knowledge Sharing** - Shared entities across domains
6. **Auditability** - All rules visible in JSON files

## Prebuilt Domains

| Domain | Status | Entities | Operations |
|--------|--------|----------|------------|
| ultimate_linux | Planned | 5 | 30 |
| docker | Future | 3 | 20 |
| kubernetes | Future | 4 | 25 |

## Implementation Phases

| Phase | Task | Files |
|-------|------|-------|
| 1 | Core types & loader | `domain_types.rs`, `domain_loader.rs` |
| 2 | Command generator | `command_generator.rs` |
| 3 | Output parser | `output_parser.rs` |
| 4 | Domain registry | `domain_registry.rs` |
| 5 | Reasoning engine | `reasoning_engine.rs` |
| 6 | Ultimate Linux domain | JSON files |
| 7 | Integration | Update `neurosymbolic_service.rs` |

## Configuration Files

### Entity Example (process.json)
```json
{
  "name": "Process",
  "description": "A running instance of a program",
  "core_properties": [
    { "name": "pid", "type": "integer", "meaning": "Process ID" },
    { "name": "ppid", "type": "integer", "meaning": "Parent process ID" },
    { "name": "state", "type": "enum", "values": ["R", "S", "D", "T", "Z", "I"], "meaning": "Scheduler state" },
    { "name": "cpu_percent", "type": "number", "meaning": "CPU usage %" }
  ]
}
```

### Operation Example (list_processes.json)
```json
{
  "op_id": "list_processes",
  "name": "List processes",
  "input_schema": {
    "filter": { "type": "string", "optional": true },
    "user": { "type": "string", "optional": true }
  },
  "generators": [
    {
      "name": "ps_style",
      "tool": "ps",
      "template": "ps -eo pid,ppid,cmd,%cpu,%mem,state {{filter}} {{user}}",
      "when": []
    }
  ]
}
```

### Inference Rule Example
```json
{
  "rule_id": "zombie_detect",
  "if": [{ "entity": "Process", "prop": "state", "equals": "Z" }],
  "then": [
    { "conclude": "process_exited_not_reaped", "confidence": 0.99 },
    { "recommend": "identify_parent_and_fix_wait", "confidence": 0.9 }
  ]
}
```
