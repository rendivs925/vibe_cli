use super::super::entities::command::{Command, SafetyCheck};
use super::super::value_objects::safety_policy::{SafetyPolicy, SafetyResult};
use async_trait::async_trait;

/// Domain service for planning commands with safety validation
pub struct CommandPlanner {
    safety_policy: SafetyPolicy,
}

impl CommandPlanner {
    pub fn new(safety_policy: SafetyPolicy) -> Self {
        Self { safety_policy }
    }

    pub fn with_default_policy() -> Self {
        Self::new(SafetyPolicy::default())
    }

    pub fn with_strict_policy() -> Self {
        Self::new(SafetyPolicy::strict())
    }

    /// Plan a command from natural language input
    pub fn plan_command(&self, input: &str) -> Result<CommandPlanResult, CommandPlannerError> {
        // Parse the command from natural language
        let command_line = self.extract_command(input)?;
        let description = self.generate_description(input, &command_line);

        // Generate safety checks
        let safety_checks = self.generate_safety_checks(&command_line);

        // Validate against safety policy
        let safety_result = self.safety_policy.validate_command(&command_line);

        // Create command
        let command = Command::new(
            self.generate_id(),
            description,
            command_line,
            safety_checks,
            self.calculate_confidence(input),
        );

        Ok(CommandPlanResult::new(command, safety_result))
    }

    /// Plan multiple commands from complex input
    pub fn plan_multi_step(&self, input: &str) -> Result<MultiStepPlanResult, CommandPlannerError> {
        let steps = self.parse_steps(input)?;
        let mut commands = Vec::new();

        for step in &steps {
            let plan_result = self.plan_command(step)?;
            commands.push(plan_result.command().clone());
        }

        let overall_safety = self.validate_multiple_commands(&commands);
        Ok(MultiStepPlanResult::new(commands, overall_safety))
    }

    // Private helper methods
    fn extract_command(&self, input: &str) -> Result<String, CommandPlannerError> {
        let cleaned = input.trim();

        if cleaned.is_empty() {
            return Err(CommandPlannerError::EmptyInput);
        }

        // Simple extraction - in real implementation, this would use NLP/AI
        let command = if cleaned.starts_with("run ") {
            cleaned[4..].trim().to_string()
        } else if cleaned.starts_with("execute ") {
            cleaned[8..].trim().to_string()
        } else if cleaned.contains("command:") {
            cleaned
                .split("command:")
                .nth(1)
                .unwrap_or("")
                .trim()
                .to_string()
        } else {
            cleaned.to_string()
        };

        if command.is_empty() {
            Err(CommandPlannerError::CannotExtractCommand)
        } else {
            Ok(command)
        }
    }

    fn generate_description(&self, input: &str, _command: &str) -> String {
        format!("Generated from input: '{}'", input)
    }

    fn generate_safety_checks(&self, command: &str) -> Vec<SafetyCheck> {
        // This would be more sophisticated in a real implementation
        vec![
            SafetyCheck::new(
                super::super::entities::command::SafetyCheckType::FileSystemWrite,
                !command.to_lowercase().contains("rm "),
            ),
            SafetyCheck::new(
                super::super::entities::command::SafetyCheckType::NetworkAccess,
                !command.to_lowercase().contains("curl"),
            ),
        ]
    }

    fn calculate_confidence(&self, input: &str) -> f32 {
        // Simple confidence calculation - in real implementation would use ML
        if input.len() > 50 {
            0.8
        } else {
            0.6
        }
    }

    fn generate_id(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        format!(
            "cmd_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        )
    }

    fn parse_steps(&self, input: &str) -> Result<Vec<String>, CommandPlannerError> {
        // Simple step parsing - look for numbered steps or "then" keywords
        let steps: Vec<String> = input
            .split(|c| c == '\n' || c == ';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if steps.is_empty() {
            Err(CommandPlannerError::CannotExtractCommand)
        } else {
            Ok(steps)
        }
    }

    fn validate_multiple_commands(&self, commands: &[Command]) -> SafetyResult {
        let all_checks: Vec<_> = commands
            .iter()
            .flat_map(|cmd| cmd.safety_checks().to_vec())
            .collect();

        let overall_safe = all_checks.iter().all(|check| check.passed());
        SafetyResult::new(overall_safe, all_checks)
    }
}

/// Result of command planning
#[derive(Debug, Clone)]
pub struct CommandPlanResult {
    command: Command,
    safety_result: SafetyResult,
}

impl CommandPlanResult {
    pub fn new(command: Command, safety_result: SafetyResult) -> Self {
        Self {
            command,
            safety_result,
        }
    }

    pub fn command(&self) -> &Command {
        &self.command
    }

    pub fn safety_result(&self) -> &SafetyResult {
        &self.safety_result
    }

    pub fn is_safe_to_execute(&self) -> bool {
        self.safety_result.is_safe() && self.command.is_safe()
    }
}

/// Result of multi-step planning
#[derive(Debug, Clone)]
pub struct MultiStepPlanResult {
    commands: Vec<Command>,
    safety_result: SafetyResult,
    error_message: Option<String>,
}

impl MultiStepPlanResult {
    pub fn new(commands: Vec<Command>, safety_result: SafetyResult) -> Self {
        Self {
            commands,
            safety_result,
            error_message: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            commands: vec![],
            safety_result: SafetyResult::new(false, vec![]),
            error_message: Some(message),
        }
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn safety_result(&self) -> &SafetyResult {
        &self.safety_result
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn is_success(&self) -> bool {
        self.error_message.is_none()
    }

    pub fn is_safe_to_execute(&self) -> bool {
        self.is_success() && self.safety_result.is_safe()
    }
}

/// Errors that can occur during command planning
#[derive(Debug, Clone)]
pub enum CommandPlannerError {
    EmptyInput,
    CannotExtractCommand,
    InvalidSyntax,
}

impl std::fmt::Display for CommandPlannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandPlannerError::EmptyInput => write!(f, "Input is empty"),
            CommandPlannerError::CannotExtractCommand => {
                write!(f, "Cannot extract command from input")
            }
            CommandPlannerError::InvalidSyntax => write!(f, "Invalid command syntax"),
        }
    }
}

impl std::error::Error for CommandPlannerError {}

/// Trait for async command planning (for future extension)
#[async_trait]
pub trait AsyncCommandPlanner: Send + Sync {
    async fn plan_command_async(
        &self,
        input: &str,
    ) -> Result<CommandPlanResult, CommandPlannerError>;
    async fn plan_multi_step_async(
        &self,
        input: &str,
    ) -> Result<MultiStepPlanResult, CommandPlannerError>;
}
