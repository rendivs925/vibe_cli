use crate::tools::common::ensure_args_at_least;
use crate::tools::common::run_bash;
use domain::tools::{ServiceManager, Tool, ToolError, ToolOutput};

pub struct SvcTool;

impl Tool for SvcTool {
    fn name(&self) -> &str {
        "svc"
    }

    fn description(&self) -> &str {
        "Run service action with init-system detection"
    }

    fn usage(&self) -> &str {
        "svc <start|stop|restart|status|enable|disable> <service>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["svc status nginx", "svc restart sshd"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 2, self.usage())?;
        let mgr = ServiceManager::detect();
        let action = args[0];
        let service = args[1];

        let command = match action {
            "start" => mgr.start_command(service),
            "stop" => mgr.stop_command(service),
            "restart" => mgr.restart_command(service),
            "status" => mgr.status_command(service),
            "enable" => mgr.enable_command(service),
            "disable" => mgr.disable_command(service),
            _ => {
                return Err(ToolError::InvalidArguments(format!(
                    "unknown action '{action}', usage: {}",
                    self.usage()
                )));
            }
        };

        let mut out = run_bash(&command)?;
        out.metadata.insert(
            "detected_service_manager".to_string(),
            mgr.name().to_string(),
        );
        out.metadata
            .insert("resolved_command".to_string(), command.to_string());
        Ok(out)
    }
}
