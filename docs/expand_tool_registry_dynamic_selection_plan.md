# Vibe CLI: Expand Tool Registry + Dynamic Selection + Learning

## Executive Summary

This document presents a comprehensive plan for expanding Vibe CLI's tool registry from 13 hardcoded tools to 30+ dynamic tools, implementing intelligent selection with brief descriptions, and adding user preference learning system.

## Design Philosophy

**From**: 13 hardcoded tools with fixed parameters  
**To**: Dynamic tool discovery with capability-based matching + user learning

### Core Principles

1. **Tool Expansion** - Significantly increase available tool coverage
2. **Brief Descriptions** - Clear, concise tool descriptions for quick decisions
3. **User Learning** - Adapt tool recommendations based on usage patterns
4. **Conservative Controls** - Maintain user approval for all operations

---

## Tool Registry Expansion Plan

### Phase 1: Tool Categories Expansion (Week 1-2)

#### Current State Analysis
- **13 hardcoded tools** with fixed functionality
- **Static command mappings** with no flexibility
- **No learning capabilities** or user preference tracking

#### Target Tool Categories

**File Operations** (expand from 3 to 8):
- `file_batch_read` - Read multiple files efficiently
- `file_smart_search` - Search within files with context
- `file_differential_read` - Read only changed portions
- `file_metadata_extract` - Extract file properties
- `file_backup_restore` - Create/restore backups
- `file_permission_check` - Verify access rights
- `file_encoding_convert` - Convert between encodings

**System Commands** (expand from 2 to 6):
- `package_manager_wrapper` - Safe package operations
- `build_tool_orchestrator` - Multi-tool build coordination
- `test_runner_smart` - Intelligent test execution
- `dependency_analyzer` - Project dependency analysis
- `environment_setup` - Configure dev environments
- `process_manager` - External process coordination

**Code Analysis** (expand from 2 to 6):
- `code_linter_adaptive` - Context-aware linting
- `code_formatter_style` - Project style formatting
- `code_complexity_analyzer` - Measure code complexity
- `code_security_scanner` - Security vulnerability detection
- `code_refactoring_suggester` - Suggest improvements
- `code_duplicate_detector` - Find duplicate code patterns

**Web & External** (expand from 2 to 6):
- `web_search_contextual` - Context-aware web queries
- `api_call_manager` - Safe external API interactions
- `documentation_fetcher` - Get relevant documentation
- `version_checker` - Check dependency versions
- `repository_analyzer` - Git repo analysis
- `release_monitor` - Track project releases

**Collaboration** (add 4 new tools):
- `conflict_resolver` - Merge conflict assistance
- `review_analyzer` - Code review suggestions
- `commit_message_generator` - Smart commit messages
- `changelog_generator` - Automated changelog creation

### Tool Metadata System

```rust
pub struct ExpandedTool {
    id: String,
    name: String,
    category: ToolCategory,
    capabilities: Vec<Capability>,
    risk_level: RiskLevel,
    resource_requirements: ResourceRequirements,
    input_types: Vec<InputType>,
    output_types: Vec<OutputType>,
    dependencies: Vec<String>,  // Other tools needed
    conflicts: Vec<String>,      // Tools incompatible
    user_rating: Option<f32>,  // Learn from usage
    brief_description: String,     // One-line description
    success_rate: f32,           // Historical success
}

pub enum ToolCategory {
    FileOperations,
    SystemCommands,
    CodeAnalysis,
    WebExternal,
    Collaboration,
}

pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

---

## Dynamic Tool Selection System

### Phase 2: Intent Analysis Engine (Week 3-4)

#### A. Goal Understanding
```rust
pub struct IntentAnalyzer {
    llm_client: LlmClient,
    context_analyzer: ContextAnalyzer,
    pattern_recognizer: PatternRecognizer,
}

impl IntentAnalyzer {
    pub async fn analyze_intent(
        &self,
        user_input: &str,
        project_context: &ProjectContext,
    ) -> Result<IntentAnalysis> {
        // 1. Extract user goal from natural language
        // 2. Identify required capabilities
        // 3. Determine task complexity
        // 4. Match to tool categories
    }
}
```

#### B. Capability-Based Matching
```rust
pub struct CapabilityMatcher {
    tool_registry: ExpandedToolRegistry,
    context_analyzer: ContextAnalyzer,
    preference_engine: PreferenceEngine,
}

