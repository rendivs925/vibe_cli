# Vibe CLI: Automatic Mode Selection with Multi-Agent Collaboration

## Overview

This document outlines a comprehensive plan to enhance the Vibe CLI with intelligent automatic mode selection and collaborative multi-agent workflows. The system will automatically route user inputs to appropriate execution modes without requiring manual flag selection, while leveraging multiple specialized agents working together for complex tasks.

## Current Architecture Analysis

### Existing CLI Modes
- **Agent**: Multi-step agent execution with planning
- **AI Agent**: Enhanced reasoning with tool calls
- **Build**: Safe code modifications with RAG context
- **Explain**: Code/file explanation
- **RAG**: Codebase Q&A with retrieval
- **Query**: Single command generation (default)
- **Chat**: Interactive conversation
- **Test**: Test execution with monitoring

### Existing Agent Infrastructure
- **AgentService**: Main orchestrator with RAG integration
- **ParallelAgentOrchestrator**: Parallel task execution
- **StreamingAgentOrchestrator**: Real-time execution
- **TaskDecomposer**: Intelligent task breakdown
- **InputClassifier**: Input categorization system

## Core Enhancement: Automatic Mode Selection

### Input Classification System
The existing `InputClassifier` categorizes inputs into:
- `Command`: Shell command requests
- `Question`: Questions about code/project
- `Conversation`: General chat
- `CodeSnippet`: Code to analyze/explain
- `FileOperation`: File operations
- `SystemQuery`: System information queries
- `Ambiguous`: Cannot determine with confidence

### Enhanced Classification for Collaboration
```rust
pub struct CollaborativeClassification {
    input_type: InputType,
    complexity_score: f32,           // 0.0-1.0 (simple to complex)
    collaboration_required: bool,
    optimal_agent_count: usize,
    suggested_agents: Vec<AgentType>,
    coordination_strategy: CoordinationStrategy,
}
```

### Automatic Mode Routing Logic
- **Simple Commands** (score < 0.3): Route to `handle_query_streaming`
- **Code Questions** (score 0.3-0.6): Route to `handle_rag` + research agent
- **Code Analysis** (CodeSnippet): Route to `handle_explain`
- **File Operations** (FileOperation): Route to `handle_build`
- **Complex Tasks** (score > 0.6): Route to multi-agent collaboration
- **Conversations**: Route to `handle_chat`

## Multi-Agent Collaboration Architecture

### Specialized Agent Types
- **CoordinatorAgent**: Orchestrates sub-agents and manages workflows
- **ResearchAgent**: Information gathering and codebase analysis
- **PlanningAgent**: Task planning and dependency analysis
- **ExecutionAgent**: Safe command and file operations
- **ValidationAgent**: Safety checking and result verification
- **OptimizationAgent**: Performance analysis and learning

### Agent Communication Infrastructure
```rust
pub struct AgentCommunicationBus {
    agent_channels: HashMap<AgentId, mpsc::Sender<AgentMessage>>,
    coordinator_channel: mpsc::Sender<CoordinatorMessage>,
    event_bus: broadcast::Sender<SystemEvent>,
}
```

### Task Decomposition Strategies
```rust
pub enum DecompositionStrategy {
    SingleAgent,           // Simple tasks
    ParallelSpecialists,   // Independent subtasks by specialty
    SequentialPipeline,    // Research → Plan → Execute → Validate
    HybridCollaborative,   // Complex interdependent workflows
}
```

## Real-World Use Cases

### 1. Feature Implementation
**Input**: "add user authentication to the web app"
**Classification**: FileOperation + High Complexity
**Agent Workflow**:
- ResearchAgent: Analyze current auth patterns
- PlanningAgent: Design integration plan
- ExecutionAgent: Implement auth middleware
- ValidationAgent: Test auth flows

### 2. Performance Optimization
**Input**: "optimize the database queries"
**Classification**: SystemQuery + High Complexity
**Agent Workflow**:
- ResearchAgent: Analyze query patterns
- PlanningAgent: Identify optimization opportunities
- ExecutionAgent: Implement optimizations
- ValidationAgent: Verify improvements
- OptimizationAgent: Learn for future tasks

### 3. Bug Investigation
**Input**: "debug the memory leak"
**Classification**: SystemQuery + High Complexity
**Agent Workflow**:
- ResearchAgent: Gather metrics and logs
- PlanningAgent: Create debugging strategy
- ExecutionAgent: Run diagnostics
- ValidationAgent: Verify fixes

## Implementation Roadmap

### Phase 1: Foundation (Specialized Agents)
- [ ] Create `SpecializedAgent` trait and base implementations
- [ ] Implement basic agent communication protocols
- [ ] Extend `InputClassifier` for complexity assessment
- [ ] Add confidence-based routing with fallbacks

### Phase 2: Coordination Infrastructure
- [ ] Build `AgentCommunicationBus` for inter-agent messaging
- [ ] Implement `SharedContext` for knowledge sharing
- [ ] Create coordination algorithms for different strategies
- [ ] Add real-time collaboration visibility

