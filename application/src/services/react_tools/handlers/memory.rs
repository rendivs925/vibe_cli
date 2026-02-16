use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;
use crate::services::react_tools::prompts::memory as prompts;

/// Handler for show_facts tool
pub struct ShowFactsHandler;

#[async_trait]
impl ReactToolHandler for ShowFactsHandler {
    fn name(&self) -> &str {
        "show_facts"
    }
    
    fn description(&self) -> &str {
        "Show extracted facts"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let facts_display = format_facts(&context.facts);
        
        Ok(ToolResult::new(ReactTool::ShowFacts)
            .with_output(facts_display))
    }
    
    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Display all extracted facts.".to_string()
    }
}

/// Handler for show_hypotheses tool
pub struct ShowHypothesesHandler;

#[async_trait]
impl ReactToolHandler for ShowHypothesesHandler {
    fn name(&self) -> &str {
        "show_hypotheses"
    }
    
    fn description(&self) -> &str {
        "Show current hypotheses"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let hypotheses_display = format_hypotheses(&context.hypotheses);
        
        Ok(ToolResult::new(ReactTool::ShowHypotheses)
            .with_output(hypotheses_display))
    }
    
    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Display all current hypotheses.".to_string()
    }
}

/// Handler for show_history tool
pub struct ShowHistoryHandler;

#[async_trait]
impl ReactToolHandler for ShowHistoryHandler {
    fn name(&self) -> &str {
        "show_history"
    }
    
    fn description(&self) -> &str {
        "Show session history"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        Ok(ToolResult::new(ReactTool::ShowHistory)
            .with_output(format!("Session History:\n\n{}", context.session_history)))
    }
    
    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Display session history.".to_string()
    }
}

/// Handler for show_context tool
pub struct ShowContextHandler;

#[async_trait]
impl ReactToolHandler for ShowContextHandler {
    fn name(&self) -> &str {
        "show_context"
    }
    
    fn description(&self) -> &str {
        "Show full context"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let full_context = format_full_context(context);
        
        Ok(ToolResult::new(ReactTool::ShowContext)
            .with_output(full_context))
    }
    
    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Display full session context.".to_string()
    }
}

/// Handler for show_plan tool
pub struct ShowPlanHandler;

#[async_trait]
impl ReactToolHandler for ShowPlanHandler {
    fn name(&self) -> &str {
        "show_plan"
    }
    
    fn description(&self) -> &str {
        "Show current plan"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, _context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        // This would show the current plan if one exists
        Ok(ToolResult::new(ReactTool::ShowPlan)
            .with_output("Current plan: Investigation in progress. Use plan_next to generate a plan.".to_string()))
    }
    
    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Display current investigation plan.".to_string()
    }
}

/// Handler for compact_session tool
pub struct CompactSessionHandler;

#[async_trait]
impl ReactToolHandler for CompactSessionHandler {
    fn name(&self) -> &str {
        "compact_session"
    }
    
    fn description(&self) -> &str {
        "Compact session history"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let compacted = compact_history(context);
        
        Ok(ToolResult::new(ReactTool::CompactSession)
            .with_output(format!("Session compacted:\n\n{}", compacted)))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::compact_session_prompt(context)
    }
}

// Helper functions

fn format_facts(facts: &str) -> String {
    if facts.trim().is_empty() {
        "No facts extracted yet.".to_string()
    } else {
        format!("Extracted Facts:\n\n{}", facts)
    }
}

fn format_hypotheses(hypotheses: &str) -> String {
    if hypotheses.trim().is_empty() {
        "No hypotheses recorded yet.".to_string()
    } else {
        format!("Current Hypotheses:\n\n{}", hypotheses)
    }
}

fn format_full_context(context: &RetrievedContext) -> String {
    format!(
        "=== FULL SESSION CONTEXT ===\n\n\
        Goal: {}\n\
        Steps: {}\n\n\
        --- Facts ---\n{}\n\n\
        --- Hypotheses ---\n{}\n\n\
        --- History ---\n{}\n\n\
        --- Latest Output ---\n{}\n\n\
        ============================",
        context.goal,
        context.steps,
        context.facts,
        context.hypotheses,
        context.session_history,
        context.latest_output
    )
}

fn compact_history(context: &RetrievedContext) -> String {
    format!(
        "Summary of {} steps:\n\n\
        Goal: {}\n\
        Key Findings: {} facts collected\n\
        Active Hypotheses: {}\n\n\
        Current Status: Investigation in progress",
        context.steps,
        context.goal,
        context.facts.lines().count(),
        context.hypotheses.lines().count()
    )
}

/// Build the default memory tool handlers
pub fn build_memory_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::ShowFacts, Arc::new(ShowFactsHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ShowHypotheses, Arc::new(ShowHypothesesHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ShowHistory, Arc::new(ShowHistoryHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ShowContext, Arc::new(ShowContextHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ShowPlan, Arc::new(ShowPlanHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::CompactSession, Arc::new(CompactSessionHandler) as Arc<dyn ReactToolHandler>),
    ]
}
