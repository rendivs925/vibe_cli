use anyhow::anyhow;
use domain::entities::react::{ProposedCommand, ReactSession, ReactStep, ReactTool, ToolDecision, ToolResult};
use domain::repositories::react_repository::{ReactCommandRepository, ReactRepository};
use infrastructure::ollama_client::OllamaClient;
use infrastructure::session_indexing_service::SessionIndexingService;
use infrastructure::storage::KnowledgeGraph;
use shared::types::Result;
use std::path::PathBuf;
use std::sync::Arc;

use crate::services::learning_service::LearningService;
use crate::services::react_analysis_service::AnalysisService;
use crate::services::react_command_parser::parse_command_list;
use crate::services::react_context_retriever::ContextRetriever;
use crate::services::react_prompt_service::ReactPromptService;
use crate::services::neurosymbolic_service::NeurosymbolicService;
use crate::services::react_tools::{ReactConfig, ToolMode, ToolRegistry};

mod workflow;

pub struct ReactAgentService {
    neurosymbolic_service: Option<Arc<NeurosymbolicService>>,
    react_repository: Arc<dyn ReactRepository>,
    command_repository: Arc<dyn ReactCommandRepository>,
    indexing_service: Option<Arc<SessionIndexingService>>,
    client: OllamaClient,
    analysis_service: AnalysisService,
    context_retriever: ContextRetriever,
    prompt_service: ReactPromptService,
    learning_service: LearningService,
    tool_registry: ToolRegistry,
    react_config: ReactConfig,
    max_iterations: u32,
}

impl ReactAgentService {
    pub fn new(
        neurosymbolic_service: Option<Arc<NeurosymbolicService>>,
        react_repository: Arc<dyn ReactRepository>,
        command_repository: Arc<dyn ReactCommandRepository>,
    ) -> Result<Self> {
        let knowledge_graph = init_knowledge_graph_arc();
        let context_retriever = if let Some(kg) = knowledge_graph {
            ContextRetriever::new().with_knowledge_graph(kg)
        } else {
            ContextRetriever::new()
        };

        Ok(Self {
            neurosymbolic_service,
            react_repository,
            command_repository,
            indexing_service: None,
            client: OllamaClient::new()?,
            analysis_service: AnalysisService::new(),
            context_retriever,
            prompt_service: ReactPromptService::new(),
            learning_service: LearningService::new()?,
            tool_registry: ToolRegistry::with_default_handlers(),
            react_config: ReactConfig::default(),
            max_iterations: 10,
        })
    }

    pub fn with_max_iterations(mut self, max_iterations: u32) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Configure the ReAct tool system
    pub fn with_config(mut self, config: ReactConfig) -> Self {
        self.react_config = config;
        self
    }

    /// Set the tool mode
    pub fn with_tool_mode(mut self, mode: ToolMode) -> Self {
        self.react_config.tool_mode = mode;
        self
    }

    /// Get a reference to the tool registry
    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    /// Get a mutable reference to the tool registry
    pub fn tool_registry_mut(&mut self) -> &mut ToolRegistry {
        &mut self.tool_registry
    }

    /// Get the current configuration
    pub fn config(&self) -> &ReactConfig {
        &self.react_config
    }

    /// Enable semantic indexing for cross-session search
    pub fn with_indexing_service(mut self, service: Arc<SessionIndexingService>) -> Self {
        self.indexing_service = Some(service.clone());
        // Also update context retriever to use the indexing service
        self.context_retriever = self.context_retriever.with_indexing_service(service);
        self
    }

    pub async fn start_session(&self, query: String, neurosymbolic: bool) -> Result<ReactSession> {
        let mut session = ReactSession::new(query, neurosymbolic);
        let intent = self.analysis_service.infer_intent(&session.query);
        session.set_intent(intent);
        for constraint in self.analysis_service.extract_constraints(&session.query) {
            session.memory.add_constraint(constraint);
        }
        self.react_repository
            .save_session(&session)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        Ok(session)
    }

    pub async fn generate_reasoning(&self, session: &ReactSession) -> Result<String> {
        let context = self.context_retriever.retrieve_with_semantic_search(session).await;
        let learning_context = self
            .learning_service
            .format_learning_context(&session.query)
            .unwrap_or_default();
        let failed = self
            .learning_service
            .get_failed_commands(&session.query, 5)
            .unwrap_or_default();
        let failed_commands = if failed.is_empty() {
            "None".to_string()
        } else {
            failed.join("; ")
        };

        let learning_context = if learning_context.trim().is_empty() {
            String::new()
        } else {
            learning_context
        };
        let prompt = self.prompt_service.reasoning_prompt(
            &session.query,
            &context,
            &learning_context,
            &failed_commands,
        );

        let response = self.client.generate_response(&prompt).await?;
        let thought = response
            .trim()
            .trim_start_matches("ANALYZE:")
            .trim()
            .to_string();
        if thought.is_empty() {
            return Err(anyhow!("empty reasoning response"));
        }
        Ok(thought)
    }

