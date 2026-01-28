use domain::{Command, SafetyPolicy};
use shared::types::Result;

pub struct SafetyService {
    policy: SafetyPolicy,
}

impl SafetyService {
    pub fn new() -> Self {
        Self {
            policy: SafetyPolicy::new(),
        }
    }

    pub fn validate_command(&self, command: &Command) -> Result<()> {
        let safety_result = self.policy.validate_command_entity(command);

        if safety_result.is_safe() {
            Ok(())
        } else {
            let failed_checks: Vec<String> = safety_result
                .checks()
                .iter()
                .filter(|check| !check.passed())
                .map(|check| {
                    format!(
                        "{}: {}",
                        check.check_type().description(),
                        check.reason().unwrap_or("No reason provided")
                    )
                })
                .collect();

            Err(anyhow::anyhow!(
                "Command failed safety validation: {}",
                failed_checks.join(", ")
            ))
        }
    }

    pub fn validate_command_string(&self, command_line: &str) -> Result<()> {
        let safety_result = self.policy.validate_command(command_line);

        if safety_result.is_safe() {
            Ok(())
        } else {
            let failed_checks: Vec<String> = safety_result
                .checks()
                .iter()
                .filter(|check| !check.passed())
                .map(|check| {
                    format!(
                        "{}: {}",
                        check.check_type().description(),
                        check.reason().unwrap_or("No reason provided")
                    )
                })
                .collect();

            Err(anyhow::anyhow!(
                "Command failed safety validation: {}",
                failed_checks.join(", ")
            ))
        }
    }
}
