use super::critic_agent::Critique;
use super::generator_agent::CandidateSolution;
use super::tester_agent::TestResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub winning_solution: Option<CandidateSolution>,
    pub scores: HashMap<String, f32>,
    pub consensus_score: f32,
    pub reasoning: String,
}

pub struct Consensus {
    pub critique_weight: f32,
    pub test_weight: f32,
    pub confidence_weight: f32,
}

impl Consensus {
    pub fn new() -> Self {
        Self {
            critique_weight: 0.4,
            test_weight: 0.4,
            confidence_weight: 0.2,
        }
    }

    pub fn with_weights(critique_weight: f32, test_weight: f32, confidence_weight: f32) -> Self {
        Self {
            critique_weight,
            test_weight,
            confidence_weight,
        }
    }

    pub fn resolve(
        &self,
        solutions: &[CandidateSolution],
        critiques: &[Critique],
        test_results: &[TestResult],
    ) -> ConsensusResult {
        let mut scores: HashMap<String, f32> = HashMap::new();

        for solution in solutions {
            let critique_score = critiques
                .iter()
                .find(|c| c.solution_id == solution.id)
                .map(|c| c.score)
                .unwrap_or(0.5);

            let test_score = test_results
                .iter()
                .find(|t| t.solution_id == solution.id)
                .map(|t| if t.passed { 1.0 } else { 0.0 })
                .unwrap_or(0.5);

            let combined_score = (critique_score * self.critique_weight)
                + (test_score * self.test_weight)
                + (solution.confidence * self.confidence_weight);

            scores.insert(solution.id.clone(), combined_score);
        }

        let mut sorted: Vec<_> = scores.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());

        let winning_id = sorted.first().map(|(id, _)| (*id).clone());
        let winning_solution = if let Some(ref wid) = winning_id {
            solutions.iter().find(|s| s.id == *wid).cloned()
        } else {
            None
        };

        let consensus_score = sorted.first().map(|(_, score)| **score).unwrap_or(0.0);

        let reasoning = if let Some((id, score)) = sorted.first() {
            format!(
                "Solution {} selected with score {:.2}. Critiques weighted at {:.0}%, tests at {:.0}%.",
                id,
                score,
                self.critique_weight * 100.0,
                self.test_weight * 100.0
            )
        } else {
            "No consensus reached".to_string()
        };

        ConsensusResult {
            winning_solution,
            scores: scores.clone(),
            consensus_score,
            reasoning,
        }
    }

    pub fn vote(&self, solutions: &[CandidateSolution]) -> Option<CandidateSolution> {
        if solutions.is_empty() {
            return None;
        }

        let mut votes: HashMap<String, usize> = HashMap::new();

        for solution in solutions {
            *votes.entry(solution.id.clone()).or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = votes.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        solutions
            .iter()
            .find(|s| s.id == *sorted.first().unwrap().0)
            .cloned()
    }
}

impl Default for Consensus {
    fn default() -> Self {
        Self::new()
    }
}
