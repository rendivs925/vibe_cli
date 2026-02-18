use crate::memory::{default_memory_path, lifelong::LifelongMemoryStore};
use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct RememberTool;

impl Tool for RememberTool {
    fn name(&self) -> &str {
        "remember"
    }

    fn description(&self) -> &str {
        "Store a fact in lifelong memory"
    }

    fn usage(&self) -> &str {
        "remember <fact>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["remember \"nginx config in /etc/nginx/nginx.conf\""]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let fact = args.join(" ");
        let store = LifelongMemoryStore::new(default_memory_path())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let id = store
            .remember(&fact)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolOutput::success(format!("Stored memory id {}", id)))
    }
}