    pub async fn propose_commands(
        &self,
        reasoning: &str,
        session: &ReactSession,
    ) -> Result<Vec<ProposedCommand>> {
        if session.neurosymbolic_enabled {
            if let Some(service) = self.neurosymbolic_service.as_ref() {
                if let Some(suggestion) = service.suggest_commands_from_domains(&session.query) {
                    let mut commands = Vec::new();
                    let reasoning = format!(
                        "Matched domain op '{}' (id: {}, confidence {:.0}%)",
                        suggestion.op_name,
                        suggestion.op_id,
                        suggestion.confidence * 100.0
                    );
                    for command in suggestion.commands {
                        commands.push(ProposedCommand::new(
                            command,
                            format!("Domain op: {}", suggestion.op_name),
                            reasoning.clone(),
                        ));
                    }
                    if !commands.is_empty() {
                        return Ok(commands);
                    }
                }
            }
        }

        let context = self.context_retriever.retrieve_with_semantic_search(session).await;
        let failed = self
            .learning_service
            .get_failed_commands(&session.query, 5)
            .unwrap_or_default();
        let failed_commands = if failed.is_empty() {
            "None".to_string()
        } else {
            failed.join("; ")
        };
        let prompt = self.prompt_service.command_prompt(
            &session.query,
            reasoning,
            &context,
            &failed_commands,
        );

        let response = self.client.generate_response(&prompt).await?;
        let parsed = parse_command_list(&response);
        let mut commands = Vec::new();
        for command in parsed {
            if command.trim().is_empty() {
                continue;
            }
            commands.push(ProposedCommand::new(
                command,
                "LLM proposed".to_string(),
                reasoning.to_string(),
            ));
        }
        Ok(commands)
    }

    pub async fn execute_approved_command(&self, command: &mut ProposedCommand) -> Result<()> {
        if command.approved != Some(true) {
            return Err(anyhow!("command not approved"));
        }

        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&command.command)
            .output()?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        command.execute(exit_code, stdout, stderr);
        Ok(())
    }

    pub async fn save_session(&self, session: &ReactSession) -> Result<()> {
        self.react_repository
            .update_session(session)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn save_step(&self, step: &ReactStep) -> Result<()> {
        self.react_repository
            .save_step(step)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn update_step(&self, step: &ReactStep) -> Result<()> {
        self.react_repository
            .update_step(step)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn save_command(&self, command: &ProposedCommand) -> Result<()> {
        self.command_repository
            .save_command(command)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn update_command(&self, command: &ProposedCommand) -> Result<()> {
        self.command_repository
            .update_command(command)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    async fn add_step(&self, session: &mut ReactSession, mut step: ReactStep) -> Result<()> {
        step.start();
        step.complete();
        self.react_repository
            .save_step(&step)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        session.add_step(step);
        self.react_repository
            .update_session(session)
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

}

impl ReactAgentService {
    /// Index the current session for semantic search
    pub async fn index_session(&self, session: &ReactSession) -> Result<()> {
        if let Some(ref service) = self.indexing_service {
            let success_rate = if session.steps.is_empty() {
                0.0
            } else {
                let successful = session.steps.iter()
                    .filter(|s| matches!(s.status, domain::entities::react::ReactStepStatus::Completed))
                    .count();
                successful as f32 / session.steps.len() as f32
            };

            service.index_session(
                &session.id,
                &session.query,
                session.compacted_summary.as_deref(),
                Some(session.memory.semantic_tags.clone()),
                success_rate,
            ).await?;
        }
        Ok(())
    }

    /// Select the appropriate tool for the next step
    /// 
    /// This method:
    /// 1. Generates a tool selection prompt with all available tools
    /// 2. Calls the LLM to select a tool
    /// 3. Parses the response to extract the selected tool
    /// 4. Returns error if no valid tool can be selected
    pub async fn select_tool(&self, session: &ReactSession, reasoning: &str) -> Result<ToolDecision> {
        let context = self.context_retriever.retrieve_with_semantic_search(session).await;
        let prompt = self.prompt_service.tool_selection_prompt(&session.query, reasoning, &context);
        
        let response = self.client.generate_response(&prompt).await?;
        
        // Parse the tool selection response
        if let Some((tool, justification, context_needed)) = self.prompt_service.parse_tool_selection(&response) {
            return Ok(ToolDecision {
                tool,
                justification,
                context_needed,
                confidence: 1.0,
            });
        }
        
        // If parsing fails, return error - don't silently fallback
        Err(anyhow!("Failed to parse tool selection from LLM response"))
    }

    /// Execute a tool and return the result
    /// 
    /// Uses the ToolRegistry to find and execute the appropriate handler.
    /// Falls back to default behavior based on ReactConfig if needed.
    pub async fn execute_tool(&self, tool: ReactTool, session: &ReactSession, _reasoning: &str) -> Result<ToolResult> {
        // Full mode: execute tool directly from registry
        if let Some(handler) = self.tool_registry.get(tool) {
            let context = self.context_retriever.retrieve_with_semantic_search(session).await;
            handler.execute(&context, None).await
        } else {
            Err(anyhow!("Tool {:?} not found in registry", tool))
        }
    }

    /// Get tool suggestion based on next_tool from previous result
    pub async fn get_next_tool(&self, current_result: &ToolResult) -> Option<ReactTool> {
        current_result.next_tool_suggestion
    }

    /// Index a command execution
    pub async fn index_command_execution(
        &self,
        command: &ProposedCommand,
        session_id: &str,
    ) -> Result<()> {
        if let Some(ref service) = self.indexing_service {
            let command_id = format!("{}-{}", session_id, command.id);
            let exit_code = command.exit_code.unwrap_or(-1);
            
            service.index_command(
                &command_id,
                session_id,
                &command.command,
                command.stdout.as_deref(),
                exit_code,
            ).await?;
        }
        Ok(())
    }
}

fn init_knowledge_graph() -> Option<KnowledgeGraph> {
    let home = std::env::var("HOME").ok()?;
    let kg_path = PathBuf::from(home).join(".config/vibe_cli/knowledge_graph.db");
    KnowledgeGraph::new(kg_path).ok()
}

fn init_knowledge_graph_arc() -> Option<Arc<KnowledgeGraph>> {
    init_knowledge_graph().map(Arc::new)
}
