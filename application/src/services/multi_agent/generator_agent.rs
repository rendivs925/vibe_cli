use super::agent::{Agent, AgentConfig, AgentRole};
use crate::services::rag_service::RagService;
use infrastructure::ollama_client::OllamaClient;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSolution {
    pub id: String,
    pub content: String,
    pub confidence: f32,
    pub reasoning: String,
}

impl CandidateSolution {
    pub fn new(id: String, content: String) -> Self {
        Self {
            id,
            content,
            confidence: 0.5,
            reasoning: String::new(),
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = reasoning;
        self
    }
}

pub struct GeneratorAgent {
    agent: Agent,
    num_candidates: usize,
}

impl GeneratorAgent {
    pub fn new(client: OllamaClient, num_candidates: usize) -> Self {
        let config = AgentConfig {
            name: "Generator".to_string(),
            role: AgentRole::Generator,
            model: "qwen2.5".to_string(),
            temperature: 0.8,
            max_tokens: 4096,
        };
        
        Self {
            agent: Agent::new(config, client),
            num_candidates,
        }
    }

    pub fn with_rag_service(mut self, rag_service: Arc<RagService>) -> Self {
        self.agent = self.agent.with_rag_service(rag_service);
        self
    }

    pub async fn generate_candidates(&self, task: &str) -> Result<Vec<CandidateSolution>> {
        let prompt = format!(
            r#"Generate {} distinct candidate solutions for the following task.

Task: {}

For each solution:
1. Provide a clear, actionable response
2. Consider different approaches and perspectives
3. Ensure the solution is practical and implementable

Return your solutions in the following JSON format:
[
  {{"id": "1", "content": "solution description", "confidence": 0.8, "reasoning": "why this approach"}},
  ...
]"#,
            self.num_candidates, task
        );

        let response = self.agent.generate(&prompt).await?;
        self.parse_candidates(&response)
    }

    pub async fn generate_with_iteration(
        &self,
        task: &str,
        feedback: &str,
        iteration: usize,
    ) -> Result<Vec<CandidateSolution>> {
        let prompt = format!(
            r#"Generate {} distinct candidate solutions for the following task, taking into account the feedback provided.

Task: {}

Previous Feedback:
{}

Iteration: {}

For each solution:
1. Address the feedback provided
2. Provide a clear, actionable response
3. Consider different approaches and perspectives

Return your solutions in JSON format:
[
  {{"id": "1", "content": "solution description", "confidence": 0.8, "reasoning": "why this approach"}},
  ...
]"#,
            self.num_candidates, task, feedback, iteration
        );

        let response = self.agent.generate(&prompt).await?;
        self.parse_candidates(&response)
    }

    fn parse_candidates(&self, response: &str) -> Result<Vec<CandidateSolution>> {
        let trimmed = response.trim();
        
        if let Ok(candidates) = serde_json::from_str::<Vec<CandidateSolution>>(trimmed) {
            return Ok(candidates);
        }

        if let Some(json_start) = trimmed.find('[') {
            if let Some(json_end) = trimmed.rfind(']') {
                let json_str = &trimmed[json_start..=json_end];
                if let Ok(candidates) = serde_json::from_str::<Vec<CandidateSolution>>(json_str) {
                    return Ok(candidates);
                }
            }
        }

        let lines: Vec<&str> = trimmed.lines().filter(|l| !l.is_empty()).collect();
        let mut candidates = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            if line.contains("content") || line.contains("solution") {
                let id = format!("{}", i + 1);
                let content = line.trim().trim_start_matches('-').trim().to_string();
                if !content.is_empty() {
                    candidates.push(CandidateSolution::new(id, content));
                }
            }
        }

        if candidates.is_empty() {
            candidates.push(CandidateSolution::new(
                "1".to_string(),
                response.to_string(),
            ));
        }

        Ok(candidates)
    }
}

impl Clone for GeneratorAgent {
    fn clone(&self) -> Self {
        Self {
            agent: self.agent.clone(),
            num_candidates: self.num_candidates,
        }
    }
}
