use crate::ports::{Cache, StorageService};
use async_trait::async_trait;
use domain::entities::command::Command;
use domain::services::command_planner::{CommandPlanner, CommandPlannerError};
use domain::CommandExecution;
use domain::CommandPlanResult;
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
    pub async fn generate_command(&self, input: &str) -> Result<CommandExecutionPlan, AppError> {
        println!("🧠 Neurosymbolic Analysis Started for: {}", input);

        // Use neurosymbolic service for enhanced reasoning
        let neurosymbolic_response = match self.neurosymbolic_service.process_query(input).await {
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
                        response.ranked_solutions[0].solution.neural_score
                    );
                    println!(
                        "⚙️ Symbolic Score: {:.1}",
                        response.ranked_solutions[0].solution.symbolic_score
                    );

                    // Convert to command using simplified approach
                    let command = domain::entities::command::Command::new(
                        format!("neuro_{}", response.ranked_solutions[0].solution.id),
                        response.ranked_solutions[0].solution.description.clone(),
                        response.ranked_solutions[0]
                            .solution
                            .command_sequence
                            .first()
                            .unwrap_or(&"")
                            .clone(),
                        vec![],
                        response.ranked_solutions[0].solution.confidence,
                    );

                    println!("🔧 Generated Command: {}", command.command_line());

                    let plan_result = domain::command_plan::CommandPlan {
                        id: format!("neuro_{}", response.ranked_solutions[0].solution.id),
                        description: response.ranked_solutions[0].solution.description.clone(),
                        steps: vec![command.command_line()],
                        safety_checks: vec![],
                    };

                    CommandExecutionPlan::new(
                        command,
                        domain::value_objects::safety_policy::SafetyResult::new(true, vec![]),
                        false,
                    )
                } else {
                    println!("⚠️ No viable neurosymbolic solutions found");
                    CommandExecutionPlan::cached(domain::entities::command::Command::new(
                        "fallback".to_string(),
                        "No solution found".to_string(),
                        "echo 'No neurosymbolic solution available'".to_string(),
                        vec![],
                        0.5,
                    ))
                }
            }
            Err(e) => {
                println!("❌ Neurosymbolic Service Error: {}", e);
                Err(AppError::domain(format!(
                    "Neurosymbolic service error: {}",
                    e
                )))
            }
        };

        // Check cache first
        let cache_key = format!("cmd:{:x}", md5::compute(input.as_bytes()));
        if let Some(cached_command) = self.cache.get(&cache_key).await? {
            println!("📋 Using cached neurosymbolic solution");
            return Ok(CommandExecutionPlan::cached(cached_command));
        }

        plan_result
    }

    /// Generate multiple commands from complex input
    pub async fn generate_multi_step(
        &self,
        input: &str,
    ) -> Result<MultiStepExecutionPlan, AppError> {
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

        Ok(MultiStepExecutionPlan::new(
            plan_result.commands().to_vec(),
            plan_result.safety_result().clone(),
        ))
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
