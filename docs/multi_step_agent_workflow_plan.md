# Multi-Step Agent Workflow Enhancement Plan

## Overview
This document outlines a comprehensive enhancement to the `--agent` workflow, transforming it from a basic command sequencer into a sophisticated, safe, and user-friendly multi-step execution system with safety integration, progress tracking, and error recovery capabilities.

## Current --agent Workflow Limitations

### Existing Flow
```
User: vibe --agent "set up a web project"

1. AI generates: ["mkdir webapp", "cd webapp && npm init -y", "cd webapp && npm install express"]
2. Shows plan: [1] mkdir webapp, [2] cd webapp && npm init -y, etc.
3. For each command: "Run this command? [y/N]"
4. Executes individually with basic sandboxing
```

### Problems
- **Individual confirmations**: Each command requires separate approval
- **No safety analysis**: Commands aren't assessed for risks or system impact
- **Limited context**: No dependency analysis or prerequisite checking
- **Basic error handling**: Simple pass/fail with sandbox fallback
- **No progress tracking**: No indication of overall plan completion
- **No rollback capability**: Failed steps leave partial changes

## Enhanced Multi-Step Agent Workflow

### Phase 1: Pre-Analysis & Safety Assessment

#### Task Analysis
```
ANALYZING TASK: "set up a web project"

Detected Requirements:
  - Directory creation and navigation
  - Node.js project initialization
  - Package management (npm)
  - Basic file structure setup

System Impact Analysis:
  - Disk space required: ~50MB
  - Network access: Required for npm installs
  - Permissions needed: Standard user access
  - Execution time estimate: 2-3 minutes

Safety Assessment: Low risk - Standard development setup
  - No system-critical operations
  - No privileged commands required
  - Network access for package downloads only
```

#### Command Risk Classification
```rust
enum AgentCommandRisk {
    InfoOnly,           // ls, pwd, cat (no system changes)
    SafeOperations,     // mkdir, echo, cp (low risk)
    NetworkAccess,      // npm install, git clone (medium risk)
    SystemChanges,      // chmod, chown, systemctl (high risk)
    Destructive,        // rm -rf, dd, format (blocked)
    Unknown,            // Requires manual review
}
```

### Phase 2: Enhanced Plan Presentation

#### Structured Plan Display
```
EXECUTION PLAN (4 steps - Estimated: 2-3 minutes):

+---------------------------------------------+
| STEP 1: Project Structure                   |
| Command: mkdir webapp && cd webapp          |
| Purpose: Create project directory and       |
|          navigate to it                      |
| Risk Level: None                            |
| Dependencies: None                          |
| Estimated Time: < 1 second                  |
+---------------------------------------------+

+---------------------------------------------+
| STEP 2: Node.js Initialization              |
| Command: npm init -y                        |
| Purpose: Create package.json with default   |
|          settings                           |
| Risk Level: Low (network access for npm     |
|            registry)                        |
| Dependencies: Node.js installed             |
| Estimated Time: 5-10 seconds                |
+---------------------------------------------+

+---------------------------------------------+
| STEP 3: Framework Installation              |
| Command: npm install express                |
| Purpose: Install Express.js web framework   |
| Risk Level: Low (network access, disk space)|
| Dependencies: npm available                 |
| Estimated Time: 30-60 seconds               |
+---------------------------------------------+

+---------------------------------------------+
| STEP 4: Application Bootstrap               |
| Command: echo "const express = require     |
|            ('express'); ..."                |
| Purpose: Create basic server.js file        |
| Risk Level: None                            |
| Dependencies: Directory exists              |
| Estimated Time: < 1 second                  |
+---------------------------------------------+

Total Impact: ~50MB disk space, Network required
Parallelizable steps: None (sequential dependencies)
```

#### Plan Modification Options
```
MODIFY PLAN:
  (e) Edit step commands
  (r) Remove steps
  (a) Add new steps
  (o) Reorder steps
  (s) Split complex steps
  (m) Merge simple steps
  (p) Preview with different execution options
```

### Phase 3: Smart Confirmation System

#### Batch vs Step-by-Step Execution
```
EXECUTION OPTIONS:

1. Execute complete plan (recommended)
   - All steps run automatically
   - Progress tracking enabled
   - Automatic error recovery

2. Step-by-step execution
   - Confirm each step individually
   - Full control over execution
   - Manual intervention possible

3. Dry run mode
   - Show what would happen
   - Validate commands without execution
   - Test system compatibility

4. Review and modify
   - Edit plan before execution
   - Add safety checks
   - Customize execution parameters

Choose execution mode (1-4) or 'cancel':
```

#### Safety Override Integration
```
SAFETY CONCERNS DETECTED:

- Step 2 & 3 require network access for package downloads
- Untrusted packages may be installed from npm registry
- Disk space usage: ~50MB

Allow network access for package installation? [y/N]
  (y) Allow for this plan only
  (a) Always allow for --agent commands
  (n) Skip network-dependent steps
  (c) Cancel execution
```

