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

pub struct GrepContextHandler;

#[async_trait]
impl ReactToolHandler for GrepContextHandler {
    fn name(&self) -> &str {
        "grep_context"
    }

    fn description(&self) -> &str {
        "Grep with surrounding context"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let pattern = infer_pattern(context);
        Ok(ToolResult::new(ReactTool::GrepContext)
            .with_commands(vec![format!("grep_context \"{}\"", pattern)])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Search for the most relevant pattern with surrounding context.".to_string()
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
        (ReactTool::GrepContext, Arc::new(GrepContextHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::FindPatterns, Arc::new(FindPatternsHandler) as Arc<dyn ReactToolHandler>),
    ]
}

fn infer_pattern(context: &RetrievedContext) -> String {
    let words: Vec<&str> = context
        .goal
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();
    if words.is_empty() {
        "TODO|FIXME|error".to_string()
    } else {
        words.iter().take(3).cloned().collect::<Vec<_>>().join("|")
    }
}
