use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;
use crate::services::react_tools::prompts::investigation as prompts;

/// Handler for suggest_command tool
pub struct SuggestCommandHandler;

#[async_trait]
impl ReactToolHandler for SuggestCommandHandler {
    fn name(&self) -> &str {
        "suggest_command"
    }
    
    fn description(&self) -> &str {
        "Propose diagnostic command to run"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, _context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        // This tool doesn't execute directly - it signals that commands should be generated
        // The actual command generation happens in ReactAgentService
        Ok(ToolResult::new(ReactTool::SuggestCommand)
            .with_output("Requesting command suggestions based on current context...".to_string())
            .with_next_tool(ReactTool::Summarize))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::suggest_command_prompt(context)
    }
}

/// Handler for suggest_read tool
pub struct SuggestReadHandler;

#[async_trait]
impl ReactToolHandler for SuggestReadHandler {
    fn name(&self) -> &str {
        "suggest_read"
    }
    
    fn description(&self) -> &str {
        "Propose file to read"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        // This tool suggests a file path to read
        // Returns the file path as a command
        let suggested_path = infer_file_to_read(context);
        let command = format!("read {}", suggested_path);
        
        Ok(ToolResult::new(ReactTool::SuggestRead)
            .with_output(format!("Suggested file to examine: {}", suggested_path))
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::suggest_read_prompt(context)
    }
}

/// Handler for suggest_grep tool
pub struct SuggestGrepHandler;

#[async_trait]
impl ReactToolHandler for SuggestGrepHandler {
    fn name(&self) -> &str {
        "suggest_grep"
    }
    
    fn description(&self) -> &str {
        "Propose search pattern"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        // Suggests a grep pattern to search
        let pattern = infer_grep_pattern(context);
        let command = format!("grep '{}'", pattern);
        
        Ok(ToolResult::new(ReactTool::SuggestGrep)
            .with_output(format!("Suggested search pattern: {}", pattern))
            .with_commands(vec![command])
            .with_next_tool(ReactTool::ExtractErrors))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::suggest_grep_prompt(context)
    }
}

/// Handler for suggest_rag tool
pub struct SuggestRagHandler;

#[async_trait]
impl ReactToolHandler for SuggestRagHandler {
    fn name(&self) -> &str {
        "suggest_rag"
    }
    
    fn description(&self) -> &str {
        "Propose RAG query for code context"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        // Suggests a RAG query for codebase exploration
        let query = infer_rag_query(context);
        let command = format!("rag \"{}\" 10", query);
        
        Ok(ToolResult::new(ReactTool::SuggestRag)
            .with_output(format!("Suggested RAG query: {}", query))
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::suggest_rag_prompt(context)
    }
}

/// Handler for suggest_discovery tool
pub struct SuggestDiscoveryHandler;

#[async_trait]
impl ReactToolHandler for SuggestDiscoveryHandler {
    fn name(&self) -> &str {
        "suggest_discovery"
    }
    
    fn description(&self) -> &str {
        "Propose system discovery command"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }
    
    fn requires_output(&self) -> bool {
        false
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        // Suggests system discovery commands
        let commands = infer_discovery_commands(context);
        
        Ok(ToolResult::new(ReactTool::SuggestDiscovery)
            .with_output(format!("Suggested {} discovery command(s)", commands.len()))
            .with_commands(commands)
            .with_next_tool(ReactTool::ExtractMetrics))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::suggest_discovery_prompt(context)
    }
}

// Helper functions for inference

fn infer_file_to_read(context: &RetrievedContext) -> String {
    // Try to infer which file to read based on context
    let query_lower = context.goal.to_lowercase();
    
    // Common config files
    if query_lower.contains("config") || query_lower.contains("configuration") {
        return "config.toml".to_string();
    }
    
    if query_lower.contains("cargo") || query_lower.contains("rust") {
        return "Cargo.toml".to_string();
    }
    
    if query_lower.contains("package") || query_lower.contains("dependency") {
        if std::path::Path::new("Cargo.toml").exists() {
            return "Cargo.toml".to_string();
        }
        if std::path::Path::new("package.json").exists() {
            return "package.json".to_string();
        }
    }
    
    if query_lower.contains("readme") || query_lower.contains("documentation") {
        return "README.md".to_string();
    }
    
    // Default to a common file
    "README.md".to_string()
}

fn infer_grep_pattern(context: &RetrievedContext) -> String {
    // Extract likely search patterns from the goal
    let query = &context.goal;
    
    // Extract key terms (simplified)
    let words: Vec<&str> = query
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .filter(|w| !is_common_word(w))
        .collect();
    
    if words.is_empty() {
        return "TODO|FIXME|error|Error".to_string();
    }
    
    // Join top words with OR
    words.iter().take(3).cloned().collect::<Vec<_>>().join("|")
}

fn is_common_word(word: &str) -> bool {
    const COMMON: &[&str] = &[
        "what", "where", "when", "why", "how", "which", "this", "that", "with",
        "from", "have", "been", "were", "they", "them", "their", "there",
        "about", "would", "could", "should", "find", "look", "show", "tell",
    ];
    COMMON.contains(&word.to_lowercase().as_str())
}

fn infer_rag_query(context: &RetrievedContext) -> String {
    // Use the goal as the RAG query, with some cleanup
    context.goal.clone()
}

fn infer_discovery_commands(context: &RetrievedContext) -> Vec<String> {
    let query_lower = context.goal.to_lowercase();
    let mut commands = Vec::new();
    
    if query_lower.contains("process") || query_lower.contains("running") {
        commands.push("shell ps aux".to_string());
    }
    
    if query_lower.contains("disk") || query_lower.contains("space") || query_lower.contains("full") {
        commands.push("shell df -h".to_string());
    }
    
    if query_lower.contains("memory") || query_lower.contains("ram") {
        commands.push("shell free -h".to_string());
    }
    
    if query_lower.contains("service") || query_lower.contains("systemd") {
        commands.push("shell systemctl list-units --type=service --state=running".to_string());
    }
    
    if query_lower.contains("network") || query_lower.contains("port") || query_lower.contains("connection") {
        commands.push("shell ss -tlnp".to_string());
    }
    
    if commands.is_empty() {
        // Default discovery commands
        commands.push("shell ls -la".to_string());
        commands.push("shell pwd".to_string());
    }
    
    commands
}

/// Build the default investigation tool handlers
pub fn build_investigation_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::SuggestCommand, Arc::new(SuggestCommandHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::SuggestRead, Arc::new(SuggestReadHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::SuggestGrep, Arc::new(SuggestGrepHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::SuggestRag, Arc::new(SuggestRagHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::SuggestDiscovery, Arc::new(SuggestDiscoveryHandler) as Arc<dyn ReactToolHandler>),
    ]
}
