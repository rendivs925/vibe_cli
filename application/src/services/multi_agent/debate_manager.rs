use super::consensus::Consensus;
use super::critic_agent::{CriticAgent, Critique};
use super::generator_agent::{CandidateSolution, GeneratorAgent};
use super::tester_agent::{TesterAgent, TestResult};
use crate::services::rag_service::RagService;
use infrastructure::ollama_client::OllamaClient;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateConfig {
    pub num_candidates: usize,
    pub max_iterations: usize,
    pub enable_critique: bool,
    pub enable_testing: bool,
    pub early_exit_threshold: f32,
}

impl Default for DebateConfig {
    fn default() -> Self {
        Self {
            num_candidates: 3,
            max_iterations: 3,
            enable_critique: true,
            enable_testing: true,
            early_exit_threshold: 0.85,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebateResult {
    pub solution: Option<CandidateSolution>,
    pub all_solutions: Vec<CandidateSolution>,
    pub iterations: usize,
    pub consensus_result: String,
    pub success: bool,
}

pub struct DebateManager {
    generator: GeneratorAgent,
    critic: Option<CriticAgent>,
    tester: Option<TesterAgent>,
    consensus: Consensus,
    config: DebateConfig,
}

impl DebateManager {
    pub fn new(client: OllamaClient) -> Self {
        let config = DebateConfig::default();
        
        Self {
            generator: GeneratorAgent::new(client.clone(), config.num_candidates),
            critic: Some(CriticAgent::new(client.clone())),
            tester: Some(TesterAgent::new(client.clone())),
            consensus: Consensus::new(),
            config,
        }
    }

    pub fn with_config(client: OllamaClient, config: DebateConfig) -> Self {
        Self {
            generator: GeneratorAgent::new(client.clone(), config.num_candidates),
            critic: Some(CriticAgent::new(client.clone())),
            tester: Some(TesterAgent::new(client.clone())),
            consensus: Consensus::new(),
            config,
        }
    }

    pub fn with_rag_service(mut self, rag_service: Arc<RagService>) -> Self {
        self.generator = self.generator.with_rag_service(rag_service.clone());
        if let Some(ref mut critic) = self.critic {
            *critic = critic.clone().with_rag_service(rag_service.clone());
        }
        if let Some(ref mut tester) = self.tester {
            *tester = tester.clone().with_rag_service(rag_service);
        }
        self
    }

    pub async fn debate(&self, task: &str) -> Result<DebateResult> {
        let mut solutions = self.generator.generate_candidates(task).await?;
        
        if solutions.is_empty() {
            return Ok(DebateResult {
                solution: None,
                all_solutions: vec![],
                iterations: 0,
                consensus_result: "No solutions generated".to_string(),
                success: false,
            });
        }

        let mut iteration = 0;
        let mut feedback = String::new();

        while iteration < self.config.max_iterations {
            iteration += 1;

            let critiques = if self.config.enable_critique {
                if let Some(ref critic) = self.critic {
                    critic.critique_batch(&solutions).await?
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            let test_results = if self.config.enable_testing {
                if let Some(ref tester) = self.tester {
                    tester.validate_batch(&solutions).await?
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            let consensus_result = self.consensus.resolve(&solutions, &critiques, &test_results);

            if consensus_result.consensus_score >= self.config.early_exit_threshold {
                return Ok(DebateResult {
                    solution: consensus_result.winning_solution,
                    all_solutions: solutions,
                    iterations: iteration,
                    consensus_result: consensus_result.reasoning,
                    success: true,
                });
            }

            if iteration < self.config.max_iterations {
                if let Some(ref critic) = self.critic {
                    feedback = critic.aggregate_feedback(&critiques).await?;
                }
                
                solutions = self
                    .generator
                    .generate_with_iteration(task, &feedback, iteration)
                    .await?;
            }
        }

        let final_critiques = if let Some(ref critic) = self.critic {
            critic.critique_batch(&solutions).await?
        } else {
            vec![]
        };

        let final_tests = if let Some(ref tester) = self.tester {
            tester.validate_batch(&solutions).await?
        } else {
            vec![]
        };

        let consensus_result = self.consensus.resolve(&solutions, &final_critiques, &final_tests);

        Ok(DebateResult {
            solution: consensus_result.winning_solution,
            all_solutions: solutions,
            iterations: iteration,
            consensus_result: consensus_result.reasoning,
            success: consensus_result.consensus_score > 0.5,
        })
    }

    pub async fn debate_simple(&self, task: &str) -> Result<CandidateSolution> {
        let result = self.debate(task).await?;
        
        Ok(result.solution.unwrap_or_else(|| {
            CandidateSolution::new(
                "1".to_string(),
                "No solution found via debate".to_string(),
            )
        }))
    }
}

impl Clone for DebateManager {
    fn clone(&self) -> Self {
        Self {
            generator: self.generator.clone(),
            critic: self.critic.clone(),
            tester: self.tester.clone(),
            consensus: self.consensus.clone(),
            config: self.config.clone(),
        }
    }
}

impl Clone for Consensus {
    fn clone(&self) -> Self {
        Self {
            critique_weight: self.critique_weight,
            test_weight: self.test_weight,
            confidence_weight: self.confidence_weight,
        }
    }
}
