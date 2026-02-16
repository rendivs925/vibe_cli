use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;
use crate::services::react_tools::prompts::action as prompts;

/// Handler for apply_fix tool
pub struct ApplyFixHandler;

#[async_trait]
impl ReactToolHandler for ApplyFixHandler {
    fn name(&self) -> &str {
        "apply_fix"
    }
    
    fn description(&self) -> &str {
        "Apply a fix or change"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Action
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let fix_plan = generate_fix_plan(context);
        
        Ok(ToolResult::new(ReactTool::ApplyFix)
            .with_output(format!("Fix plan generated:\n{}", fix_plan)))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::apply_fix_prompt(context)
    }
}

/// Handler for edit_file tool
pub struct EditFileHandler;

#[async_trait]
impl ReactToolHandler for EditFileHandler {
    fn name(&self) -> &str {
        "edit_file"
    }
    
    fn description(&self) -> &str {
        "Edit an existing file"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Action
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, params: Option<&str>) -> Result<ToolResult> {
        let file_path = params.unwrap_or("");
        let suggested_path = if file_path.is_empty() {
            infer_file_to_edit(context)
        } else {
            file_path.to_string()
        };
        
        let command = format!("update {} <old> <new>", suggested_path);
        
        Ok(ToolResult::new(ReactTool::EditFile)
            .with_output(format!("Suggested file to edit: {}", suggested_path))
            .with_commands(vec![command]))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::edit_file_prompt(context)
    }
}

/// Handler for create_file tool
pub struct CreateFileHandler;

#[async_trait]
impl ReactToolHandler for CreateFileHandler {
    fn name(&self) -> &str {
        "create_file"
    }
    
    fn description(&self) -> &str {
        "Create a new file"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Action
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, params: Option<&str>) -> Result<ToolResult> {
        let file_path = params.unwrap_or("");
        let suggested_path = if file_path.is_empty() {
            infer_new_file_path(context)
        } else {
            file_path.to_string()
        };
        
        let command = format!("write {} <content>", suggested_path);
        
        Ok(ToolResult::new(ReactTool::CreateFile)
            .with_output(format!("Suggested new file: {}", suggested_path))
            .with_commands(vec![command]))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::create_file_prompt(context)
    }
}

/// Handler for run_command tool
pub struct RunCommandHandler;

#[async_trait]
impl ReactToolHandler for RunCommandHandler {
    fn name(&self) -> &str {
        "run_command"
    }
    
    fn description(&self) -> &str {
        "Run a command directly"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Action
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, params: Option<&str>) -> Result<ToolResult> {
        // This tool signals that a command should be run without suggestion phase
        let command = params.unwrap_or("").to_string();
        
        if command.is_empty() {
            Ok(ToolResult::new(ReactTool::RunCommand)
                .with_output("No command specified. Use suggest_command to propose commands.".to_string()))
        } else {
            Ok(ToolResult::new(ReactTool::RunCommand)
                .with_output(format!("Direct command execution: {}", command))
                .with_commands(vec![command]))
        }
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::run_command_prompt(context)
    }
}

/// Handler for retry tool
pub struct RetryHandler;

#[async_trait]
impl ReactToolHandler for RetryHandler {
    fn name(&self) -> &str {
        "retry"
    }
    
    fn description(&self) -> &str {
        "Retry failed operation"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Action
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        // Find the last failed command from history
        let last_failed = find_last_failed_command(context);
        
        let (output, commands) = if let Some(cmd) = last_failed {
            (format!("Retrying last failed command: {}", cmd), vec![cmd])
        } else {
            ("No failed command found to retry.".to_string(), vec![])
        };
        
        Ok(ToolResult::new(ReactTool::Retry)
            .with_output(output)
            .with_commands(commands))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::retry_prompt(context)
    }
}

// Helper functions

fn generate_fix_plan(context: &RetrievedContext) -> String {
    let query_lower = context.goal.to_lowercase();
    
    if query_lower.contains("permission") {
        "Fix plan:\n  1. Check current permissions\n  2. Apply correct permissions with chmod/chown\n  3. Verify fix".to_string()
    } else if query_lower.contains("service") && (query_lower.contains("stop") || query_lower.contains("not running")) {
        "Fix plan:\n  1. Check service status\n  2. Start service with systemctl start <service>\n  3. Enable auto-start if needed".to_string()
    } else if query_lower.contains("disk full") || query_lower.contains("no space") {
        "Fix plan:\n  1. Identify large files/directories\n  2. Clean up unnecessary files\n  3. Consider log rotation\n  4. Monitor space after cleanup".to_string()
    } else {
        "Fix plan:\n  1. Identify root cause from current findings\n  2. Apply targeted fix\n  3. Verify the fix resolves the issue".to_string()
    }
}

fn infer_file_to_edit(context: &RetrievedContext) -> String {
    // Try to extract file path from context
    let output = &context.latest_output;
    
    // Look for file paths in output
    for line in output.lines() {
        if line.contains("/") && (line.contains(".toml") || line.contains(".conf") || line.contains(".json")) {
            // Extract what looks like a file path
            let words: Vec<&str> = line.split_whitespace().collect();
            for word in words {
                if word.contains('/') && word.contains('.') {
                    return word.to_string();
                }
            }
        }
    }
    
    // Default based on context
    if output.contains("Cargo") {
        "Cargo.toml".to_string()
    } else if output.contains("config") {
        "config.toml".to_string()
    } else {
        "<file_path>".to_string()
    }
}

fn infer_new_file_path(context: &RetrievedContext) -> String {
    let query_lower = context.goal.to_lowercase();
    
    if query_lower.contains("script") {
        "./script.sh".to_string()
    } else if query_lower.contains("config") {
        "./config.toml".to_string()
    } else if query_lower.contains("readme") || query_lower.contains("documentation") {
        "./README.md".to_string()
    } else {
        "./new_file.txt".to_string()
    }
}

fn find_last_failed_command(context: &RetrievedContext) -> Option<String> {
    // Parse session history to find last failed command
    // Look for patterns like "exit code: 1" or "Failed"
    let history = &context.session_history;
    
    // Simple heuristic: look for lines with "exit" and non-zero codes
    for line in history.lines().rev() {
        if line.contains("exit") && (line.contains("1") || line.contains("error")) {
            // Try to extract command from previous lines
            return Some("<previous_command>".to_string());
        }
    }
    
    None
}

/// Build the default action tool handlers
pub fn build_action_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::ApplyFix, Arc::new(ApplyFixHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::EditFile, Arc::new(EditFileHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::CreateFile, Arc::new(CreateFileHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::RunCommand, Arc::new(RunCommandHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Retry, Arc::new(RetryHandler) as Arc<dyn ReactToolHandler>),
    ]
}
