use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;
use crate::services::react_tools::prompts::resolution as prompts;

/// Handler for conclude_success tool
pub struct ConcludeSuccessHandler;

#[async_trait]
impl ReactToolHandler for ConcludeSuccessHandler {
    fn name(&self) -> &str {
        "conclude_success"
    }
    
    fn description(&self) -> &str {
        "Problem solved - end session"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Resolution
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let summary = generate_success_summary(context);
        
        Ok(ToolResult::new(ReactTool::ConcludeSuccess)
            .with_output(summary)
            .conclude())
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::conclude_success_prompt(context)
    }
}

/// Handler for conclude_fail tool
pub struct ConcludeFailHandler;

#[async_trait]
impl ReactToolHandler for ConcludeFailHandler {
    fn name(&self) -> &str {
        "conclude_fail"
    }
    
    fn description(&self) -> &str {
        "Cannot solve - end session"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Resolution
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let summary = generate_failure_summary(context);
        
        Ok(ToolResult::new(ReactTool::ConcludeFail)
            .with_output(summary)
            .conclude())
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::conclude_fail_prompt(context)
    }
}

/// Handler for escalate tool
pub struct EscalateHandler;

#[async_trait]
impl ReactToolHandler for EscalateHandler {
    fn name(&self) -> &str {
        "escalate"
    }
    
    fn description(&self) -> &str {
        "Need human assistance"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Resolution
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let escalation_info = generate_escalation_info(context);
        
        Ok(ToolResult::new(ReactTool::Escalate)
            .with_output(escalation_info)
            .conclude())
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::escalate_prompt(context)
    }
}

/// Handler for defer tool
pub struct DeferHandler;

#[async_trait]
impl ReactToolHandler for DeferHandler {
    fn name(&self) -> &str {
        "defer"
    }
    
    fn description(&self) -> &str {
        "Defer task for later"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Resolution
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let defer_info = generate_defer_info(context);
        
        Ok(ToolResult::new(ReactTool::Defer)
            .with_output(defer_info)
            .conclude())
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::defer_prompt(context)
    }
}

// Helper functions

fn generate_success_summary(context: &RetrievedContext) -> String {
    format!(
        "=== SESSION COMPLETED SUCCESSFULLY ===\n\n\
        Goal: {}\n\n\
        Root Cause: {}\n\n\
        Resolution: Issue has been resolved.\n\n\
        Steps Taken: {}\n\n\
        Key Findings:\n{}\n\n\
        ====================================",
        context.goal,
        infer_root_cause(context),
        context.steps,
        context.facts
    )
}

fn generate_failure_summary(context: &RetrievedContext) -> String {
    format!(
        "=== SESSION ENDED WITHOUT RESOLUTION ===\n\n\
        Goal: {}\n\n\
        Reason: Unable to resolve with current information/tools.\n\n\
        Steps Taken: {}\n\n\
        What Was Tried:\n{}\n\n\
        Recommendation: Consider manual investigation or additional diagnostic data.\n\n\
        =======================================",
        context.goal,
        context.steps,
        context.session_history.lines().take(10).collect::<Vec<_>>().join("\n")
    )
}

fn generate_escalation_info(context: &RetrievedContext) -> String {
    format!(
        "=== ESCALATION REQUIRED ===\n\n\
        Goal: {}\n\n\
        Reason for Escalation:\n{}\n\n\
        Current Findings:\n{}\n\n\
        Context for Human:\n\
        - Steps taken: {}\n\
        - Facts collected: {}\n\
        - Hypotheses tested: {}\n\n\
        ==========================",
        context.goal,
        determine_escalation_reason(context),
        context.facts,
        context.steps,
        context.facts.lines().count(),
        context.hypotheses.lines().count()
    )
}

fn generate_defer_info(context: &RetrievedContext) -> String {
    format!(
        "=== TASK DEFERRED ===\n\n\
        Goal: {}\n\n\
        Status: Deferred for later resolution\n\n\
        Progress So Far:\n{}\n\n\
        Next Steps When Resuming:\n\
        1. Review collected facts\n\
        2. Check if issue persists\n\
        3. Continue from step {}\n\n\
        =====================",
        context.goal,
        context.facts,
        context.steps + 1
    )
}

fn infer_root_cause(context: &RetrievedContext) -> String {
    // Try to infer root cause from facts
    if !context.facts.is_empty() {
        // Take the most significant fact
        context.facts.lines().next().unwrap_or("Unknown").to_string()
    } else {
        "Issue resolved through systematic investigation".to_string()
    }
}

fn determine_escalation_reason(context: &RetrievedContext) -> String {
    if context.steps > 10 {
        "Investigation exceeded reasonable step limit without resolution"
    } else if context.facts.is_empty() {
        "Unable to extract meaningful diagnostic data"
    } else {
        "Issue requires domain expertise or manual intervention"
    }.to_string()
}

/// Build the default resolution tool handlers
pub fn build_resolution_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::ConcludeSuccess, Arc::new(ConcludeSuccessHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ConcludeFail, Arc::new(ConcludeFailHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Escalate, Arc::new(EscalateHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Defer, Arc::new(DeferHandler) as Arc<dyn ReactToolHandler>),
    ]
}
