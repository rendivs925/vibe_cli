use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;

pub struct RememberHandler;

#[async_trait]
impl ReactToolHandler for RememberHandler {
    fn name(&self) -> &str {
        "remember"
    }

    fn description(&self) -> &str {
        "Store a fact in lifelong memory"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let fact = default_fact(context);
        Ok(ToolResult::new(ReactTool::Remember)
            .with_commands(vec![format!("remember \"{}\"", fact)])
            .with_next_tool(ReactTool::ShowFacts))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Store a key fact in lifelong memory.".to_string()
    }
}

pub struct RecallHandler;

#[async_trait]
impl ReactToolHandler for RecallHandler {
    fn name(&self) -> &str {
        "recall"
    }

    fn description(&self) -> &str {
        "Retrieve from memory"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let query = context.goal.replace('"', "\\\"");
        Ok(ToolResult::new(ReactTool::Recall)
            .with_commands(vec![format!("recall \"{}\"", query)])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Recall relevant information from memory.".to_string()
    }
}

pub struct ConsolidateHandler;

#[async_trait]
impl ReactToolHandler for ConsolidateHandler {
    fn name(&self) -> &str {
        "consolidate"
    }

    fn description(&self) -> &str {
        "Summarize to long-term memory"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let summary = context.session_history.replace('"', "\\\"");
        Ok(ToolResult::new(ReactTool::Consolidate)
            .with_commands(vec![format!("consolidate \"{}\"", summary)])
            .with_next_tool(ReactTool::ShowHistory))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Consolidate this session into long-term memory.".to_string()
    }
}

pub struct SearchMemoryHandler;

#[async_trait]
impl ReactToolHandler for SearchMemoryHandler {
    fn name(&self) -> &str {
        "search_memory"
    }

    fn description(&self) -> &str {
        "Search lifelong memory"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let query = context.goal.replace('"', "\\\"");
        Ok(ToolResult::new(ReactTool::SearchMemory)
            .with_commands(vec![format!("search_memory \"{}\"", query)])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Search lifelong memory for related information.".to_string()
    }
}

pub struct LearnPatternsHandler;

#[async_trait]
impl ReactToolHandler for LearnPatternsHandler {
    fn name(&self) -> &str {
        "learn_patterns"
    }

    fn description(&self) -> &str {
        "Extract reusable patterns"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let pattern = context.goal.replace('"', "\\\"");
        Ok(ToolResult::new(ReactTool::LearnPatterns)
            .with_commands(vec![format!("learn_patterns \"{}\"", pattern)])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Capture a reusable pattern from this session.".to_string()
    }
}

pub fn build_memory_extended_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::Remember, Arc::new(RememberHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Recall, Arc::new(RecallHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Consolidate, Arc::new(ConsolidateHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::SearchMemory, Arc::new(SearchMemoryHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::LearnPatterns, Arc::new(LearnPatternsHandler) as Arc<dyn ReactToolHandler>),
    ]
}

fn default_fact(context: &RetrievedContext) -> String {
    if let Some(fact) = context.facts_list.first() {
        return format!("{}={}", fact.key, fact.value);
    }
    if !context.latest_output.trim().is_empty() {
        return context.latest_output.lines().next().unwrap_or("fact").to_string();
    }
    context.goal.clone()
}
