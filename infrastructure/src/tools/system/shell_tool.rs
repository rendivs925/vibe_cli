use crate::tools::common::{ensure_args_at_least, run_bash};
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct ShellTool;

impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command"
    }

    fn usage(&self) -> &str {
        "shell <command>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["shell ls -la", "shell systemctl status nginx"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        run_bash(&args.join(" "))
    }
}
