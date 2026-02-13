use crate::tools::common::{ensure_args_at_least, run_bash};
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct GitTool;

impl Tool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Structured git operations for agent workflows"
    }

    fn usage(&self) -> &str {
        "git <status|diff|add|commit|log> [args]"
    }

    fn examples(&self) -> Vec<&str> {
        vec![
            "git status",
            "git diff presentation/src/cli/handlers/react.rs",
            "git add presentation/src/cli/handlers/react.rs",
            "git commit fix react flow improvements",
        ]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let action = args[0];
        let tail = if args.len() > 1 {
            args[1..].join(" ")
        } else {
            String::new()
        };

        let command = match action {
            "status" => "git status --short".to_string(),
            "diff" => {
                if tail.is_empty() {
                    "git diff".to_string()
                } else {
                    format!("git diff -- {tail}")
                }
            }
            "add" => {
                if tail.is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "git add requires file paths".to_string(),
                    ));
                }
                format!("git add {tail}")
            }
            "commit" => {
                if tail.is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "git commit requires a message".to_string(),
                    ));
                }
                let message = tail.replace('"', "\\\"");
                format!("git commit -m \"{message}\"")
            }
            "log" => "git log --oneline -20".to_string(),
            _ => {
                return Err(ToolError::InvalidArguments(format!(
                    "unknown git action '{action}', usage: {}",
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
