use crate::ports::{Cache, StorageService};
use domain::entities::command::Command;
use domain::services::command_planner::CommandPlanner;
use domain::CommandExecution;
use shared::error::AppError;

/// Enhanced use case for command generation with neurosymbolic reasoning
pub struct CommandUseCase {
    command_planner: CommandPlanner,
    storage: StorageService,
    cache: Box<dyn Cache>,
    neurosymbolic_service: crate::services::neurosymbolic_service::NeurosymbolicService,
}

impl CommandUseCase {
    pub fn new(
        command_planner: CommandPlanner,
        storage: StorageService,
        cache: Box<dyn Cache>,
        neurosymbolic_service: crate::services::neurosymbolic_service::NeurosymbolicService,
    ) -> Self {
        Self {
            command_planner,
            storage,
            cache,
            neurosymbolic_service,
        }
    }

    /// Generate a command using enhanced neurosymbolic reasoning
    pub async fn generate_command(&mut self, input: &str) -> Result<CommandExecution, AppError> {
        // Check cache first
        let cache_key = format!("cmd:{:x}", md5::compute(input.as_bytes()));
        if let Some(cached_command_str) = self.cache.get(&cache_key).await? {
            let cached_command: Command = serde_json::from_str(&cached_command_str).map_err(|e| AppError::new(e.to_string()))?;
            println!("📋 Using cached neurosymbolic solution");
            return Ok(CommandExecution::from_command(&cached_command));
        }

        println!("🧠 Neurosymbolic Analysis Started for: {}", input);

        // Use neurosymbolic service for enhanced reasoning
        match self.neurosymbolic_service.process_query(input).await {
            Ok(response) => {
                println!("✅ Neurosymbolic Processing Complete");
                println!("🎯 Intent: {}", response.intent.id);
                println!("📊 Confidence: {:.1}%", response.confidence * 100.0);

                if !response.ranked_solutions.is_empty() {
                    println!(
                        "🏆 Top Solution: {}",
                        response.ranked_solutions[0].solution.description
                    );
                    println!(
                        "🧠 Neural Score: {:.1}",
                        response.ranked_solutions[0].neural_score
                    );
                    println!(
                        "⚙️ Symbolic Score: {:.1}",
                        response.ranked_solutions[0].symbolic_score
                    );

                    // Convert to command using simplified approach
                    let command = domain::entities::command::Command::new(
                        format!("neuro_{}", response.ranked_solutions[0].solution.id),
                        response.ranked_solutions[0].solution.description.clone(),
                        response.ranked_solutions[0]
                            .solution
                            .command_sequence
                            .first()
                            .unwrap_or(&"".to_string())
                            .clone(),
                        vec![],
                        response.ranked_solutions[0].combined_score,
                    );

                    println!("🔧 Generated Command: {}", command.command_line());
                    self.cache.set(&cache_key, &serde_json::to_string(&command).unwrap()).await?;
                    Ok(CommandExecution::from_command(&command))
                } else {
                    println!("⚠️ No viable neurosymbolic solutions found");
                    let command = domain::entities::command::Command::new(
                        "fallback".to_string(),
                        "No solution found".to_string(),
                        "echo 'No neurosymbolic solution available'".to_string(),
                        vec![],
                        0.5,
                    );
                    Ok(CommandExecution::from_command(&command))
                }
            }
            Err(e) => {
                println!("❌ Neurosymbolic Service Error: {}", e);
                Err(AppError::domain(format!(
                    "Neurosymbolic service error: {}",
                    e
                )))
            }
        }
    }

    /// Generate multiple commands from complex input
    pub async fn generate_multi_step(&self, input: &str) -> Result<CommandExecution, AppError> {
        // Use enhanced command planner for multi-step analysis
        let plan_result = self
            .command_planner
            .plan_multi_step(input)
            .map_err(|e| AppError::domain(e.to_string()))?;

        if !plan_result.is_safe_to_execute() {
            return Err(AppError::safety(
                "Multi-step plan failed safety validation".to_string(),
            ));
        }

        // Store all commands
        for command in plan_result.commands() {
            self.storage.save_command(command).await?;
        }

        let first_command = plan_result.commands().first().ok_or_else(|| AppError::domain("No commands in plan".to_string()))?;

        Ok(CommandExecution::from_command(first_command))
    }

    /// Get similar commands for a query
    pub async fn get_similar_commands(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<Command>, AppError> {
        // Get all commands from storage and return recent ones
        self.storage
            .get_all_commands()
            .await
            .map(|cmds| cmds.into_iter().take(_limit).collect())
    }

    /// Execute a command with confirmation
    pub async fn execute_command(
        &self,
        command: &Command,
        confirmed: bool,
    ) -> Result<CommandExecution, AppError> {
        if !confirmed && !command.is_safe() {
            return Err(AppError::safety(
                "Command requires confirmation".to_string(),
            ));
        }

        // Record execution
        let execution = CommandExecution::new(
            command.id().to_string(),
            command.command_line().to_string(),
            chrono::Utc::now(),
        );

        // Save execution to storage
        self.storage.save_execution(&execution).await?;

        Ok(execution)
    }

    /// Get command execution history
    pub async fn get_execution_history(
        &self,
        limit: usize,
    ) -> Result<Vec<CommandExecution>, AppError> {
        // Get execution history from storage
        self.storage
            .get_all_executions()
            .await
            .map(|execs| execs.into_iter().take(limit).collect())
    }
}
