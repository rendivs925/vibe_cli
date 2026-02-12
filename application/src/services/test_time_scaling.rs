use infrastructure::{
    config::Config,
    ollama_client::OllamaClient,
};
use shared::types::Result;

#[derive(Clone, Debug)]
pub struct ScalingConfig {
    pub method: ScalingMethod,
    pub num_samples: usize,
    pub comparisons_per_pair: usize,
    pub opponents_per_candidate: usize,
    pub early_stopping: bool,
    pub confidence_threshold: f64,
}

impl Default for ScalingConfig {
    fn default() -> Self {
        Self {
            method: ScalingMethod::None,
            num_samples: 6,
            comparisons_per_pair: 3,
            opponents_per_candidate: 5,
            early_stopping: false,
            confidence_threshold: 0.9,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScalingMethod {
    None,
    Knockout,
    League,
}

#[derive(Clone, Debug)]
pub struct CandidateCommand {
    pub command: String,
    pub win_count: usize,
    pub loss_count: usize,
    pub total_comparisons: usize,
}

impl CandidateCommand {
    pub fn new(command: String) -> Self {
        Self {
            command,
            win_count: 0,
            loss_count: 0,
            total_comparisons: 0,
        }
    }

    pub fn win_rate(&self) -> f64 {
        if self.total_comparisons == 0 {
            return 0.5;
        }
        self.win_count as f64 / self.total_comparisons as f64
    }
}

#[allow(dead_code)]
pub struct TestTimeComputeService {
    client: OllamaClient,
    config: Config,
}

impl TestTimeComputeService {
    pub fn new(client: OllamaClient, config: Config) -> Self {
        Self { client, config }
    }

    pub async fn select_best_command(
        &self,
        user_query: &str,
        scaling_config: &ScalingConfig,
    ) -> Result<Option<String>> {
        if scaling_config.method == ScalingMethod::None {
            return Ok(None);
        }

        let candidates = self
            .generate_candidates(user_query, scaling_config.num_samples)
            .await?;

        if candidates.is_empty() {
            return Ok(None);
        }

        if candidates.len() == 1 {
            return Ok(Some(candidates[0].command.clone()));
        }

        let selected = match scaling_config.method {
            ScalingMethod::Knockout => {
                self.run_knockout_tournament(&candidates, scaling_config)
                    .await?
            }
            ScalingMethod::League => {
                self.run_league_competition(&candidates, scaling_config)
                    .await?
            }
            ScalingMethod::None => candidates[0].command.clone(),
        };

        Ok(Some(selected))
    }

    async fn generate_candidates(
        &self,
        user_query: &str,
        num_samples: usize,
    ) -> Result<Vec<CandidateCommand>> {
        let mut candidates = Vec::new();

        for _ in 0..num_samples {
            if let Some(cmd) = self.generate_single_candidate(user_query).await? {
                if !cmd.is_empty() {
                    candidates.push(CandidateCommand::new(cmd));
                }
            }
        }

        Ok(candidates)
    }

    async fn generate_single_candidate(
        &self,
        user_query: &str,
    ) -> Result<Option<String>> {
        let prompt = format!(
            r#"Generate a single shell command to accomplish the following task:

Task: {}

Requirements:
- Output ONLY the command, no explanation
- The command should be correct and complete
- Use appropriate flags for the target platform (Linux)
"#,
            user_query
        );

        let response = self.client.generate_response(&prompt).await?;
        let cleaned = response.trim().to_string();
        
        if cleaned.is_empty() {
            return Ok(None);
        }

        Ok(Some(cleaned))
    }

    async fn run_knockout_tournament(
        &self,
        candidates: &[CandidateCommand],
        config: &ScalingConfig,
    ) -> Result<String> {
        let mut participants: Vec<CandidateCommand> = candidates.to_vec();

        while participants.len() > 1 {
            let mut next_round = Vec::new();

            let mut i = 0;
            while i < participants.len() {
                if i + 1 >= participants.len() {
                    next_round.push(participants.remove(i));
                    break;
                }

                let (candidate_a, candidate_b) = (
                    participants[i].command.clone(),
                    participants[i + 1].command.clone(),
                );

                let winner_idx = self
                    .compare_pairs(
                        &candidate_a,
                        &candidate_b,
                        config.comparisons_per_pair,
                    )
                    .await;

                if winner_idx == 0 {
                    next_round.push(participants.remove(i));
                    participants.remove(i);
                } else {
                    participants.remove(i);
                    next_round.push(participants.remove(i));
                }
            }

            participants = next_round;
        }

        Ok(participants.pop().map(|c| c.command).unwrap_or_else(|| {
            candidates.first().map(|c| c.command.clone()).unwrap_or_default()
        }))
    }

    async fn run_league_competition(
        &self,
        candidates: &[CandidateCommand],
        config: &ScalingConfig,
    ) -> Result<String> {
        let mut league: Vec<CandidateCommand> = candidates.to_vec();
        let num_opponents = config.opponents_per_candidate.min(candidates.len().saturating_sub(1));

        let commands: Vec<String> = league.iter().map(|c| c.command.clone()).collect();

        for (idx, candidate) in league.iter_mut().enumerate() {
            for _ in 0..num_opponents {
                let opponent_idx = rand_index(commands.len());
                if opponent_idx == idx {
                    continue;
                }

                let opponent_cmd = &commands[opponent_idx];

                let winner = self
                    .compare_two(&candidate.command, opponent_cmd)
                    .await;

                if winner == 0 {
                    candidate.win_count += 1;
                } else {
                    candidate.loss_count += 1;
                }
                candidate.total_comparisons += 1;
            }
        }

        league.sort_by(|a, b| b.win_rate().partial_cmp(&a.win_rate()).unwrap());

        Ok(league
            .first()
            .map(|c| c.command.clone())
            .unwrap_or_else(|| candidates.first().map(|c| c.command.clone()).unwrap_or_default()))
    }

    async fn compare_pairs(
        &self,
        candidate_a: &str,
        candidate_b: &str,
        num_comparisons: usize,
    ) -> usize {
        let mut wins_for_a = 0;

        for _ in 0..num_comparisons {
            let winner = self.compare_two(candidate_a, candidate_b).await;
            if winner == 0 {
                wins_for_a += 1;
            }
        }

        if wins_for_a > (num_comparisons / 2) {
            0
        } else {
            1
        }
    }

    async fn compare_two(&self, candidate_a: &str, candidate_b: &str) -> usize {
        let prompt = format!(
            r#"Compare these two shell commands and determine which one is better for the task.

Command A: {}
Command B: {}

Consider:
1. Correctness - does the command do what it claims?
2. Safety - is it safe to run?
3. Efficiency - is it the most direct solution?
4. Compatibility - does it work on Linux?

Respond with ONLY "A" or "B" (no explanation):"#,
            candidate_a, candidate_b
        );

        let response = self.client.generate_response(&prompt).await.unwrap_or_default();
        let response_lower = response.to_lowercase().trim().to_string();

        if response_lower.starts_with('a') {
            0
        } else {
            1
        }
    }
}

fn rand_index(max: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as usize;
    seed % max
}
