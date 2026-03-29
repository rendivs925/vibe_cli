use crate::ollama_client::OllamaClient;
use shared::types::Result;

#[derive(Debug, Clone)]
pub struct RerankConfig {
    pub initial_top_k: usize,
    pub final_top_k: usize,
    pub batch_size: usize,
    pub min_relevance_score: f32,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            initial_top_k: 20,
            final_top_k: 5,
            batch_size: 5,
            min_relevance_score: 0.3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RerankedChunk {
    pub text: String,
    pub path: String,
    pub original_rank: usize,
    pub rerank_score: f32,
}

pub struct Reranker {
    client: OllamaClient,
    config: RerankConfig,
}

impl Reranker {
    pub fn new(client: OllamaClient) -> Self {
        Self::with_config(client, RerankConfig::default())
    }

    pub fn with_config(client: OllamaClient, config: RerankConfig) -> Self {
        Self { client, config }
    }

    pub fn config(&self) -> &RerankConfig {
        &self.config
    }

    pub async fn rerank(
        &self,
        query: &str,
        chunks: Vec<String>,
    ) -> Result<Vec<RerankedChunk>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        if chunks.len() <= self.config.final_top_k {
            return Ok(chunks
                .into_iter()
                .enumerate()
                .map(|(i, text)| RerankedChunk {
                    text: text.clone(),
                    path: extract_path_from_chunk(&text).unwrap_or_default(),
                    original_rank: i,
                    rerank_score: 1.0 - (i as f32 * 0.1),
                })
                .collect());
        }

        let initial_chunks: Vec<_> = chunks
            .into_iter()
            .take(self.config.initial_top_k)
            .collect();

        let mut scored_chunks = Vec::new();

        for (idx, chunk) in initial_chunks.iter().enumerate() {
            let score = self.compute_relevance_score(query, chunk).await?;
            scored_chunks.push(RerankedChunk {
                text: chunk.clone(),
                path: extract_path_from_chunk(chunk).unwrap_or_default(),
                original_rank: idx,
                rerank_score: score,
            });
        }

        scored_chunks.sort_by(|a, b| b.rerank_score.partial_cmp(&a.rerank_score).unwrap());

        Ok(scored_chunks
            .into_iter()
            .filter(|c| c.rerank_score >= self.config.min_relevance_score)
            .take(self.config.final_top_k)
            .collect())
    }

    async fn compute_relevance_score(&self, query: &str, chunk: &str) -> Result<f32> {
        let truncated_chunk = if chunk.chars().count() > 1500 {
            chunk.chars().take(1500).collect::<String>()
        } else {
            chunk.to_string()
        };

        let prompt = format!(
            r#"You are a relevance scorer. Rate how relevant the following chunk is to the query.
Return ONLY a number between 0.0 and 1.0, where:
- 1.0 = extremely relevant, directly answers the query
- 0.5 = somewhat relevant, contains related information
- 0.0 = not relevant at all

Query: {}

Chunk: {}

Relevance score:"#,
            query, truncated_chunk
        );

        let response = self.client.generate_response(&prompt).await?;
        let score = parse_score(&response);

        Ok(score)
    }

    pub async fn rerank_with_semantic_context(
        &self,
        query: &str,
        chunks: Vec<String>,
        semantic_scores: Vec<f32>,
    ) -> Result<Vec<RerankedChunk>> {
        if chunks.len() != semantic_scores.len() {
            return self.rerank(query, chunks).await;
        }

        let reranked = self.rerank(query, chunks).await?;

        let mut combined: Vec<RerankedChunk> = reranked
            .into_iter()
            .zip(semantic_scores.into_iter())
            .map(|(mut r, sem_score)| {
                r.rerank_score = (r.rerank_score * 0.6) + (sem_score * 0.4);
                r
            })
            .collect();

        combined.sort_by(|a, b| b.rerank_score.partial_cmp(&a.rerank_score).unwrap());

        Ok(combined)
    }
}

fn extract_path_from_chunk(chunk: &str) -> Option<String> {
    for line in chunk.lines().take(6) {
        if let Some(path) = line.strip_prefix("FILE: ") {
            return Some(path.trim().to_string());
        }
    }
    None
}

fn parse_score(response: &str) -> f32 {
    let trimmed = response.trim();
    
    if let Some(dot_pos) = trimmed.find('.') {
        let before = &trimmed[..dot_pos];
        let after = &trimmed[dot_pos + 1..];
        let before_num: f32 = before
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0.0);
        let after_num: f32 = after
            .chars()
            .take(2)
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0.0);
        let after_denom = 10.0_f32.powi(after_num.to_string().len() as i32);
        return (before_num + after_num / after_denom).clamp(0.0, 1.0);
    }

    let digits: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    
    digits.parse::<f32>().unwrap_or(0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_score() {
        assert_eq!(parse_score("0.85"), 0.85);
        assert_eq!(parse_score("0.9"), 0.9);
        assert_eq!(parse_score("1.0"), 1.0);
        assert_eq!(parse_score("The score is 0.75"), 0.75);
    }

    #[test]
    fn test_extract_path() {
        let chunk = "FILE: src/main.rs\nfn main() {}";
        assert_eq!(extract_path_from_chunk(chunk), Some("src/main.rs".to_string()));
    }
}
