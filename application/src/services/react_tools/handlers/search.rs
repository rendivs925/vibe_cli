use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;

pub struct SemanticSearchHandler;

#[async_trait]
impl ReactToolHandler for SemanticSearchHandler {
    fn name(&self) -> &str {
        "semantic_search"
    }

    fn description(&self) -> &str {
        "Semantic search across past sessions"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let query = context.goal.replace('"', "\\\"");
        Ok(ToolResult::new(ReactTool::SemanticSearch)
            .with_commands(vec![format!("semantic_search \"{}\"", query)])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Search across past sessions for similar context.".to_string()
    }
}

pub struct FindPatternsHandler;

#[async_trait]
impl ReactToolHandler for FindPatternsHandler {
    fn name(&self) -> &str {
        "find_patterns"
    }

    fn description(&self) -> &str {
        "Find learned patterns from memory"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let query = context.goal.replace('"', "\\\"");
        Ok(ToolResult::new(ReactTool::FindPatterns)
            .with_commands(vec![format!("find_patterns \"{}\"", query)])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Search for learned patterns relevant to the current goal.".to_string()
    }
}

pub fn build_search_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::SemanticSearch, Arc::new(SemanticSearchHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::FindPatterns, Arc::new(FindPatternsHandler) as Arc<dyn ReactToolHandler>),
    ]
}
