use crate::tools::tool_result::ToolOutput;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn usage(&self) -> &str;
    fn examples(&self) -> Vec<&str>;
    fn requires_confirmation(&self) -> bool;
    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError>;
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ToolError {
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
}
