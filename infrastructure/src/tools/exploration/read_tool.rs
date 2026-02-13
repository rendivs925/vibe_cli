use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};
use std::fs;

pub struct ReadTool;

impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read file contents with optional line window"
    }

    fn usage(&self) -> &str {
        "read <path> [lines] [offset]"
    }

    fn examples(&self) -> Vec<&str> {
        vec![
            "read src/main.rs",
            "read src/main.rs 50",
            "read src/main.rs 20 100",
        ]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let path = args[0];
        let lines: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(200);
        let offset: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);

        let content = fs::read_to_string(path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound(path.to_string())
            } else {
                ToolError::ExecutionFailed(err.to_string())
            }
        })?;

        let stdout = content
            .lines()
            .skip(offset)
            .take(lines)
            .enumerate()
            .map(|(idx, line)| format!("{}: {}", offset + idx + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput::success(stdout))
    }
}