### Phase 4: Intelligent Execution Engine

#### Progress Tracking with Real-time Feedback
```
EXECUTING AGENT PLAN...

PROGRESS OVERVIEW
=================
Overall: [████████░░░░░░░░░░░░░░░░░] 3/12 steps (25%)
Current: Installing dependencies...
ETA: ~2 minutes remaining

Step Status:
[OK] Step 1/12: Project structure created (mkdir webapp)
[OK] Step 2/12: Node.js project initialized (npm init -y)
[RUNNING] Step 3/12: Installing Express framework (npm install express)
      Downloading packages...
      Progress: [████████████░░░░░░] 60%
[WAITING] Step 4/12: Waiting for dependencies
[WAITING] Step 5/12: Awaiting manual review
```

#### Dependency-Aware Execution
```
DEPENDENCY CHAIN:

mkdir webapp -> cd webapp -> npm init -> npm install -> create server.js

Execution respects dependencies:
- Step 3 waits for Step 2 completion
- Parallel execution where safe
- Automatic rollback on dependency failures
```

#### Smart Error Recovery
```
STEP 7 FAILED: npm install express (network timeout)

AUTOMATED RECOVERY ATTEMPTS:
1. Retry with different registry: npm install --registry=https://registry.npmjs.org/
2. Clear npm cache and retry: npm cache clean && npm install
3. Use local cache if available

Apply automated recovery? [Y/n]
  (y) Yes, try automated fixes
  (m) Manual intervention
  (s) Skip this step and continue
  (r) Rollback and stop
```

### Phase 5: Post-Execution Analysis & Learning

#### Success Report
```
AGENT EXECUTION COMPLETE

EXECUTION SUMMARY:
- Total steps: 12
- Successful: 11
- Failed: 1 (recovered automatically)
- Duration: 2m 34s
- Peak memory usage: 45MB

CREATED FILES:
- webapp/package.json (2.1KB)
- webapp/package-lock.json (8.4KB)
- webapp/node_modules/ (48MB)
- webapp/server.js (156B)

NEXT STEPS SUGGESTED:
1. Start development server: cd webapp && npm start
2. Open in browser: http://localhost:3000
3. Add additional middleware: npm install cors helmet
4. Set up testing: npm install --save-dev jest
```

#### Learning Integration
```yaml
# Stored in ~/.vibe_cli/agent_learning.yaml
agent_patterns:
  web_project_setup:
    success_rate: 0.95
    average_duration: "2m 30s"
    common_failures:
      - network_timeouts: "retry with different registry"
      - disk_space: "clear npm cache first"
    optimizations:
      - parallel_install: "npm install express cors helmet concurrently"
      - prefer_yarn: "if yarn available, use instead of npm"

user_preferences:
  network_access: allow_for_agent
  error_recovery: automatic
  confirmation_level: batch_execution
```

## Technical Architecture

### Core Components

#### AgentPlan Structure
```rust
struct AgentPlan {
    steps: Vec<AgentStep>,
    metadata: PlanMetadata,
    safety_assessment: SafetyAssessment,
    dependencies: DependencyGraph,
}

struct AgentStep {
    id: String,
    command: String,
    description: String,
    risk_level: AgentCommandRisk,
    estimated_duration: Duration,
    dependencies: Vec<String>,
    rollback_command: Option<String>,
}
```

#### Execution Engine
```rust
struct AgentExecutor {
    plan: AgentPlan,
    progress_tracker: ProgressTracker,
    safety_engine: SafetyEngine,
    error_recovery: ErrorRecoveryEngine,
    learning_engine: LearningEngine,
}

impl AgentExecutor {
    async fn execute_plan(&mut self) -> Result<ExecutionResult> {
        // Pre-execution validation
        // Safety assessment
        // Dependency resolution
        // Progress tracking
        // Error recovery
        // Learning updates
    }
}
```

### Configuration System

#### Agent Configuration
```yaml
agent:
  # Safety settings
  safety_level: smart                    # paranoid | smart | permissive
  require_step_confirmations: false      # true for step-by-step mode
  auto_approve_safe_commands: true
  allow_network_access: ask              # always | ask | never

  # Execution settings
  max_execution_time: 300                # seconds per step
  enable_parallel_execution: true
  enable_rollback: true
  enable_learning: true

  # UI settings
  show_progress_bars: true
  show_eta_estimates: true
  show_detailed_errors: true
  color_output: auto                     # always | never | auto

  # Recovery settings
  max_retry_attempts: 3
  retry_delay_seconds: 5
  enable_automated_recovery: true
```

#### Command-Specific Rules
```yaml
command_rules:
  npm:
    network_required: true
    disk_intensive: true
    rollback_command: "npm uninstall {package}"
    safety_level: low

  git:
    network_required: true
    destructive_allowed: false
    common_failures: ["network_timeout", "auth_failure"]

  mkdir:
    safety_level: safe
    no_network: true
    rollback_command: "rmdir {path}"
```

