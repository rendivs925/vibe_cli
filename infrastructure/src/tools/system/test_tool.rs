use crate::tools::common::run_bash;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct TestTool;

impl Tool for TestTool {
    fn name(&self) -> &str {
        "test"
    }

    fn description(&self) -> &str {
        "Run test commands"
    }

    fn usage(&self) -> &str {
        "test [pattern]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["test", "test react", "test domain_config"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        let command = if let Some(pattern) = args.first() {
            format!("cargo test {pattern}")
        } else {
            "cargo test".to_string()
        };

        let mut out = run_bash(&command)?;
        out.metadata
            .insert("resolved_command".to_string(), command.to_string());
        Ok(out)
    }
}
