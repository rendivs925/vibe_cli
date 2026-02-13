use crate::tools::common::ensure_args_at_least;
use crate::tools::common::run_bash;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files"
    }

    fn usage(&self) -> &str {
        "grep <pattern> [path]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["grep ReactSession src", "grep TODO ."]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let pattern = args[0];
        let path = args.get(1).copied().unwrap_or(".");
        let cmd = format!(
            "if command -v rg >/dev/null 2>&1; then rg -n --hidden --glob '!.git' -- '{}' '{}'; else grep -RIn -- '{}' '{}'; fi",
            pattern, path, pattern, path
        );
        run_bash(&cmd)
    }
}
