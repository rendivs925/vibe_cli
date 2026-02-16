# Complete Dynamic Tool-Based ReAct System

## Table of Contents
1. [Overview](#overview)
2. [Tool Taxonomy](#tool-taxonomy)
3. [Architecture](#architecture)
4. [State Machine](#state-machine)
5. [Tool Selection Logic](#tool-selection-logic)
6. [Integration Points](#integration-points)
7. [File Changes](#file-changes)
8. [Backward Compatibility](#backward-compatibility)
9. [Example Flows](#example-flows)

---

## 1. Overview

### Current State
```
ANALYZE → SUGGESTED (always command) → OUTPUT → repeat
```

### Target State
```
ANALYZE → TOOL SELECTION → TOOL EXECUTION → OUTPUT → repeat
```

### Key Principles
- **Dynamic Tool Selection**: AI chooses right tool at each step
- **Explicit Reasoning**: Every tool choice has justification
- **State Awareness**: Tools track session state
- **Graceful Degradation**: Falls back to command if tool fails

---

## 2. Tool Taxonomy

### Category A: Investigation Tools (Gathering Data)

| Tool | Purpose | When to Use | Output |
|------|---------|-------------|--------|
| `suggest_command` | Propose shell command | Need to run diagnostic | 1-3 commands |
| `suggest_read` | Propose file read | Need to examine file | File content |
| `suggest_grep` | Propose grep pattern | Need to search content | Search results |
| `suggest_rag` | Propose RAG query | Need codebase context | Relevant code |
| `suggest_discovery` | Propose discovery | Need system info | System data |

### Category B: Analysis Tools (Understanding Data)

| Tool | Purpose | When to Use | Output |
|------|---------|-------------|--------|
| `summarize` | Summarize output | Output too long | 3-5 sentence summary |
| `extract_errors` | Find errors | Need to find failures | Error list |
| `extract_warnings` | Find warnings | Need to find issues | Warning list |
| `extract_metrics` | Parse numbers | Need quantitative data | Metrics list |
| `extract_patterns` | Find patterns | Need pattern detection | Pattern list |
| `compare` | Compare data | Need before/after | Differences |
| `correlate` | Find relationships | Need to connect data | Correlation info |

### Category C: Planning Tools (Strategy)

| Tool | Purpose | When to Use | Output |
|------|---------|-------------|--------|
| `plan_next` | Next steps | At decision points | 2-3 steps with rationale |
| `narrow_focus` | Narrow scope | Investigation too broad | Focused hypothesis |
| `branch` | Explore alternatives | Multiple paths possible | Options with tradeoffs |
| `rethink` | New approach | Stuck/looping | Alternative strategy |
| `prioritize` | Rank options | Multiple options | Ranked list |

### Category D: Action Tools (Making Changes)

| Tool | Purpose | When to Use | Output |
|------|---------|-------------|--------|
| `apply_fix` | Apply fix | Ready to fix | Execution result |
| `edit_file` | Edit file | Need to modify | Edit confirmation |
| `create_file` | Create file | Need new file | Creation confirmation |
| `run_command` | Run command | Need to execute | Command output |
| `retry` | Retry failed | Previous failed | Retry result |

### Category E: Verification Tools (Checking)

| Tool | Purpose | When to Use | Output |
|------|---------|-------------|--------|
| `check_goal` | Verify goal | Need success check | YES/NO + reason |
| `verify_fix` | Verify fix | After applying fix | Verification result |
| `verify_syntax` | Check syntax | Before apply | Syntax valid? |
| `test_hypothesis` | Test hypothesis | Need to validate | Test result |

### Category F: Memory Tools (Context)

| Tool | Purpose | When to Use | Output |
|------|---------|-------------|--------|
| `show_facts` | Show facts | Review findings | Facts list |
| `show_hypotheses` | Show hypotheses | Review theories | Hypotheses |
| `show_history` | Show history | Session review | Step summary |
| `show_context` | Show full context | Debug issues | All data |
| `show_plan` | Show current plan | Review strategy | Plan items |
| `compact_session` | Compact history | History too long | Summary |

### Category G: Resolution Tools (Ending)

| Tool | Purpose | When to Use | Output |
|------|---------|-------------|--------|
| `conclude_success` | Success end | Problem solved | Final summary |
| `conclude_fail` | Failure end | Can't solve | Final summary + reason |
| `escalate` | Human help | Need assistance | Escalation info |
| `defer` | Defer task | Need external | Deferral info |

### Category H: Interaction Tools (User)

| Tool | Purpose | When to Use | Output |
|------|---------|-------------|--------|
| `ask_clarification` | Get clarification | Ambiguous | Question for user |
| `ask_confirmation` | Get confirmation | Need approval | Confirmation prompt |
| `explain` | Explain reasoning | User confused | Explanation |
| `suggest_alternatives` | Offer options | Multiple ways | Options list |

---

## 3. Architecture

### Core Components

```
┌─────────────────────────────────────────────────────────────────┐
│                    ReAct Loop Controller                         │
│  ┌─────────────┐    ┌────────────────┐    ┌─────────────────┐  │
│  │   ANALYZE   │───▶│ TOOL SELECTOR │───▶│ TOOL EXECUTOR  │  │
│  └─────────────┘    └────────────────┘    └─────────────────┘  │
│         │                   │                     │            │
│         │                   ▼                     ▼            │
│         │          ┌────────────────┐    ┌─────────────────┐  │
│         │          │TOOL DECISION   │    │ TOOL OUTPUT     │  │
│         │          │   (LLM)        │    │  PROCESSOR      │  │
│         │          └────────────────┘    └─────────────────┘  │
│         │                                       │              │
│         ▼                                       ▼              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Session State                        │   │
│  │  - steps[], facts[], hypotheses[], constraints[]      │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Data Structures

```rust
// Tool Selection Decision
pub struct ToolDecision {
    pub tool: ReactTool,           // Selected tool
    pub justification: String,      // Why this tool
    pub context_needed: String,    // What data needed
    pub confidence: f32,          // Selection confidence
}

// Tool Execution Result  
pub struct ToolResult {
    pub tool: ReactTool,
    pub output: String,            // Display to user
    pub commands: Vec<String>,    // Commands to execute
    pub facts_extracted: Vec<Fact>,
    pub hypotheses_updated: Vec<Hypothesis>,
    pub next_tool_suggestion: Option<ReactTool>,
    pub should_continue: bool,
    pub should_ask_user: bool,
    pub user_question: Option<String>,
}

// Tool Registry
pub struct ToolRegistry {
    tools: HashMap<ReactTool, Arc<dyn ReactToolHandler>>,
}

pub trait ReactToolHandler {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn category(&self) -> ToolCategory;
    fn requires_output(&self) -> bool;
    fn execute(&self, context: &RetrievedContext, params: &str) -> Result<ToolResult>;
    fn get_prompt(&self, context: &RetrievedContext) -> String;
}
```

---

## 4. State Machine

```
┌──────────────┐
│   START      │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   ANALYZE    │ ◄──────────────────────────────────┐
└──────┬───────┘                                   │
       │                                            │
       ▼                                            │
┌──────────────────┐                                │
│  TOOL SELECTION   │                                │
│  (LLM chooses)   │                                │
└────────┬─────────┘                                │
         │                                          │
    ┌────┴────┬──────────┬──────────┐               │
    ▼         ▼          ▼          ▼               │
┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐           │
│Action │ │Analysis│ │Planning│ │ Meta  │           │
│Tools  │ │ Tools  │ │ Tools  │ │ Tools │           │
└───┬───┘ └───┬───┘ └───┬───┘ └───┬───┘           │
    │         │          │         │                │
    ▼         ▼          ▼         ▼                │
┌─────────────────────────────────────┐            │
│        TOOL EXECUTION               │            │
│  - Run LLM prompt                   │            │
│  - Extract commands                 │            │
│  - Update session                  │            │
└──────────────┬──────────────────────┘            │
               │                                    │
        ┌──────┴──────┐                            │
        ▼             ▼                            │
┌─────────────┐ ┌─────────────┐                    │
│ User Input  │ │ Continue   │────────────────────┘
│   Needed?   │ │   Loop?    │
└──────┬──────┘ └──────┬──────┘
       │              │
       ▼              ▼
┌─────────────┐ ┌─────────────┐
│   OUTPUT    │ │    END      │
│   Display   │ │  (conclude) │
└─────────────┘ └─────────────┘
```

---

## 5. Tool Selection Logic

### Decision Tree

```
After ANALYZE, LLM chooses tool based on:

1. What do we need?
   │
   ├─► Data/Information ──► Investigation Tools
   │      └─► Commands exist? ──► suggest_command
   │      └─► Need file? ──► suggest_read
   │      └─► Need search? ──► suggest_grep
   │      └─► Need code? ──► suggest_rag
   │
   ├─► Understanding ──► Analysis Tools
   │      └─► Output long? ──► summarize
   │      └─► Find errors? ──► extract_errors
   │      └─► Get numbers? ──► extract_metrics
   │      └─► Compare data? ──► compare
   │
   ├─► Strategy ──► Planning Tools
   │      └─► Next step? ──► plan_next
   │      └─► Too broad? ──► narrow_focus
   │      └─► Stuck? ──► rethink
   │
   ├─► Make Changes ──► Action Tools
   │      └─► Ready to fix? ──► apply_fix
   │      └─► Edit file? ──► edit_file
   │
   ├─► Check Progress ──► Verification Tools
   │      └─► Done? ──► check_goal
   │      └─► Fixed? ──► verify_fix
   │
   ├─► Review ──► Memory Tools
   │      └─► Show facts? ──► show_facts
   │      └─► Show history? ──► show_history
   │
   └─► End ──► Resolution Tools
          └─► Solved? ──► conclude_success
          └─► Can't solve? ──► escalate
```

### Tool Selection Prompt

```
## TOOL SELECTION

Based on your ANALYZE, choose ONE tool from this list:

### Investigation (need data)
- suggest_command: Propose diagnostic command to run
- suggest_read: Propose file to read
- suggest_grep: Propose search pattern  
- suggest_rag: Propose RAG query for code context
- suggest_discovery: Propose system discovery command

### Analysis (understand data)
- summarize: Summarize output in 3-5 sentences
- extract_errors: Extract error messages from output
- extract_warnings: Extract warnings from output
- extract_metrics: Extract numeric metrics from output
- compare: Compare two outputs or states

### Planning (strategy)
- plan_next: Propose 2-3 next steps
- narrow_focus: Narrow investigation scope
- branch: Explore alternative approaches
- rethink: Take completely new approach

### Action (make changes)
- apply_fix: Apply a fix or change
- edit_file: Edit an existing file
- create_file: Create a new file

### Verification (check)
- check_goal: Verify if original goal achieved
- verify_fix: Verify if fix was applied correctly

### Memory (context)
- show_facts: Show extracted facts
- show_hypotheses: Show current hypotheses
- show_history: Show session history
- show_context: Show all context

### Resolution (end)
- conclude_success: Problem solved
- conclude_fail: Cannot solve, escalate needed

### Interaction (user)
- ask_clarification: Need user clarification
- explain: Explain reasoning to user

---

Respond in this format:
TOOL: <tool_name>
JUSTIFY: <why this tool is right choice>
CONTEXT: <what data you're using>
```

---

## 6. Integration Points

### With Existing Systems

| System | Integration | Data Flow |
|--------|-------------|-----------|
| **Neurosymbolic** | Get domain operations | `suggest_command` uses domain ops |
| **RAG** | Query codebase | `suggest_rag` calls RAG |
| **Knowledge Graph** | Entity context | All tools can query KG |
| **Session Storage** | Persist state | Save after each tool |
| **Learning Service** | Track patterns | Record tool success |

### Tool-Specific Integrations

```rust
// suggest_command - uses neurosymbolic + RAG
pub fn handle_suggest_command(ctx: &RetrievedContext) -> Result<ToolResult> {
    // 1. Query neurosymbolic for domain operations
    let domain_ops = neurosymbolic_service.suggest_operations(&ctx.query);
    
    // 2. Query RAG for relevant code
    let code_ctx = rag_service.query(&ctx.query).await?;
    
    // 3. Generate command using LLM with all context
    let prompt = build_command_prompt(ctx, domain_ops, code_ctx);
    let commands = llm.generate(prompt).await?;
    
    // 4. Validate commands
    let validated = validate_commands(commands);
    
    Ok(ToolResult { commands: validated, ... })
}

// summarize - uses LLM
pub fn handle_summarize(ctx: &RetrievedContext) -> Result<ToolResult> {
    let prompt = format_summarize_prompt(ctx.latest_output);
    let summary = llm.generate(prompt).await?;
    Ok(ToolResult { output: summary, ... })
}

// show_facts - direct access
pub fn handle_show_facts(ctx: &RetrievedContext) -> Result<ToolResult> {
    let facts = ctx.facts.clone();
    let output = format_facts_list(&facts);
    Ok(ToolResult { output, ... })
}
```

---

## 7. File Changes

### New Files

| File | Purpose |
|------|---------|
| `domain/entities/react_tools.rs` | Tool enum, result types |
| `application/services/react_tool_service.rs` | Tool registry & execution |
| `application/services/tool_handlers/` | Individual tool handlers |
| `application/prompts/tool_selection.rs` | Tool selection prompts |

### Modified Files

| File | Changes |
|------|---------|
| `domain/entities/react.rs` | Add `ReactTool` enum, `ToolDecision` |
| `application/services/react_prompt_service.rs` | Add tool-specific prompts |
| `application/services/react_agent_service.rs` | Add tool execution flow |
| `presentation/cli/handlers/react.rs` | Replace with tool-based loop |

### Module Structure

```
application/src/services/react_tools/
├── mod.rs              # Module exports
├── registry.rs         # Tool registry
├── selector.rs         # Tool selection logic
├── executor.rs         # Tool execution
├── handlers/
│   ├── mod.rs
│   ├── investigation.rs   # suggest_command, suggest_read, etc.
│   ├── analysis.rs        # summarize, extract_*, compare
│   ├── planning.rs        # plan_next, narrow_focus, etc.
│   ├── action.rs          # apply_fix, edit_file
│   ├── verification.rs    # check_goal, verify_fix
│   ├── memory.rs         # show_facts, show_history
│   └── resolution.rs     # conclude, escalate
└── prompts/
    ├── mod.rs
    ├── selection.rs      # Tool selection prompt
    └── handlers/
        ├── mod.rs
        ├── summarize.rs
        ├── plan_next.rs
        └── ...
```

---

## 8. Backward Compatibility

### Gradual Rollout

1. **Phase 1**: Add tool selection, default to `suggest_command`
2. **Phase 2**: Add analysis tools (summarize, extract_errors)
3. **Phase 3**: Add planning tools (plan_next)
4. **Phase 4**: Full tool system

### Compatibility Mode

```rust
pub struct ReactConfig {
    pub tool_mode: ToolMode,
    pub default_tool: ReactTool,
}

pub enum ToolMode {
    Legacy,      // Always use suggest_command
    Mixed,       // Use tool selection, fall back to command
    Full,        // Full dynamic tool system
}

impl Default for ReactConfig {
    fn default() -> Self {
        Self {
            tool_mode: ToolMode::Mixed,
            default_tool: ReactTool::SuggestCommand,
        }
    }
}
```

### Fallback Logic

```rust
pub async fn execute_tool(
    tool: ReactTool,
    context: &RetrievedContext,
) -> Result<ToolResult> {
    // Try tool handler
    if let Some(handler) = registry.get(tool) {
        match handler.execute(context).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Log error
                error!("Tool {} failed: {}", tool, e);
                // Fall back to suggest_command
            }
        }
    }
    
    // Fallback to suggest_command
    suggest_command_fallback(context).await
}
```

---

## 9. Example Flows

### Example 1: Simple Diagnostics

```
USER: vibe_cli --react "nginx not responding"

--- ANALYZE ---
Nginx not responding - need to check:
1. Is nginx running?
2. What does logs show?
3. Are there any errors?

--- TOOL SELECTION ---
TOOL: suggest_command
JUSTIFY: Need to gather diagnostic data about nginx status
CONTEXT: User reported nginx not responding

--- SUGGESTED ---
systemctl status nginx
nginx -t
tail -20 /var/log/nginx/error.log

--- USER CONFIRMS ---
y

▶ Executing: systemctl status nginx

--- OUTPUT ---
● nginx.service - A high performance web server
   Loaded: loaded (/lib/systemd/system/nginx.service; enabled)
   Active: active (running) since Mon 2025-01-01 00:00:00
   Process: 1234 ExecStart=/usr/sbin/nginx (code=exited, status=0/SUCCESS)

--- TOOL SELECTION ---
TOOL: check_goal
JUSTIFY: Have initial data, check if goal achieved

--- CHECK GOAL ---
NO - nginx is running but user said "not responding"
Need to check network/connection issues

--- TOOL SELECTION ---
TOOL: suggest_command
JUSTIFY: Need to check nginx connections

--- SUGGESTED ---
ss -tlnp | grep :80
netstat -an | grep :80

--- TOOL SELECTION ---
TOOL: conclude_success
JUSTIFY: nginx is running and accepting connections

--- CONCLUSION ---
Root cause: Unknown (nginx running)
Status: Issue resolved - nginx is responding
```

### Example 2: Complex Investigation

```
USER: vibe_cli --react "debug memory leak in docker container"

--- ANALYZE ---
Memory leak in docker container - systematic investigation needed:
1. Identify which container
2. Check current memory usage
3. Analyze memory over time
4. Find root process

--- TOOL SELECTION ---
TOOL: suggest_command
JUSTIFY: Need to find the container

--- SUGGESTED ---
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Memory}}"

--- OUTPUT ---
webapp     Up 2 hours    512MB
database   Up 2 hours    256MB
redis      Up 2 hours    50MB

--- TOOL SELECTION ---
TOOL: extract_metrics
JUSTIFY: Memory values shown, need detailed metrics

--- EXTRACT METRICS ---
webapp: 512MB (high)
database: 256MB (normal)
redis: 50MB (normal)

--- TOOL SELECTION ---
TOOL: suggest_command
JUSTIFY: Need detailed memory info for webapp

--- SUGGESTED ---
docker stats webapp --no-stream
docker exec webapp ps aux --sort=-%mem | head -10

--- OUTPUT ---
CONTAINER   CPU %   MEM %   MEM LIMIT
webapp      2.50    85.00   512MB

PID USER      %MEM   COMMAND
1  node       45.2   npm start
8  node       30.1   node worker.js

--- TOOL SELECTION ---
TOOL: plan_next
JUSTIFY: Found memory hogs, need next step

--- PLAN NEXT ---
1. Check what npm start is doing (check logs)
2. Analyze worker.js memory pattern
3. Consider restarting container temporarily

--- TOOL SELECTION ---
TOOL: suggest_command
JUSTIFY: Need to check application logs

--- SUGGESTED ---
docker logs --tail 100 webapp

--- OUTPUT ---
<logs showing repeated heap allocation>

--- TOOL SELECTION ---
TOOL: extract_errors
JUSTIFY: Need to find errors in logs

--- EXTRACT ERRORS ---
- "FATAL: JavaScript heap out of memory"
- "Error: Cannot allocate memory"

--- TOOL SELECTION ---
TOOL: conclude_success
JUSTIFY: Found root cause

--- CONCLUSION ---
Root cause: Node.js heap out of memory in webapp container
Fix applied: Increase container memory limit or optimize Node.js heap
```

---

## Implementation Phases

### Phase 1: Tool Selection + Suggest Command
- Add `ReactTool` enum with all tools
- Add tool selection prompt
- Add `ToolDecision` and `ToolResult` types
- Default to `suggest_command` for backward compatibility
- Modify handler to show tool selection

### Phase 2: Analysis Tools
- Add summarize tool
- Add extract_errors, extract_warnings tools
- Add extract_metrics tool
- Add compare tool

### Phase 3: Planning Tools
- Add plan_next tool
- Add narrow_focus tool
- Add rethink tool

### Phase 4: Full System
- Add all remaining tools
- Add tool-specific handlers
- Full backward compatibility removal
- Learning system integration
