use anyhow::Result;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::collections::HashMap;
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::handlers;

/// Trait that all ReAct tool handlers must implement
#[async_trait::async_trait]
pub trait ReactToolHandler: Send + Sync {
    /// Returns the tool name
    fn name(&self) -> &str;
    
    /// Returns the tool description
    fn description(&self) -> &str;
    
    /// Returns the tool category
    fn category(&self) -> ToolCategory;
    
    /// Whether this tool requires the latest output to function
    fn requires_output(&self) -> bool;
    
    /// Execute the tool with the given context
    async fn execute(&self, context: &RetrievedContext, params: Option<&str>) -> Result<ToolResult>;
    
    /// Get the prompt template for this tool
    fn get_prompt(&self, context: &RetrievedContext) -> String;
}

/// Registry for all ReAct tools
pub struct ToolRegistry {
    tools: HashMap<ReactTool, Arc<dyn ReactToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
    
    /// Register a tool handler
    pub fn register(&mut self, tool: ReactTool, handler: Arc<dyn ReactToolHandler>) {
        self.tools.insert(tool, handler);
    }
    
    /// Get a tool handler by ReactTool enum
    pub fn get(&self, tool: ReactTool) -> Option<Arc<dyn ReactToolHandler>> {
        self.tools.get(&tool).cloned()
    }
    
    /// Get a tool handler by name
    pub fn get_by_name(&self, name: &str) -> Option<Arc<dyn ReactToolHandler>> {
        name.parse::<ReactTool>()
            .ok()
            .and_then(|tool| self.tools.get(&tool).cloned())
    }
    
    /// Get all registered tools
    pub fn get_all_tools(&self) -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
        self.tools.iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }
    
    /// Get tools by category
    pub fn get_by_category(&self, category: ToolCategory) -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
        self.tools.iter()
            .filter(|(tool, _)| tool.category() == category)
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }
    
    /// Check if a tool is registered
    pub fn has_tool(&self, tool: ReactTool) -> bool {
        self.tools.contains_key(&tool)
    }
    
    /// Get the number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }
    
    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Build a registry with all default tool handlers
    pub fn with_default_handlers() -> Self {
        let mut registry = Self::new();
        
        // Register investigation tools
        for (tool, handler) in handlers::build_investigation_handlers() {
            registry.register(tool, handler);
        }
        
        // Register analysis tools
        for (tool, handler) in handlers::build_analysis_handlers() {
            registry.register(tool, handler);
        }
        
        // Register planning tools
        for (tool, handler) in handlers::build_planning_handlers() {
            registry.register(tool, handler);
        }
        
        // Register action tools
        for (tool, handler) in handlers::build_action_handlers() {
            registry.register(tool, handler);
        }
        
        // Register verification tools
        for (tool, handler) in handlers::build_verification_handlers() {
            registry.register(tool, handler);
        }
        
        // Register memory tools
        for (tool, handler) in handlers::build_memory_handlers() {
            registry.register(tool, handler);
        }
        
        // Register resolution tools
        for (tool, handler) in handlers::build_resolution_handlers() {
            registry.register(tool, handler);
        }
        
        // Register interaction tools
        for (tool, handler) in handlers::build_interaction_handlers() {
            registry.register(tool, handler);
        }

        // Register code tools
        for (tool, handler) in handlers::build_code_handlers() {
            registry.register(tool, handler);
        }

        // Register web tools
        for (tool, handler) in handlers::build_web_handlers() {
            registry.register(tool, handler);
        }

        // Register document tools
        for (tool, handler) in handlers::build_document_handlers() {
            registry.register(tool, handler);
        }

        // Register search tools
        for (tool, handler) in handlers::build_search_handlers() {
            registry.register(tool, handler);
        }

        // Register extended memory tools
        for (tool, handler) in handlers::build_memory_extended_handlers() {
            registry.register(tool, handler);
        }
        
        registry
    }
}

/// Configuration for ReAct tool behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    /// Always use suggest_command (legacy behavior)
    Legacy,
    /// Use tool selection with fallback to suggest_command
    Mixed,
    /// Full dynamic tool system
    Full,
}

impl ToolMode {
    pub fn name(&self) -> &'static str {
        match self {
            ToolMode::Legacy => "legacy",
            ToolMode::Mixed => "mixed",
            ToolMode::Full => "full",
        }
    }
}

impl std::str::FromStr for ToolMode {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "legacy" => Ok(ToolMode::Legacy),
            "mixed" => Ok(ToolMode::Mixed),
            "full" => Ok(ToolMode::Full),
            _ => Err(format!("Unknown tool mode: {}", s)),
        }
    }
}

/// Configuration for ReAct tool system
#[derive(Debug, Clone)]
pub struct ReactConfig {
    /// Current tool mode
    pub tool_mode: ToolMode,
    /// Default tool when mode is Legacy or as fallback
    pub default_tool: ReactTool,
    /// Maximum iterations for tool execution
    pub max_iterations: u32,
    /// Whether to show tool selection reasoning
    pub show_tool_reasoning: bool,
}

impl ReactConfig {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_mode(mut self, mode: ToolMode) -> Self {
        self.tool_mode = mode;
        self
    }
    
    pub fn with_default_tool(mut self, tool: ReactTool) -> Self {
        self.default_tool = tool;
        self
    }
    
    pub fn with_max_iterations(mut self, iterations: u32) -> Self {
        self.max_iterations = iterations;
        self
    }
    
    pub fn with_show_reasoning(mut self, show: bool) -> Self {
        self.show_tool_reasoning = show;
        self
    }
}

impl Default for ReactConfig {
    fn default() -> Self {
        Self {
            tool_mode: ToolMode::Full,
            default_tool: ReactTool::SuggestCommand,
            max_iterations: 10,
            show_tool_reasoning: true,
        }
    }
}
