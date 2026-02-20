use infrastructure::ollama_client::OllamaClient;
use shared::types::Result;
use std::collections::HashMap;

pub struct ConfidenceScorer {
    client: OllamaClient,
    knowledge_weights: HashMap<String, f32>,
}

impl ConfidenceScorer {
    pub fn new(client: OllamaClient) -> Self {
        let mut knowledge_weights = HashMap::new();
        knowledge_weights.insert("exact_match".to_string(), 0.4);
        knowledge_weights.insert("partial_match".to_string(), 0.3);
        knowledge_weights.insert("inference".to_string(), 0.2);
        knowledge_weights.insert("uncertain".to_string(), 0.1);

        Self {
            client,
            knowledge_weights,
        }
    }

    pub async fn score(&self, query: &str, response: &str) -> Result<ConfidenceScore> {
        self.score_with_context(query, response, &[]).await
    }

    pub async fn score_with_context(
        &self,
        query: &str,
        response: &str,
        context_chunks: &[String],
    ) -> Result<ConfidenceScore> {
        let context_summary = if context_chunks.is_empty() {
            "No additional context provided".to_string()
        } else {
            format!(
                "Context chunks provided: {}",
                context_chunks.len()
            )
        };

        let scoring_prompt = format!(
            r#"Score the confidence of this response on a scale of 0.0 to 1.0.

Query: {}

Response: {}

{}

Evaluation Criteria:
1. Does the response directly answer the query? (0-0.3)
2. Is the response well-supported by evidence? (0-0.3)
3. Are there any uncertainties or caveats? (0-0.2)
4. How confident is the tone? (0-0.2)

Return JSON:
{{
    "overall_score": 0.85,
    "component_scores": {{
        "answer_accuracy": 0.9,
        "evidence_support": 0.8,
        "uncertainty": 0.1,
        "tone_confidence": 0.85
    }},
    "reasoning": "brief explanation"
}}"#,
            query, response, context_summary
        );

        let scoring_response = self.client.generate_response(&scoring_prompt).await?;
        self.parse_score(&scoring_response)
    }

    fn parse_score(&self, response: &str) -> Result<ConfidenceScore> {
        let trimmed = response.trim();
        
        #[derive(serde::Deserialize)]
        struct RawScore {
            overall_score: f32,
            component_scores: ComponentScores,
            reasoning: String,
        }

        #[derive(serde::Deserialize)]
        struct ComponentScores {
            answer_accuracy: f32,
            evidence_support: f32,
            uncertainty: f32,
            tone_confidence: f32,
        }

        if let Ok(raw) = serde_json::from_str::<RawScore>(trimmed) {
            return Ok(ConfidenceScore {
                overall: raw.overall_score.clamp(0.0, 1.0),
                answer_accuracy: raw.component_scores.answer_accuracy.clamp(0.0, 1.0),
                evidence_support: raw.component_scores.evidence_support.clamp(0.0, 1.0),
                uncertainty: raw.component_scores.uncertainty.clamp(0.0, 1.0),
                tone_confidence: raw.component_scores.tone_confidence.clamp(0.0, 1.0),
                reasoning: raw.reasoning,
            });
        }

        if let Some(json_start) = trimmed.find('{') {
            if let Some(json_end) = trimmed.rfind('}') {
                let json_str = &trimmed[json_start..=json_end];
                if let Ok(raw) = serde_json::from_str::<RawScore>(json_str) {
                    return Ok(ConfidenceScore {
                        overall: raw.overall_score.clamp(0.0, 1.0),
                        answer_accuracy: raw.component_scores.answer_accuracy.clamp(0.0, 1.0),
                        evidence_support: raw.component_scores.evidence_support.clamp(0.0, 1.0),
                        uncertainty: raw.component_scores.uncertainty.clamp(0.0, 1.0),
                        tone_confidence: raw.component_scores.tone_confidence.clamp(0.0, 1.0),
                        reasoning: raw.reasoning,
                    });
                }
            }
        }

        let overall = if response.to_lowercase().contains("high") {
            0.8
        } else if response.to_lowercase().contains("medium") {
            0.5
        } else if response.to_lowercase().contains("low") {
            0.3
        } else {
            0.5
        };

        Ok(ConfidenceScore {
            overall,
            answer_accuracy: overall,
            evidence_support: overall,
            uncertainty: 1.0 - overall,
            tone_confidence: overall,
            reasoning: response.to_string(),
        })
    }

    pub fn combine_scores(&self, scores: &[ConfidenceScore]) -> ConfidenceScore {
        if scores.is_empty() {
            return ConfidenceScore {
                overall: 0.0,
                answer_accuracy: 0.0,
                evidence_support: 0.0,
                uncertainty: 1.0,
                tone_confidence: 0.0,
                reasoning: "No scores to combine".to_string(),
            };
        }

        let n = scores.len() as f32;
        
        ConfidenceScore {
            overall: scores.iter().map(|s| s.overall).sum::<f32>() / n,
            answer_accuracy: scores.iter().map(|s| s.answer_accuracy).sum::<f32>() / n,
            evidence_support: scores.iter().map(|s| s.evidence_support).sum::<f32>() / n,
            uncertainty: scores.iter().map(|s| s.uncertainty).sum::<f32>() / n,
            tone_confidence: scores.iter().map(|s| s.tone_confidence).sum::<f32>() / n,
            reasoning: format!("Combined {} scores", scores.len()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfidenceScore {
    pub overall: f32,
    pub answer_accuracy: f32,
    pub evidence_support: f32,
    pub uncertainty: f32,
    pub tone_confidence: f32,
    pub reasoning: String,
}

impl ConfidenceScore {
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.overall >= threshold
    }

    pub fn needs_verification(&self, threshold: f32) -> bool {
        self.overall < threshold || self.uncertainty > (1.0 - threshold)
    }
}

impl Clone for ConfidenceScorer {
    fn clone(&self) -> Self {
        Self {
            client: OllamaClient::new().expect("Failed to create Ollama client"),
            knowledge_weights: self.knowledge_weights.clone(),
        }
    }
}
