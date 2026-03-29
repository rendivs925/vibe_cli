use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use domain::entities::Hypothesis;
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;
use crate::services::react_tools::prompts::planning as prompts;

/// Handler for plan_next tool
pub struct PlanNextHandler;

#[async_trait]
impl ReactToolHandler for PlanNextHandler {
    fn name(&self) -> &str {
        "plan_next"
    }
    
    fn description(&self) -> &str {
        "Propose 2-3 next steps"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Planning
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let plan = generate_next_steps(context);
        
        Ok(ToolResult::new(ReactTool::PlanNext)
            .with_output(format!("Proposed next steps:\n{}", plan))
            .with_next_tool(ReactTool::SuggestCommand))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::plan_next_prompt(context)
    }
}

/// Handler for narrow_focus tool
pub struct NarrowFocusHandler;

#[async_trait]
impl ReactToolHandler for NarrowFocusHandler {
    fn name(&self) -> &str {
        "narrow_focus"
    }
    
    fn description(&self) -> &str {
        "Narrow investigation scope"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Planning
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let focus = generate_focus(context);
        
        // Create a hypothesis from the narrowed focus
        let hypotheses = vec![Hypothesis {
            description: focus.clone(),
            confidence: 0.8,
            supporting_facts: vec![],
            created_at: chrono::Utc::now(),
        }];
        
        Ok(ToolResult::new(ReactTool::NarrowFocus)
            .with_output(format!("Narrowed focus:\n{}", focus))
            .with_hypotheses(hypotheses)
            .with_next_tool(ReactTool::SuggestCommand))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::narrow_focus_prompt(context)
    }
}

/// Handler for branch tool
pub struct BranchHandler;

#[async_trait]
impl ReactToolHandler for BranchHandler {
    fn name(&self) -> &str {
        "branch"
    }
    
    fn description(&self) -> &str {
        "Explore alternative approaches"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Planning
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let branches = generate_branches(context);
        
        Ok(ToolResult::new(ReactTool::Branch)
            .with_output(format!("Alternative approaches:\n{}", branches))
            .with_next_tool(ReactTool::Prioritize))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::branch_prompt(context)
    }
}

/// Handler for rethink tool
pub struct RethinkHandler;

#[async_trait]
impl ReactToolHandler for RethinkHandler {
    fn name(&self) -> &str {
        "rethink"
    }
    
    fn description(&self) -> &str {
        "Take completely new approach"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Planning
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let approach = generate_new_approach(context);
        
        // Create a hypothesis for the new approach
        let hypotheses = vec![Hypothesis {
            description: "New approach required".to_string(),
            confidence: 0.6,
            supporting_facts: vec![],
            created_at: chrono::Utc::now(),
        }];
        
        Ok(ToolResult::new(ReactTool::Rethink)
            .with_output(format!("New approach suggested:\n{}", approach))
            .with_hypotheses(hypotheses)
            .with_next_tool(ReactTool::SuggestCommand))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::rethink_prompt(context)
    }
}

/// Handler for prioritize tool
pub struct PrioritizeHandler;

#[async_trait]
impl ReactToolHandler for PrioritizeHandler {
    fn name(&self) -> &str {
        "prioritize"
    }
    
    fn description(&self) -> &str {
        "Rank options"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Planning
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let ranking = generate_priorities(context);
        
        Ok(ToolResult::new(ReactTool::Prioritize)
            .with_output(format!("Prioritized actions:\n{}", ranking))
            .with_next_tool(ReactTool::SuggestCommand))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::prioritize_prompt(context)
    }
}

// Helper functions

fn generate_next_steps(context: &RetrievedContext) -> String {
    let query_lower = context.goal.to_lowercase();
    let mut steps = Vec::new();
    
    // Generic investigation steps
    steps.push("1. Gather more diagnostic data about the current state".to_string());
    
    if query_lower.contains("error") || query_lower.contains("fail") {
        steps.push("2. Examine error logs and identify the specific failure point".to_string());
        steps.push("3. Check related configurations that might be causing the error".to_string());
    } else if query_lower.contains("performance") || query_lower.contains("slow") {
        steps.push("2. Profile the system to identify bottlenecks".to_string());
        steps.push("3. Check resource utilization (CPU, memory, I/O)".to_string());
    } else if query_lower.contains("config") || query_lower.contains("setup") {
        steps.push("2. Review configuration files for correctness".to_string());
        steps.push("3. Verify dependencies and environment settings".to_string());
    } else {
        steps.push("2. Analyze the collected data for patterns".to_string());
        steps.push("3. Formulate and test hypotheses about the issue".to_string());
    }
    
    steps.join("\n")
}

fn generate_focus(context: &RetrievedContext) -> String {
    let query_lower = context.goal.to_lowercase();
    
    if !context.facts.is_empty() {
        // Focus on most recent fact (last line)
        let latest_fact = context.facts.lines().last().unwrap_or("Unknown");
        format!(
            "Focus on: {}\nThis appears to be the most relevant recent finding.",
            latest_fact
        )
    } else if query_lower.contains("service") {
        "Focus on: Service status and configuration\nInvestigate systemd units, ports, and logs.".to_string()
    } else if query_lower.contains("file") || query_lower.contains("disk") {
        "Focus on: Filesystem and disk usage\nCheck available space, inodes, and large files.".to_string()
    } else if query_lower.contains("process") {
        "Focus on: Process state and resource usage\nInvestigate CPU, memory, and open files.".to_string()
    } else {
        "Focus on: Current error state or most recent output\nBuild understanding step by step.".to_string()
    }
}

fn generate_branches(_context: &RetrievedContext) -> String {
    vec![
        "Option A: Continue current investigation path - verify assumptions with more data",
        "Option B: Try alternative diagnostic approach - examine from a different angle",
        "Option C: Reset and restart investigation - clear facts and begin fresh",
    ].join("\n")
}

fn generate_new_approach(context: &RetrievedContext) -> String {
    let _query_lower = context.goal.to_lowercase();
    
    if context.steps > 5 {
        format!(
            "After {} steps without clear resolution, consider:\n\n\
            1. Break down the problem into smaller sub-problems\n\
            2. Verify basic assumptions (is the service actually installed?)\n\
            3. Look for external factors (network, permissions, dependencies)\n\
            4. Consider escalating to documentation or human expertise",
            context.steps
        )
    } else {
        "New approach: Start with the basics\n\n\
        1. Verify the system is in the expected state\n\
        2. Check prerequisites and dependencies\n\
        3. Examine most recent changes or events\n\
        4. Build a timeline of what happened".to_string()
    }
}

fn generate_priorities(_context: &RetrievedContext) -> String {
    vec![
        "High: Verify system state and gather current diagnostic data",
        "Medium: Analyze findings and identify root cause",
        "Low: Apply fixes and verify resolution",
    ].join("\n")
}

/// Build the default planning tool handlers
pub fn build_planning_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::PlanNext, Arc::new(PlanNextHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::NarrowFocus, Arc::new(NarrowFocusHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Branch, Arc::new(BranchHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Rethink, Arc::new(RethinkHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Prioritize, Arc::new(PrioritizeHandler) as Arc<dyn ReactToolHandler>),
    ]
}
