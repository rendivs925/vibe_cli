use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};
use std::fs;
use std::path::Path;

pub struct RemoveTool;

impl Tool for RemoveTool {
    fn name(&self) -> &str {
        "remove"
    }

    fn description(&self) -> &str {
        "Delete a file or directory"
    }

    fn usage(&self) -> &str {
        "remove <path>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["remove /tmp/stale.log"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let path = Path::new(args[0]);
        if !path.exists() {
            return Err(ToolError::NotFound(args[0].to_string()));
        }
        if path.is_dir() {
            fs::remove_dir_all(path).map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        } else {
            fs::remove_file(path).map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        }

        Ok(ToolOutput::success(format!("Removed {}", args[0])))
    }
}
