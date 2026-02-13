use crate::tools::common::run_bash;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct BuildTool;

impl Tool for BuildTool {
    fn name(&self) -> &str {
        "build"
    }

    fn description(&self) -> &str {
        "Run cargo build-related actions"
    }

    fn usage(&self) -> &str {
        "build <check|build|fmt|clippy> [package]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["build check", "build build", "build fmt", "build clippy presentation"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        let action = args.first().copied().unwrap_or("check");
        let package = args.get(1).copied();

        let command = match (action, package) {
            ("check", Some(pkg)) => format!("cargo check --package {pkg}"),
            ("check", None) => "cargo check".to_string(),
            ("build", Some(pkg)) => format!("cargo build --package {pkg}"),
            ("build", None) => "cargo build".to_string(),
            ("fmt", _) => "cargo fmt --all".to_string(),
            ("clippy", Some(pkg)) => format!("cargo clippy --package {pkg} -- -D warnings"),
            ("clippy", None) => "cargo clippy -- -D warnings".to_string(),
            _ => {
                return Err(ToolError::InvalidArguments(format!(
                    "unknown build action '{action}', usage: {}",
                    self.usage()
                )));
            }
        };

        let mut out = run_bash(&command)?;
        out.metadata.insert("tool_action".to_string(), action.to_string());
        out.metadata
            .insert("resolved_command".to_string(), command.to_string());
        Ok(out)
    }
}
