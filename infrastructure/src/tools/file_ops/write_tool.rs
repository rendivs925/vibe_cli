use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};
use std::fs;

pub struct WriteTool;

impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file"
    }

    fn usage(&self) -> &str {
        "write <path> <content>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["write notes.txt hello"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 2, self.usage())?;
        let path = args[0];
        let content = args[1..].join(" ");
        fs::write(path, content).map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        Ok(ToolOutput::success(format!("Wrote {path}")))
    }
}
