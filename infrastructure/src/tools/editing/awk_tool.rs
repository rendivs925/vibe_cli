use crate::tools::common::{ensure_args_at_least, run_process};
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct AwkTool;

impl Tool for AwkTool {
    fn name(&self) -> &str {
        "awk"
    }

    fn description(&self) -> &str {
        "Run awk script against a file"
    }

    fn usage(&self) -> &str {
        "awk <script> <path>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["awk '{print $1}' Cargo.toml"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 2, self.usage())?;
        run_process("awk", &[args[0], args[1]])
    }
}
