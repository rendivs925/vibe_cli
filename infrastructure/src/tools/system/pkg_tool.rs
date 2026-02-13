use crate::tools::common::{ensure_args_at_least, run_bash};
use domain::tools::{PackageManager, Tool, ToolError, ToolOutput};

pub struct PkgTool;

impl Tool for PkgTool {
    fn name(&self) -> &str {
        "pkg"
    }

    fn description(&self) -> &str {
        "Run package manager action with distro detection"
    }

    fn usage(&self) -> &str {
        "pkg <install|remove|search|update|upgrade> [package]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["pkg install nginx", "pkg search redis", "pkg upgrade"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;

        let pm = PackageManager::detect();
        let action = args[0];
        let package = args.get(1).copied().unwrap_or_default();

        let command = match action {
            "install" => {
                if package.is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "install requires a package".to_string(),
                    ));
                }
                pm.install_command(package)
            }
            "remove" => {
                if package.is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "remove requires a package".to_string(),
                    ));
                }
                pm.remove_command(package)
            }
            "search" => {
                if package.is_empty() {
                    return Err(ToolError::InvalidArguments(
                        "search requires a query".to_string(),
                    ));
                }
                pm.search_command(package)
            }
            "update" => pm.update_command(),
            "upgrade" => pm.upgrade_command(),
            _ => {
                return Err(ToolError::InvalidArguments(format!(
                    "unknown action '{action}', usage: {}",
                    self.usage()
                )));
            }
        };

        let mut out = run_bash(&command)?;
        out.metadata.insert(
            "detected_package_manager".to_string(),
            pm.name().to_string(),
        );
        out.metadata
            .insert("resolved_command".to_string(), command.to_string());
        Ok(out)
    }
}
