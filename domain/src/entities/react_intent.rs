use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryIntent {
    pub task_type: TaskType,
    pub target: Option<String>,
    pub constraints: Vec<String>,
    pub tool_categories: Vec<ToolCategory>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Debug,
    Explore,
    Fix,
    Explain,
    Monitor,
    Configure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    Process,
    Network,
    Filesystem,
    Service,
    Logs,
    Package,
    Git,
    Build,
    Shell,
}

impl QueryIntent {
    pub fn new(
        task_type: TaskType,
        target: Option<String>,
        constraints: Vec<String>,
        tool_categories: Vec<ToolCategory>,
        confidence: f32,
    ) -> Self {
        Self {
            task_type,
            target,
            constraints,
            tool_categories,
            confidence,
        }
    }
}