impl CapabilityMatcher {
    pub async fn find_matching_tools(
        &self,
        intent: &IntentAnalysis,
        context: &ProjectContext,
    ) -> Result<Vec<ToolSuggestion>> {
        // 1. Match capabilities to requirements
        // 2. Filter by project type and context
        // 3. Apply user preferences and learning
        // 4. Rank by success rate and relevance
    }
}
```

#### C. Brief Description System
**Format**: "Tool: Purpose (risk level)"
**Examples**:
- `file_batch_read`: "Read multiple files quickly (low risk)"
- `code_linter_adaptive`: "Check code quality with project rules (medium risk)"
- `package_manager_wrapper`: "Safe package operations (high risk)"
- `web_search_contextual`: "Find relevant information online (medium risk)"

---

## User Learning System

### Phase 3: Preference Tracking (Week 5-6)

#### A. Usage Pattern Analysis
```rust
pub struct UsageAnalyzer {
    command_history: Vec<UserCommand>,
    tool_selection_patterns: HashMap<String, UsagePattern>,
    success_tracking: HashMap<String, SuccessMetrics>,
    rejection_patterns: HashMap<String, RejectionPattern>,
}

pub struct UsagePattern {
    tool_combination: Vec<String>,
    project_types: Vec<ProjectType>,
    success_rate: f32,
    time_of_day: Vec<TimeOfDay>,
    user_satisfaction: f32,
}
```

#### B. Adaptive Scoring Algorithm
```rust
pub struct AdaptiveScorer {
    base_weights: ScoringWeights,
    learned_adjustments: HashMap<String, f32>,
    context_multiplier: ContextMultiplier,
}

