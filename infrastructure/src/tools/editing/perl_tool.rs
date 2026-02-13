use crate::tools::common::{ensure_args_at_least, run_process};
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct PerlTool;

impl Tool for PerlTool {
    fn name(&self) -> &str {
        "perl"
    }

    fn description(&self) -> &str {
        "Apply perl regex replacement in place"
    }

    fn usage(&self) -> &str {
        "perl <regex> <replacement> <path>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["perl foo\\s+bar baz src/main.rs"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 3, self.usage())?;
        let expr = format!("s/{}/{}/g", args[0], args[1]);
        run_process("perl", &["-pi", "-e", &expr, args[2]])
    }
}
