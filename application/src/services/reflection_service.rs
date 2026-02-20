use infrastructure::ollama_client::OllamaClient;
use shared::types::Result;

pub struct ReflectionConfig {
    pub max_reflections: usize,
    pub confidence_threshold: f32,
    pub enable_citation_check: bool,
}

impl Default for ReflectionConfig {
    fn default() -> Self {
        Self {
            max_reflections: 3,
            confidence_threshold: 0.7,
            enable_citation_check: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReflectionResult {
    pub response: String,
    pub confidence: f32,
    pub citations_verified: bool,
    pub needs_refinement: bool,
    pub refinement_feedback: Option<String>,
}

pub struct ReflectionService {
    client: OllamaClient,
    config: ReflectionConfig,
}

impl ReflectionService {
    pub fn new(client: OllamaClient) -> Self {
        Self::with_config(client, ReflectionConfig::default())
    }

    pub fn with_config(client: OllamaClient, config: ReflectionConfig) -> Self {
        Self { client, config }
    }

    pub async fn reflect(&self, query: &str, response: &str) -> Result<ReflectionResult> {
        self.reflect_with_context(query, response, "").await
    }

    pub async fn reflect_with_context(
        &self,
        query: &str,
        response: &str,
        context: &str,
    ) -> Result<ReflectionResult> {
        let reflection_prompt = format!(
            r#"Analyze the following response for quality and accuracy.

Query: {}

Response: {}

{}

Instructions:
1. Evaluate if the response directly answers the query
2. Check if claims are supported by evidence
3. Assess confidence level (0.0 to 1.0)
4. Identify any uncertain claims
5. Verify citations if present

Return JSON:
{{
    "confidence": 0.85,
    "needs_refinement": false,
    "refinement_feedback": "feedback if needed",
    "citations_verified": true
}}"#,
            query,
            response,
            if context.is_empty() {
                "Context: None".to_string()
            } else {
                format!("Context:\n{}", context)
            }
        );

        let reflection_response = self.client.generate_response(&reflection_prompt).await?;
        self.parse_reflection_result(&reflection_response, response)
    }

    fn parse_reflection_result(
        &self,
        reflection: &str,
        original_response: &str,
    ) -> Result<ReflectionResult> {
        let trimmed = reflection.trim();
        
        #[derive(serde::Deserialize)]
        struct RawReflection {
            confidence: f32,
            needs_refinement: bool,
            refinement_feedback: Option<String>,
            citations_verified: bool,
        }

        if let Ok(raw) = serde_json::from_str::<RawReflection>(trimmed) {
            return Ok(ReflectionResult {
                response: original_response.to_string(),
                confidence: raw.confidence.clamp(0.0, 1.0),
                citations_verified: raw.citations_verified,
                needs_refinement: raw.needs_refinement,
                refinement_feedback: raw.refinement_feedback,
            });
        }

        if let Some(json_start) = trimmed.find('{') {
            if let Some(json_end) = trimmed.rfind('}') {
                let json_str = &trimmed[json_start..=json_end];
                if let Ok(raw) = serde_json::from_str::<RawReflection>(json_str) {
                    return Ok(ReflectionResult {
                        response: original_response.to_string(),
                        confidence: raw.confidence.clamp(0.0, 1.0),
                        citations_verified: raw.citations_verified,
                        needs_refinement: raw.needs_refinement,
                        refinement_feedback: raw.refinement_feedback,
                    });
                }
            }
        }

        let confidence = if reflection.to_lowercase().contains("high") {
            0.8
        } else if reflection.to_lowercase().contains("medium") {
            0.5
        } else if reflection.to_lowercase().contains("low") {
            0.3
        } else {
            0.5
        };

        let needs_refinement = reflection.to_lowercase().contains("refine")
            || reflection.to_lowercase().contains("improve")
            || reflection.to_lowercase().contains("uncertain");

        Ok(ReflectionResult {
            response: original_response.to_string(),
            confidence,
            citations_verified: !reflection.to_lowercase().contains("citation missing"),
            needs_refinement,
            refinement_feedback: if needs_refinement {
                Some("Response needs refinement based on reflection".to_string())
            } else {
                None
            },
        })
    }

    pub async fn refine_response(
        &self,
        query: &str,
        response: &str,
        feedback: &str,
    ) -> Result<String> {
        let refine_prompt = format!(
            r#"Refine the following response based on the feedback.

Original Query: {}

Current Response: {}

Feedback: {}

Provide an improved response that addresses the feedback while maintaining accuracy."#,
            query, response, feedback
        );

        self.client.generate_response(&refine_prompt).await
    }

    pub async fn reflective_loop(
        &self,
        query: &str,
        initial_response: &str,
        context: &str,
    ) -> Result<ReflectionResult> {
        let mut current_response = initial_response.to_string();
        let mut current_confidence = 0.0;
        
        for iteration in 0..self.config.max_reflections {
            let result = self
                .reflect_with_context(query, &current_response, context)
                .await?;

            if !result.needs_refinement || result.confidence >= self.config.confidence_threshold {
                return Ok(result);
            }

            if let Some(feedback) = &result.refinement_feedback {
                current_response = self
                    .refine_response(query, &current_response, feedback)
                    .await?;
                current_confidence = result.confidence;
            } else {
                break;
            }

            if iteration == self.config.max_reflections - 1 {
                return Ok(ReflectionResult {
                    response: current_response,
                    confidence: current_confidence,
                    citations_verified: result.citations_verified,
                    needs_refinement: true,
                    refinement_feedback: Some("Max reflections reached".to_string()),
                });
            }
        }

        Ok(ReflectionResult {
            response: current_response,
            confidence: current_confidence,
            citations_verified: true,
            needs_refinement: false,
            refinement_feedback: None,
        })
    }
}

impl Clone for ReflectionService {
    fn clone(&self) -> Self {
        Self {
            client: OllamaClient::new().expect("Failed to create Ollama client"),
            config: self.config.clone(),
        }
    }
}

impl Clone for ReflectionConfig {
    fn clone(&self) -> Self {
        Self {
            max_reflections: self.max_reflections,
            confidence_threshold: self.confidence_threshold,
            enable_citation_check: self.enable_citation_check,
        }
    }
}