impl AdaptiveScorer {
    pub fn calculate_tool_score(
        &self,
        tool: &ExpandedTool,
        context: &ProjectContext,
        user_preferences: &UserPreferences,
    ) -> f32 {
        // 1. Base score from capabilities match
        // 2. Apply learned adjustments from user history
        // 3. Weight by project context relevance
        // 4. Factor in user satisfaction ratings
    }
}
```

#### C. User Profile Storage
```rust
pub struct UserPreferences {
    tool_ratings: HashMap<String, f32>,
    preferred_combinations: HashMap<TaskType, Vec<String>>,
    risk_tolerance: RiskTolerance,
    project_specific_preferences: HashMap<String, ProjectPreferences>,
    learning_data: LearningMetrics,
}
```

---

## Implementation Strategy

### Phase 1: Tool Registry Expansion (Week 1-2)
1. **Expand infrastructure/tools.rs** with 17 new tools
2. **Implement metadata system** for all tools
3. **Create capability framework** for dynamic matching
4. **Add tool dependency resolution** and conflict detection

### Phase 2: Dynamic Selection Engine (Week 3-4)
1. **Create IntentAnalyzer** for goal understanding
2. **Implement CapabilityMatcher** for tool selection
3. **Add brief description generator** for clear communication
4. **Integrate with existing safety systems**

### Phase 3: Learning System Integration (Week 5-6)
1. **Create UsageAnalyzer** for pattern recognition
2. **Implement AdaptiveScorer** for personalized recommendations
3. **Add UserPreferences** storage and retrieval
4. **Integrate learning with tool selection engine**

### Phase 4: Conservative Controls (Week 7-8)
1. **Maintain user approval** for all tool selections
2. **Add risk assessment** with clear indicators
3. **Implement rollback system** for safety
4. **Ensure learning doesn't override** user consent

---

## Expected Outcomes

### Immediate Benefits
- **130% more tool coverage** (13 → 30+ tools)
- **AI-powered tool selection** instead of hardcoded mappings
- **Brief, clear descriptions** for quick decisions
- **User preference learning** for better recommendations

### Conservative Safety
- **Zero autonomous execution** - always user-approved
- **Clear risk indicators** for all suggested tools
- **Easy rollback** for any operation
- **Granular control** over tool combinations

### Long-term Learning
- **Adaptive recommendations** based on project context
- **Personalized tool suggestions** from usage patterns
- **Improved success rates** through preference tracking
- **Context-aware risk assessment** per project type

---

## Success Metrics

### You've succeeded when:
- **Tool discovery works** for all 30+ tools
- **Brief descriptions** are clear and helpful
- **Learning system improves** recommendations over time
- **User approval flow** prevents unwanted actions
- **Rollback capabilities** restore state correctly

### Integration Testing
- **Verify compatibility** with existing CLI workflows
- **Test learning system** improves recommendations
- **Validate rollback capabilities** work correctly
- **Ensure no autonomous actions** occur without consent

---

## Implementation Checklist

### Tool Registry Expansion
- [ ] Add 17 new tools across 5 categories
- [ ] Implement tool metadata and capability system
- [ ] Create tool dependency and conflict resolution
- [ ] Test all new tools with safety mechanisms

### Dynamic Selection System
- [ ] Create IntentAnalyzer for goal understanding
- [ ] Implement CapabilityMatcher for tool selection
- [ ] Add brief description generation system
- [ ] Integrate user approval workflow

### Learning System
- [ ] Create preference tracking data structures
- [ ] Implement adaptive scoring based on usage
- [ ] Add context-aware learning algorithms
- [ ] Create user profile storage and retrieval

### Conservative Controls
- [ ] Ensure all tool selections require user approval
- [ ] Add risk level indicators and brief explanations
- [ ] Implement comprehensive rollback system
- [ ] Test with various user skill levels

### Integration Testing
- [ ] Verify compatibility with existing CLI workflows
- [ ] Test learning system improves over time
- [ ] Validate rollback capabilities work correctly
- [ ] Ensure no autonomous actions occur without consent

---

## Conclusion

This plan expands Vibe CLI's tool registry from 13 to 30+ dynamic tools, implements intelligent selection with brief descriptions, adds user preference learning, and maintains your conservative control requirements.

The system will learn from user interactions to provide increasingly relevant tool suggestions while always requiring explicit user approval for any operations.

---

*Document created: 2025-01-15*
*Author: Vibe CLI Planning Team*
*Version: 1.0*
## Specific Hardcoded Startup Messages to Disable

### Current Hardcoded Messages to Remove:

1. **'🧠 Starting background intelligence services...'**
   - Location: infrastructure/src/background_supervisor.rs:135
   - Impact: Main startup message that appears automatically

2. **'✅ Background intelligence active'**
   - Location: infrastructure/src/background_supervisor.rs (displayed after services start)
   - Impact: Confirmation message after services initialize

3. **'🤖 Autonomous fix analyzer active - monitoring for errors...'**
   - Location: infrastructure/src/background_supervisor.rs:236 (in start_autonomous_fix_analyzer)
   - Impact: Dangerous autonomous fix system activation

4. **'🔨 Starting compilation watcher...'**
   - Location: infrastructure/src/background_supervisor.rs:191 (in start_compilation_watcher)
   - Impact: Automatic compilation monitoring startup

5. **'📜 Starting log tailer...'**
   - Location: infrastructure/src/background_supervisor.rs:130 (in start_log_tailer)
   - Impact: Automatic log monitoring startup

6. **'📁 File watcher starting on /home/rendi/projects/vibe_cli'**
   - Location: infrastructure/src/background_supervisor.rs:143 (in start_file_watcher)
   - Impact: Automatic file monitoring startup

7. **'📁 Watching for changes (excluding build artifacts)...'**
   - Location: infrastructure/src/background_supervisor.rs:171 (in start_file_watcher)
   - Impact: File watcher active monitoring message

8. **'🔨 Starting compilation watcher...'** (Duplicate message)
   - Location: infrastructure/src/background_supervisor.rs:191
   - Impact: Redundant startup message

9. **'📜 Starting log tailer...'** (Duplicate message)
   - Location: infrastructure/src/background_supervisor.rs:130
   - Impact: Redundant startup message

10. **'✅ Compilation watcher started'**
   - Location: infrastructure/src/background_supervisor.rs:191
   - Impact: Confirmation of compilation watcher startup

11. **'📜 Starting log tailer...'** (Duplicate message)
   - Location: infrastructure/src/background_supervisor.rs:130
   - Impact: Redundant startup message

12. **'📁 File watcher shutting down'**
   - Location: infrastructure/src/background_supervisor.rs:188
   - Impact: File watcher shutdown message

13. **'✅ Log tailer started (monitoring 1 files)'**
   - Location: infrastructure/src/background_supervisor.rs:172
   - Impact: Log tailer startup confirmation

14. **'📁 Watching for changes (excluding build artifacts)...'** (Duplicate message)
   - Location: infrastructure/src/background_supervisor.rs:171
   - Impact: Redundant file watcher monitoring message

15. **'✅ Background intelligence active'** (Duplicate message)
   - Location: Multiple locations in background_supervisor.rs
   - Impact: Confirmation that background services are running

### Removal Strategy

These messages represent the core of the hardcoded background intelligence system that needs to be completely disabled. All should be removed or commented out to achieve:

- **Zero automatic background service startup**
- **No visible background intelligence messages**
- **No autonomous fix analyzer activation**
- **No automatic compilation or file monitoring**

### Files to Modify

-  - Remove all startup messages and automatic service initialization
-  - Remove automatic background supervisor startup logic

This ensures users never see background intelligence services running without their explicit consent.

