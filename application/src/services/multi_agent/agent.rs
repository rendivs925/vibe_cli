use crate::services::rag_service::RagService;
use infrastructure::ollama_client::OllamaClient;
use shared::types::Result;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub role: AgentRole,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "Agent".to_string(),
            role: AgentRole::Generator,
            model: "qwen2.5".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    Generator,
    Critic,
    Tester,
}

pub struct Agent {
    config: AgentConfig,
    client: OllamaClient,
    rag_service: Option<Arc<RagService>>,
}

impl Agent {
    pub fn new(config: AgentConfig, client: OllamaClient) -> Self {
        Self {
            config,
            client,
            rag_service: None,
        }
    }

    pub fn with_rag_service(mut self, rag_service: Arc<RagService>) -> Self {
        self.rag_service = Some(rag_service);
        self
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    pub fn role(&self) -> AgentRole {
        self.config.role
    }

    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub async fn generate(&self, prompt: &str) -> Result<String> {
        let context = self.retrieve_context(prompt).await?;
        
        let full_prompt = if context.is_empty() {
            prompt.to_string()
        } else {
            format!(
                "{}\n\nRelevant Context:\n{}\n\nTask:\n{}",
                self.get_system_prompt(),
                context,
                prompt
            )
        };

        self.client
            .generate_response(&full_prompt)
            .await
    }

    pub async fn generate_with_context(&self, prompt: &str, extra_context: &str) -> Result<String> {
        let context = self.retrieve_context(prompt).await?;
        
        let full_prompt = format!(
            "{}\n\nRelevant Context:\n{}\n{}\n\nTask:\n{}",
            self.get_system_prompt(),
            context,
            extra_context,
            prompt
        );

        self.client
            .generate_response(&full_prompt)
            .await
    }

    fn get_system_prompt(&self) -> String {
        match self.config.role {
            AgentRole::Generator => {
                "You are a Generator Agent. Your role is to produce multiple candidate solutions \
                for the given task. Generate diverse, high-quality solutions that address the core problem. \
                Provide clear, actionable outputs.".to_string()
            }
            AgentRole::Critic => {
                "You are a Critic Agent. Your role is to evaluate candidate solutions, identify flaws, \
                and suggest improvements. Be thorough, objective, and constructive. Point out potential \
                issues and provide actionable feedback.".to_string()
            }
            AgentRole::Tester => {
                "You are a Tester Agent. Your role is to validate solutions against requirements, \
                check for safety issues, and verify correctness. Test thoroughly and provide clear \
                pass/fail assessments with reasoning.".to_string()
            }
        }
    }

    async fn retrieve_context(&self, query: &str) -> Result<String> {
        if let Some(rag) = &self.rag_service {
            let snippets = rag.relevant_chunks(query, 5).await?;
            if snippets.is_empty() {
                return Ok(String::new());
            }
            Ok(snippets.join("\n\n"))
        } else {
            Ok(String::new())
        }
    }
}

impl Clone for Agent {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            client: OllamaClient::new().expect("Failed to create Ollama client"),
            rag_service: self.rag_service.clone(),
        }
    }
}
