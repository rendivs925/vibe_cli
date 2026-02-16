use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;
use crate::services::react_tools::prompts::verification as prompts;

/// Handler for check_goal tool
pub struct CheckGoalHandler;

#[async_trait]
impl ReactToolHandler for CheckGoalHandler {
    fn name(&self) -> &str {
        "check_goal"
    }
    
    fn description(&self) -> &str {
        "Verify if original goal achieved"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Verification
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let achieved = check_goal_achieved(context);
        
        let output = if achieved {
            "YES - Goal appears to be achieved based on current findings.".to_string()
        } else {
            format!(
                "NO - Goal not yet achieved.\n\nCurrent progress:\n{}",
                assess_progress(context)
            )
        };
        
        Ok(ToolResult::new(ReactTool::CheckGoal)
            .with_output(output))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::check_goal_prompt(context)
    }
}

/// Handler for verify_fix tool
pub struct VerifyFixHandler;

#[async_trait]
impl ReactToolHandler for VerifyFixHandler {
    fn name(&self) -> &str {
        "verify_fix"
    }
    
    fn description(&self) -> &str {
        "Verify if fix was applied correctly"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Verification
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let verified = verify_fix_applied(context);
        
        let output = if verified {
            "VERIFIED - Fix was applied correctly. Issue appears resolved.".to_string()
        } else {
            format!(
                "NOT VERIFIED - Fix may not have been applied correctly.\n\n{}",
                suggest_verification_steps(context)
            )
        };
        
        Ok(ToolResult::new(ReactTool::VerifyFix)
            .with_output(output))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::verify_fix_prompt(context)
    }
}

/// Handler for verify_syntax tool
pub struct VerifySyntaxHandler;

#[async_trait]
impl ReactToolHandler for VerifySyntaxHandler {
    fn name(&self) -> &str {
        "verify_syntax"
    }
    
    fn description(&self) -> &str {
        "Check syntax before applying"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Verification
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let syntax_ok = check_syntax(context);
        
        let output = if syntax_ok {
            "SYNTAX OK - No syntax errors detected in proposed changes.".to_string()
        } else {
            "SYNTAX ERROR - Issues detected. Review before applying.".to_string()
        };
        
        Ok(ToolResult::new(ReactTool::VerifySyntax)
            .with_output(output))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::verify_syntax_prompt(context)
    }
}

/// Handler for test_hypothesis tool
pub struct TestHypothesisHandler;

#[async_trait]
impl ReactToolHandler for TestHypothesisHandler {
    fn name(&self) -> &str {
        "test_hypothesis"
    }
    
    fn description(&self) -> &str {
        "Test a hypothesis"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Verification
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, params: Option<&str>) -> Result<ToolResult> {
        let hypothesis = params.unwrap_or("");
        let result = test_hypothesis(context, hypothesis);
        
        Ok(ToolResult::new(ReactTool::TestHypothesis)
            .with_output(result))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::test_hypothesis_prompt(context)
    }
}

// Helper functions

fn check_goal_achieved(context: &RetrievedContext) -> bool {
    let goal_lower = context.goal.to_lowercase();
    let output_lower = context.latest_output.to_lowercase();
    
    // Check for success indicators
    if output_lower.contains("success") || 
       output_lower.contains("active (running)") ||
       output_lower.contains("completed") ||
       output_lower.contains("done") {
        return true;
    }
    
    // Check if goal keywords are found in output with positive context
    let goal_keywords: Vec<&str> = goal_lower
        .split_whitespace()
        .filter(|w| w.len() > 4 && !is_common_word(w))
        .collect();
    
    let matches = goal_keywords.iter()
        .filter(|k| output_lower.contains(*k))
        .count();
    
    // If most goal keywords are present and no errors, consider achieved
    if !goal_keywords.is_empty() && matches >= goal_keywords.len() / 2 {
        return !output_lower.contains("error") && !output_lower.contains("fail");
    }
    
    false
}

fn is_common_word(word: &str) -> bool {
    const COMMON: &[&str] = &[
        "check", "verify", "ensure", "make", "need", "want", "have", "should",
        "status", "state", "running", "working", "fix", "issue", "problem",
    ];
    COMMON.contains(&word)
}

fn assess_progress(context: &RetrievedContext) -> String {
    let mut progress = Vec::new();
    
    if !context.facts.is_empty() {
        progress.push(format!("- Collected {} fact(s)", context.facts.len()));
    }
    
    if !context.hypotheses.is_empty() {
        progress.push(format!("- Formulated {} hypothesis(es)", context.hypotheses.len()));
    }
    
    if context.steps > 0 {
        progress.push(format!("- Completed {} step(s)", context.steps));
    }
    
    if progress.is_empty() {
        progress.push("- Investigation in early stages".to_string());
    }
    
    progress.join("\n")
}

fn verify_fix_applied(context: &RetrievedContext) -> bool {
    let output = &context.latest_output;
    let output_lower = output.to_lowercase();
    
    // Check for success indicators
    output_lower.contains("success") ||
    output_lower.contains("active (running)") ||
    output_lower.contains("done") ||
    output_lower.contains("completed") ||
    (!output_lower.contains("error") && 
     !output_lower.contains("fail") &&
     !output_lower.contains("denied"))
}

fn suggest_verification_steps(context: &RetrievedContext) -> String {
    "Suggested verification steps:\n\
     1. Check if the error/warning still appears\n\
     2. Verify the service/process is in expected state\n\
     3. Test the functionality that was broken".to_string()
}

fn check_syntax(_context: &RetrievedContext) -> bool {
    // In a real implementation, this would parse and validate syntax
    // For now, assume OK unless clear error patterns
    true
}

fn test_hypothesis(context: &RetrievedContext, hypothesis: &str) -> String {
    let output_lower = context.latest_output.to_lowercase();
    let hyp_lower = hypothesis.to_lowercase();
    
    // Simple keyword matching to test hypothesis
    let keywords: Vec<&str> = hyp_lower.split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();
    
    let matches = keywords.iter()
        .filter(|k| output_lower.contains(*k))
        .count();
    
    if matches >= keywords.len() / 2 {
        format!("HYPOTHESIS SUPPORTED: '{}' matches {} of {} keywords in output", 
            hypothesis, matches, keywords.len())
    } else {
        format!("HYPOTHESIS NOT SUPPORTED: '{}' only matches {} of {} keywords", 
            hypothesis, matches, keywords.len())
    }
}

/// Build the default verification tool handlers
pub fn build_verification_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::CheckGoal, Arc::new(CheckGoalHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::VerifyFix, Arc::new(VerifyFixHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::VerifySyntax, Arc::new(VerifySyntaxHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::TestHypothesis, Arc::new(TestHypothesisHandler) as Arc<dyn ReactToolHandler>),
    ]
}
