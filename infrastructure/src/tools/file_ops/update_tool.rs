use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};
use std::fs;

pub struct UpdateTool;

impl Tool for UpdateTool {
    fn name(&self) -> &str {
        "update"
    }

    fn description(&self) -> &str {
        "Replace text in a file"
    }

    fn usage(&self) -> &str {
        "update <path> <old> <new>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["update src/main.rs foo bar"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 3, self.usage())?;
        let path = args[0];
        let old = args[1];
        let new = args[2];
        let content =
            fs::read_to_string(path).map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        if !content.contains(old) {
            return Err(ToolError::NotFound(format!(
                "pattern '{old}' in file '{path}'"
            )));
        }

        let updated = content.replace(old, new);
        fs::write(path, updated).map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        Ok(ToolOutput::success(format!("Updated {path}")))
    }
}
