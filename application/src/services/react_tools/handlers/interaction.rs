use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;
use crate::services::react_tools::prompts::interaction as prompts;

/// Handler for ask_clarification tool
pub struct AskClarificationHandler;

#[async_trait]
impl ReactToolHandler for AskClarificationHandler {
    fn name(&self) -> &str {
        "ask_clarification"
    }
    
    fn description(&self) -> &str {
        "Need user clarification"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Interaction
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let question = generate_clarification_question(context);
        
        Ok(ToolResult::new(ReactTool::AskClarification)
            .with_output(question.clone())
            .ask_user(question))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::ask_clarification_prompt(context)
    }
}

/// Handler for ask_confirmation tool
pub struct AskConfirmationHandler;

#[async_trait]
impl ReactToolHandler for AskConfirmationHandler {
    fn name(&self) -> &str {
        "ask_confirmation"
    }
    
    fn description(&self) -> &str {
        "Need user confirmation"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Interaction
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let confirmation_request = generate_confirmation_request(context);
        
        Ok(ToolResult::new(ReactTool::AskConfirmation)
            .with_output(confirmation_request.clone())
            .ask_user(confirmation_request))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::ask_confirmation_prompt(context)
    }
}

/// Handler for explain tool
pub struct ExplainHandler;

#[async_trait]
impl ReactToolHandler for ExplainHandler {
    fn name(&self) -> &str {
        "explain"
    }
    
    fn description(&self) -> &str {
        "Explain reasoning to user"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Interaction
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let explanation = generate_explanation(context);
        
        Ok(ToolResult::new(ReactTool::Explain)
            .with_output(explanation))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::explain_prompt(context)
    }
}

/// Handler for suggest_alternatives tool
pub struct SuggestAlternativesHandler;

#[async_trait]
impl ReactToolHandler for SuggestAlternativesHandler {
    fn name(&self) -> &str {
        "suggest_alternatives"
    }
    
    fn description(&self) -> &str {
        "Offer options to user"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Interaction
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let alternatives = generate_alternatives(context);
        
        Ok(ToolResult::new(ReactTool::SuggestAlternatives)
            .with_output(alternatives)
            .ask_user("Which option would you prefer?".to_string()))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::suggest_alternatives_prompt(context)
    }
}

// Helper functions

fn generate_clarification_question(context: &RetrievedContext) -> String {
    let query_lower = context.goal.to_lowercase();
    
    if query_lower.contains("this") || query_lower.contains("it") {
        "Could you clarify what specifically you're referring to? (e.g., which service, file, or component?)".to_string()
    } else if query_lower.contains("slow") || query_lower.contains("performance") {
        "To help diagnose performance issues, could you tell me:\n1. When did you first notice the problem?\n2. What specific operation is slow?".to_string()
    } else if query_lower.contains("error") || query_lower.contains("fail") {
        "Could you provide more details about the error:\n1. When does it occur?\n2. What is the exact error message?".to_string()
    } else {
        "I need a bit more information to help effectively. Could you clarify:\n1. What specific outcome are you looking for?\n2. Any constraints or requirements I should know about?".to_string()
    }
}

fn generate_confirmation_request(context: &RetrievedContext) -> String {
    format!(
        "Before proceeding, I want to confirm my understanding:\n\n\
        Goal: {}\n\n\
        Proposed approach: Based on current findings, I will continue investigating.\n\n\
        Is this correct? Please confirm (yes/no) or provide corrections.",
        context.goal
    )
}

fn generate_explanation(context: &RetrievedContext) -> String {
    format!(
        "=== Reasoning Explanation ===\n\n\
        Goal: {}\n\n\
        Current Strategy:\n\
        I've been systematically investigating this issue by:\n\
        1. Collecting diagnostic data\n\
        2. Extracting facts from outputs\n\
        3. Formulating hypotheses\n\
        4. Testing theories\n\n\
        Current Findings:\n{}\n\n\
        Next Steps:\n\
        Based on the evidence, I will continue with targeted diagnostics.\n\n\
        =============================",
        context.goal,
        if context.facts.is_empty() {
            "Still gathering initial data..."
        } else {
            &context.facts
        }
    )
}

fn generate_alternatives(context: &RetrievedContext) -> String {
    let query_lower = context.goal.to_lowercase();
    
    if query_lower.contains("fix") || query_lower.contains("repair") {
        "Available options:\n\
         A. Apply automatic fix (may modify system)\n\
         B. Show fix plan for manual application\n\
         C. Continue investigating for alternative solutions\n\n\
         Which would you prefer?".to_string()
    } else if query_lower.contains("check") || query_lower.contains("status") {
        "Available approaches:\n\
         A. Quick check (basic diagnostics only)\n\
         B. Thorough investigation (comprehensive analysis)\n\
         C. Focused check (specific to likely causes)\n\n\
         Which approach should I take?".to_string()
    } else {
        "I can help in several ways:\n\
         A. Fully automated investigation\n\
         B. Step-by-step with confirmation at each step\n\
         C. Analysis only (no changes to system)\n\n\
         Which mode would you prefer?".to_string()
    }
}

/// Build the default interaction tool handlers
pub fn build_interaction_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::AskClarification, Arc::new(AskClarificationHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::AskConfirmation, Arc::new(AskConfirmationHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Explain, Arc::new(ExplainHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::SuggestAlternatives, Arc::new(SuggestAlternativesHandler) as Arc<dyn ReactToolHandler>),
    ]
}
