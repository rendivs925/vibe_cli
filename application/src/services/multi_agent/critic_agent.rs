use super::agent::{Agent, AgentConfig, AgentRole};
use super::generator_agent::CandidateSolution;
use crate::services::rag_service::RagService;
use infrastructure::ollama_client::OllamaClient;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Critique {
    pub solution_id: String,
    pub issues: Vec<Issue>,
    pub suggestions: Vec<String>,
    pub overall_assessment: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub severity: IssueSeverity,
    pub description: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    Critical,
    Major,
    Minor,
    Info,
}

pub struct CriticAgent {
    agent: Agent,
}

impl CriticAgent {
    pub fn new(client: OllamaClient) -> Self {
        let config = AgentConfig {
            name: "Critic".to_string(),
            role: AgentRole::Critic,
            model: "qwen2.5".to_string(),
            temperature: 0.4,
            max_tokens: 4096,
        };

        Self {
            agent: Agent::new(config, client),
        }
    }

    pub fn with_rag_service(mut self, rag_service: Arc<RagService>) -> Self {
        self.agent = self.agent.with_rag_service(rag_service);
        self
    }

    pub async fn critique(&self, solution: &CandidateSolution) -> Result<Critique> {
        let prompt = format!(
            r#"Critique the following solution thoroughly.

Solution ID: {}
Content: {}

Provide a detailed critique including:
1. Issues identified (with severity levels: Critical, Major, Minor, Info)
2. Suggestions for improvement
3. Overall assessment
4. A score from 0.0 to 1.0

Return your critique in JSON format:
{{
  "solution_id": "1",
  "issues": [
    {{"severity": "Major", "description": "issue description", "location": "where it occurs"}}
  ],
  "suggestions": ["suggestion 1", "suggestion 2"],
  "overall_assessment": "overall assessment",
  "score": 0.7
}}"#,
            solution.id, solution.content
        );

        let response = self.agent.generate(&prompt).await?;
        self.parse_critique(&response, &solution.id)
    }

    pub async fn critique_batch(&self, solutions: &[CandidateSolution]) -> Result<Vec<Critique>> {
        let mut critiques = Vec::new();
        
        for solution in solutions {
            match self.critique(solution).await {
                Ok(critique) => critiques.push(critique),
                Err(e) => {
                    critiques.push(Critique {
                        solution_id: solution.id.clone(),
                        issues: vec![Issue {
                            severity: IssueSeverity::Info,
                            description: format!("Failed to critique: {}", e),
                            location: None,
                        }],
                        suggestions: vec![],
                        overall_assessment: "Critique failed".to_string(),
                        score: 0.5,
                    });
                }
            }
        }

        Ok(critiques)
    }

    fn parse_critique(&self, response: &str, solution_id: &str) -> Result<Critique> {
        let trimmed = response.trim();
        
        if let Ok(critique) = serde_json::from_str::<Critique>(trimmed) {
            return Ok(critique);
        }

        if let Some(json_start) = trimmed.find('{') {
            if let Some(json_end) = trimmed.rfind('}') {
                let json_str = &trimmed[json_start..=json_end];
                if let Ok(critique) = serde_json::from_str::<Critique>(json_str) {
                    return Ok(critique);
                }
            }
        }

        Ok(Critique {
            solution_id: solution_id.to_string(),
            issues: vec![],
            suggestions: vec![],
            overall_assessment: response.to_string(),
            score: 0.5,
        })
    }

    pub async fn aggregate_feedback(&self, critiques: &[Critique]) -> Result<String> {
        let mut issues_summary = String::new();
        let mut suggestions_summary = String::new();

        for critique in critiques {
            if !critique.issues.is_empty() {
                issues_summary.push_str(&format!(
                    "\n- Score {}: {}",
                    critique.score, critique.overall_assessment
                ));
            }
            for suggestion in &critique.suggestions {
                if !suggestions_summary.contains(suggestion) {
                    suggestions_summary.push_str(&format!("\n- {}", suggestion));
                }
            }
        }

        Ok(format!(
            "Issues:{}\nSuggestions:{}",
            if issues_summary.is_empty() {
                " None significant".to_string()
            } else {
                issues_summary
            },
            if suggestions_summary.is_empty() {
                " None provided".to_string()
            } else {
                suggestions_summary
            }
        ))
    }
}

impl Clone for CriticAgent {
    fn clone(&self) -> Self {
        Self {
            agent: self.agent.clone(),
        }
    }
}