## CLI Integration

### Enhanced Flags
```bash
# Basic usage
vibe --agent "setup web project"

# Advanced options
vibe --agent --step-by-step "setup web project"     # Confirm each step
vibe --agent --dry-run "setup web project"          # Preview only
vibe --agent --permissive "setup web project"       # Minimal safety checks
vibe --agent --network=allow "setup web project"    # Allow network access
vibe --agent --parallel "setup web project"         # Parallel execution where safe

# Safety options
vibe --agent --safety=paranoid "setup web project"  # Maximum safety checks
vibe --agent --no-rollback "setup web project"      # Disable rollback
vibe --agent --learn "setup web project"            # Contribute to learning

# Output options
vibe --agent --quiet "setup web project"            # Minimal output
vibe --agent --verbose "setup web project"          # Detailed progress
vibe --agent --json "setup web project"             # Machine-readable output
```

### Interactive Commands
```
During execution:
  (p) Pause execution
  (r) Resume execution
  (s) Skip current step
  (c) Cancel entire plan
  (m) Modify current step
  (h) Show help

After completion:
  (l) View execution log
  (e) Edit created files
  (t) Run tests if available
  (d) Deploy application
  (r) Rollback changes
```

## Safety & Compliance

### Multi-Layer Safety System

#### Pre-Execution Safety
- **Command validation**: Syntax and semantic checking
- **Risk assessment**: Automatic risk level assignment
- **Dependency verification**: Ensure prerequisites are met
- **System compatibility**: Verify command availability

#### Runtime Safety
- **Sandboxing**: Isolated command execution where possible
- **Resource limits**: CPU, memory, and time constraints
- **Network controls**: Explicit network access permissions
- **File system isolation**: Restrict file system access

#### Post-Execution Safety
- **Change auditing**: Track all system modifications
- **Rollback capability**: Automated cleanup on failures
- **Integrity checking**: Verify system state after changes
- **Learning updates**: Improve future safety assessments

### Compliance Features

#### Audit Logging
```json
{
  "execution_id": "agent_20241201_143052",
  "user": "developer",
  "task": "setup web project",
  "start_time": "2024-12-01T14:30:52Z",
  "end_time": "2024-12-01T14:33:26Z",
  "steps_executed": 12,
  "steps_failed": 0,
  "commands_run": [
    {"step": 1, "command": "mkdir webapp", "status": "success", "duration_ms": 150},
    {"step": 3, "command": "npm install express", "status": "success", "duration_ms": 45230}
  ],
  "system_changes": {
    "files_created": ["webapp/package.json", "webapp/server.js"],
    "directories_created": ["webapp", "webapp/node_modules"],
    "disk_usage_mb": 48.5,
    "network_access": true
  }
}
```

#### Regulatory Compliance
- **GDPR**: Data handling transparency
- **SOX**: Change tracking and auditability
- **ISO 27001**: Security control implementation
- **Enterprise policies**: Customizable safety rules

## Implementation Roadmap

### Phase 1: Foundation (Week 1-2)
- [ ] Implement AgentPlan structure and parsing
- [ ] Add basic multi-step execution framework
- [ ] Integrate with existing safety system
- [ ] Create progress tracking UI

### Phase 2: Safety & Intelligence (Week 3-4)
- [ ] Implement command risk assessment
- [ ] Add dependency analysis and resolution
- [ ] Create safety override system
- [ ] Implement automated error recovery

### Phase 3: Advanced Features (Week 5-6)
- [ ] Add parallel execution capabilities
- [ ] Implement rollback system
- [ ] Create learning and optimization engine
- [ ] Add comprehensive configuration system

### Phase 4: Polish & Testing (Week 7-8)
- [ ] Comprehensive testing across scenarios
- [ ] Performance optimization
- [ ] Documentation and user guides
- [ ] Enterprise compliance features

## Success Metrics

### User Experience
- **Task completion rate**: >95% for supported tasks
- **User satisfaction**: >4.5/5 rating
- **Error recovery success**: >80% automatic resolution
- **Time savings**: >60% faster than manual execution

### Safety & Reliability
- **Zero data loss**: No accidental destructive operations
- **Audit compliance**: 100% change tracking
- **System stability**: <0.1% system-impacting failures
- **Recovery success**: >90% successful error recovery

### Technical Performance
- **Execution speed**: <10% overhead vs manual commands
- **Resource usage**: <50MB additional memory
- **Scalability**: Support for 50+ step plans
- **Compatibility**: Works on all supported platforms

This enhanced multi-step agent workflow transforms Vibe CLI from a simple automation tool into a sophisticated, safe, and intelligent execution platform that can handle complex multi-step tasks with professional-grade reliability and user experience.</content>
<parameter name="filePath">docs/multi_step_agent_workflow_plan.md