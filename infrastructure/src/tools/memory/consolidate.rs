use crate::memory::{default_memory_path, lifelong::LifelongMemoryStore};
use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct ConsolidateTool;

impl Tool for ConsolidateTool {
    fn name(&self) -> &str {
        "consolidate"
    }

    fn description(&self) -> &str {
        "Store a summary in long-term memory"
    }

    fn usage(&self) -> &str {
        "consolidate <summary>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["consolidate \"Resolved nginx 502 by restarting service\""]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let summary = args.join(" ");
        let store = LifelongMemoryStore::new(default_memory_path())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let id = store
            .remember(&format!("summary: {}", summary))
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolOutput::success(format!("Consolidated memory id {}", id)))
    }
}
