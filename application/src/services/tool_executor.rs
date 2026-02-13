use domain::tools::{Tool, ToolError, ToolOutput};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub examples: Vec<String>,
    pub requires_confirmation: bool,
}

pub struct ToolExecutor {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolExecutor {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn execute(&self, tool_name: &str, args: &[&str]) -> Result<ToolOutput, ToolError> {
        let Some(tool) = self.tools.get(tool_name) else {
            return Err(ToolError::NotFound(format!("tool '{tool_name}'")));
        };

        tool.execute(args)
    }

    pub fn list_tools(&self) -> Vec<ToolInfo> {
        let mut info = self
            .tools
            .values()
            .map(|tool| ToolInfo {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                usage: tool.usage().to_string(),
                examples: tool.examples().into_iter().map(str::to_string).collect(),
                requires_confirmation: tool.requires_confirmation(),
            })
            .collect::<Vec<_>>();

        info.sort_by(|a, b| a.name.cmp(&b.name));
        info
    }

    pub fn get_tool_help(&self, tool_name: &str) -> Option<String> {
        let tool = self.tools.get(tool_name)?;
        let examples = tool.examples();
        let examples_block = if examples.is_empty() {
            String::new()
        } else {
            format!("\nExamples:\n- {}", examples.join("\n- "))
        };

        Some(format!(
            "{name}: {desc}\nUsage: {usage}{examples}",
            name = tool.name(),
            desc = tool.description(),
            usage = tool.usage(),
            examples = examples_block
        ))
    }

    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }
}
