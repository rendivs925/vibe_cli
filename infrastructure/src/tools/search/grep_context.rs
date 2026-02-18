use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};
use std::process::Command;

pub struct GrepContextTool;

impl Tool for GrepContextTool {
    fn name(&self) -> &str {
        "grep_context"
    }

    fn description(&self) -> &str {
        "Grep with surrounding context"
    }

    fn usage(&self) -> &str {
        "grep_context <pattern> [path] [context_lines]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["grep_context TODO", "grep_context Error src 3"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let pattern = args[0];
        let path = args.get(1).copied().unwrap_or(".");
        let context_lines = args
            .get(2)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(2);

        let output = Command::new("rg")
            .args([
                "-n",
                "-C",
                &context_lines.to_string(),
                pattern,
                path,
            ])
            .output()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && stdout.trim().is_empty() {
            return Err(ToolError::ExecutionFailed(stderr));
        }

        Ok(ToolOutput {
            success: output.status.success(),
            stdout,
            stderr,
            exit_code: code,
            format: domain::tools::OutputFormat::Text,
            metadata: Default::default(),
        })
    }
}