### Phase 3: Integration & Learning
- [ ] Integrate with existing CLI modes in `CliApp::run()`
- [ ] Add user feedback collection for agent performance
- [ ] Implement learning system to optimize agent combinations
- [ ] Create mode override capabilities (`--force-mode`, `--auto`)

### Phase 4: Advanced Features
- [ ] Agent specialization based on project type
- [ ] Cross-task learning and performance optimization
- [ ] Multi-modal input handling (voice, images, etc.)
- [ ] Predictive mode suggestions based on user history

## Technical Integration Points

### CLI Flow Modification
Modify `presentation/src/cli.rs` to add automatic routing:

```rust
// In CliApp::run() before explicit flag checks
if !has_explicit_mode(&cli) {
    if let Some(classifier) = &self.input_classifier {
        match classifier.classify_for_multi_agent(&args_str).await {
            Ok(classification) if classification.confidence >= threshold => {
                return self.handle_automatic_mode(&args_str, &classification).await;
            }
            _ => {} // Fall back to default behavior
        }
    }
}
```

### New Method: handle_automatic_mode
```rust
async fn handle_automatic_mode(
    &mut self,
    input: &str,
    classification: &CollaborativeClassification
) -> Result<()> {
    if classification.collaboration_required {
        self.handle_collaborative_task(input, classification).await
    } else {
        self.route_to_single_mode(input, &classification.input_type).await
    }
}
```

### Enhanced InputClassifier
Extend `infrastructure/src/input_classifier.rs`:
- Add `classify_for_multi_agent()` method
- Implement complexity scoring
- Add agent type recommendations
- Include project context awareness

## Safety & Reliability Measures

### Conservative Defaults
- Maintain all existing safety confirmations
- Never auto-select destructive operations without user approval
- Always show execution plans for multi-step operations
- Preserve manual flag override capability

### Error Handling & Recovery
- Graceful fallback to single-agent mode if collaboration fails
- Clear error messages with manual override instructions
- Recovery options for failed classifications
- Resource limits and timeout protections

### Performance Optimizations
- Lazy agent initialization to minimize startup overhead
- Caching of classification results (existing 1-hour TTL)
- Resource pooling for agent instances
- Parallel execution where possible

## User Experience Enhancements

### Transparency Features
- Show auto-selected mode and confidence scores (when verbose)
- Display agent collaboration process in real-time
- Provide reasoning for mode selections
- Allow user corrections to improve future classifications

### Override Capabilities
- `--auto` flag to explicitly enable automatic mode
- `--force-mode <mode>` to override automatic selection
- `--no-auto` to disable automatic routing
- All existing flags continue to work unchanged

### Learning & Adaptation
- Collect user feedback on mode selections
- Learn from corrections to improve accuracy
- Store learning data in user config directory
- Provide suggestions for optimal usage patterns

## Benefits

### For Users
- **Natural Interaction**: Describe tasks in plain English without remembering flags
- **Intelligent Scaling**: Simple tasks use single agents, complex ones coordinate multiple specialists
- **Transparent Collaboration**: See how different agents contribute to solutions
- **Improved Accuracy**: Specialized agents excel at their specific domains
- **Faster Workflows**: Parallel processing and optimized execution paths

### For the System
- **Scalability**: Handle increasingly complex requests through specialization
- **Reliability**: Redundant capabilities with cross-agent validation
- **Adaptability**: Learn optimal agent combinations per task type
- **Maintainability**: Modular architecture for easy extension and updates

## Risk Mitigation

### Backward Compatibility
- All existing functionality remains unchanged
- Manual flags continue to work exactly as before
- Automatic mode is opt-in with easy disable options
- Conservative defaults prevent unexpected behavior

### Testing Strategy
- Comprehensive unit tests for classification accuracy
- Integration tests for multi-agent collaboration
- Performance benchmarks for different complexity levels
- User acceptance testing with various input types

### Monitoring & Observability
- Detailed logging of agent coordination and performance
- Metrics collection for classification accuracy and user satisfaction
- Error tracking and automated recovery mechanisms
- Performance monitoring for agent resource usage

## Success Metrics

### User Experience
- Reduction in manual flag usage (>80% for common tasks)
- Increase in task completion success rate
- Decrease in user confusion and mode selection errors
- Positive user feedback on natural interaction

### System Performance
- Classification accuracy >95% for clear inputs
- Multi-agent coordination overhead <10% for simple tasks
- Average response time improvement for complex tasks
- Stable resource usage across different load levels

## Conclusion

This plan transforms the Vibe CLI from a flag-based interface into an intelligent collaborative assistant that understands user intent and automatically scales its response complexity. By leveraging multiple specialized agents working together, the system can handle increasingly sophisticated user requests while maintaining the safety, reliability, and performance standards of the existing codebase.

The implementation builds directly on existing infrastructure, ensuring backward compatibility while providing a foundation for future enhancements in AI-assisted development workflows.</content>
<parameter name="filePath">/home/rendi/projects/vibe_cli/docs/automatic_mode_selection_plan.md