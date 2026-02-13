use crate::tools::common::{ensure_args_at_least, run_process};
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct SedTool;

impl Tool for SedTool {
    fn name(&self) -> &str {
        "sed"
    }

    fn description(&self) -> &str {
        "Apply sed replacement in place"
    }

    fn usage(&self) -> &str {
        "sed <pattern> <replacement> <path>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["sed old new src/main.rs"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 3, self.usage())?;
        let expr = format!("s/{}/{}/g", args[0], args[1]);
        run_process("sed", &["-i", &expr, args[2]])
    }
}
