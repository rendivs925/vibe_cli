use crate::tools::common::ensure_args_at_least;
use crate::tools::common::run_bash;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct FdTool;

impl Tool for FdTool {
    fn name(&self) -> &str {
        "fd"
    }

    fn description(&self) -> &str {
        "Find files by name pattern"
    }

    fn usage(&self) -> &str {
        "fd <pattern> [directory]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["fd '*.rs' src", "fd Cargo ."]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let pattern = args[0];
        let directory = args.get(1).copied().unwrap_or(".");
        let cmd = format!(
            "if command -v fd >/dev/null 2>&1; then fd --hidden --exclude .git '{}' '{}'; else find '{}' -iname '*{}*'; fi",
            pattern, directory, directory, pattern
        );
        run_bash(&cmd)
    }
}
