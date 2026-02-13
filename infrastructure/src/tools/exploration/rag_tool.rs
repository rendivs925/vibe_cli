use crate::tools::common::ensure_args_at_least;
use crate::tools::common::run_bash;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct RagTool;

impl Tool for RagTool {
    fn name(&self) -> &str {
        "rag"
    }

    fn description(&self) -> &str {
        "Best-effort semantic lookup over local docs"
    }

    fn usage(&self) -> &str {
        "rag <query> [num_results]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["rag systemd service", "rag react flow 10"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let query = args[0];
        let limit: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(10);
        let cmd = format!(
            "rg -n --hidden --glob '!.git' -- '{}' docs src | head -n {}",
            query, limit
        );
        run_bash(&cmd)
    }
}
