use async_trait::async_trait;
use shared::error::AppError;
use domain::entities::command::Command;
use domain::CommandPlanResult;
use domain::services::command_planner::{CommandPlanner, CommandPlannerError};
use crate::ports::{StorageService, Cache};

/// Use case for command generation and execution
pub struct CommandUseCase {
    command_planner: CommandPlanner,
    storage: StorageService,
    cache: Box<dyn Cache>,
}

impl CommandUseCase {
    pub fn new(
        command_planner: CommandPlanner,
        storage: StorageService,
        cache: Box<dyn Cache>,
    ) -> Self {
        Self {
            command_planner,
            storage,
            cache,
        }
    }

    /// Generate a command from natural language
    pub async fn generate_command(&self, input: &str) -> Result<CommandExecutionPlan, AppError> {
        // Check cache first
        let cache_key = format!("cmd:{:x}", md5::compute(input.as_bytes()));
        if let Some(cached_command) = self.cache.get(&cache_key).await? {
            return Ok(CommandExecutionPlan::cached(cached_command));
        }

        // Plan command using domain service
        let plan_result = self.command_planner.plan_command(input)
            .map_err(|e| AppError::domain(e.to_string()))?;

        if !plan_result.is_safe_to_execute() {
            return Err(AppError::safety("Command failed safety validation".to_string()));
        }

        let command = plan_result.command();

        // Store command in storage
        self.storage.save_command(command).await?;

        // Cache the command
        let cache_value = format!("{}|{}", command.command_line(), command.description());
        self.cache.set(&cache_key, &cache_value).await?;

        Ok(CommandExecutionPlan::new(
            command.clone(),
            plan_result.safety_result().clone(),
            false,
        ))
    }

    /// Generate multiple commands from complex input
    pub async fn generate_multi_step(&self, input: &str) -> Result<MultiStepExecutionPlan, AppError> {
        let plan_result = self.command_planner.plan_multi_step(input)
            .map_err(|e| AppError::domain(e.to_string()))?;

        if !plan_result.is_safe_to_execute() {
            return Err(AppError::safety("One or more commands failed safety validation".to_string()));
        }

        // Store all commands
        for command in plan_result.commands() {
            self.storage.save_command(command).await?;
        }

        Ok(MultiStepExecutionPlan::new(
            plan_result.commands().to_vec(),
            plan_result.safety_result().clone(),
        ))
    }

    /// Get similar commands for a query
    pub async fn get_similar_commands(&self, _query: &str, _limit: usize) -> Result<Vec<Command>, AppError> {
        // Get all commands from storage and return the most recent ones
        self.storage.get_all_commands().await.map(|cmds| cmds.into_iter().take(_limit).collect())
    }

    /// Execute a command with confirmation
    pub async fn execute_command(&self, command: &Command, confirmed: bool) -> Result<CommandExecution, AppError> {
        if !confirmed && !command.is_safe() {
            return Err(AppError::safety("Command requires confirmation".to_string()));
        }

        // Record execution
        let execution = CommandExecution::new(
            command.id().to_string(),
            command.command_line().to_string(),
            chrono::Utc::now(),
        );

        Ok(execution)
    }

    /// Get command execution history
    pub async fn get_execution_history(&self, limit: usize) -> Result<Vec<CommandExecution>, AppError> {
        // Get execution history from storage
        self.storage.get_all_executions().await.map(|execs| execs.into_iter().take(limit).collect())
    }
}

/// Plan for command execution
#[derive(Debug, Clone)]
pub struct CommandExecutionPlan {
    command: Command,
    safety_result: domain::value_objects::safety_policy::SafetyResult,
    from_cache: bool,
}

impl CommandExecutionPlan {
    pub fn new(
        command: Command,
        safety_result: domain::value_objects::safety_policy::SafetyResult,
        from_cache: bool,
    ) -> Self {
        Self {
            command,
            safety_result,
            from_cache,
        }
    }

    pub fn cached(cache_value: String) -> Self {
        // Parse cached value "command|description"
        let parts: Vec<&str> = cache_value.split('|').collect();
        let command_line = parts.get(0).unwrap_or(&"").to_string();
        let description = parts.get(1).unwrap_or(&"").to_string();

        Self::new(
            Command::new(
                "cached".to_string(),
                description,
                command_line,
                vec![],
                0.8,
            ),
            domain::value_objects::safety_policy::SafetyResult::new(true, vec![]),
            true,
        )
    }

    pub fn command(&self) -> &Command {
        &self.command
    }

    pub fn safety_result(&self) -> &domain::value_objects::safety_policy::SafetyResult {
        &self.safety_result
    }

    pub fn is_from_cache(&self) -> bool {
        self.from_cache
    }

    pub fn is_safe_to_execute(&self) -> bool {
        self.safety_result.is_safe() && self.command.is_safe()
    }
}

/// Plan for multi-step execution
#[derive(Debug, Clone)]
pub struct MultiStepExecutionPlan {
    commands: Vec<Command>,
    safety_result: domain::value_objects::safety_policy::SafetyResult,
}

impl MultiStepExecutionPlan {
    pub fn new(
        commands: Vec<Command>,
        safety_result: domain::value_objects::safety_policy::SafetyResult,
    ) -> Self {
        Self {
            commands,
            safety_result,
        }
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn safety_result(&self) -> &domain::value_objects::safety_policy::SafetyResult {
        &self.safety_result
    }

    pub fn is_safe_to_execute(&self) -> bool {
        self.safety_result.is_safe() && self.commands.iter().all(|cmd| cmd.is_safe())
    }

    pub fn step_count(&self) -> usize {
        self.commands.len()
    }
}

/// Command execution record
#[derive(Debug, Clone)]
pub struct CommandExecution {
    id: String,
    command_line: String,
    executed_at: chrono::DateTime<chrono::Utc>,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
}

impl CommandExecution {
    pub fn new(
        id: String,
        command_line: String,
        executed_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            id,
            command_line,
            executed_at,
            exit_code: None,
            duration_ms: None,
        }
    }

    pub fn with_result(mut self, exit_code: i32, duration_ms: u64) -> Self {
        self.exit_code = Some(exit_code);
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn command_line(&self) -> &str {
        &self.command_line
    }

    pub fn executed_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.executed_at
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    pub fn is_success(&self) -> bool {
        self.exit_code.map_or(false, |code| code == 0)
    }
}

#[async_trait]
pub trait AsyncCommandService: Send + Sync {
    async fn generate_command(&self, input: &str) -> Result<CommandExecutionPlan, AppError>;
    async fn execute_command(&self, command: &Command, confirmed: bool) -> Result<CommandExecution, AppError>;
}