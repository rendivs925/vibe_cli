use super::agent::{Agent, AgentConfig, AgentRole};
use super::generator_agent::CandidateSolution;
use crate::services::rag_service::RagService;
use infrastructure::ollama_client::OllamaClient;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub solution_id: String,
    pub passed: bool,
    pub tests: Vec<TestCase>,
    pub summary: String,
    pub safety_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub passed: bool,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCriteria {
    pub must_have: Vec<String>,
    pub should_avoid: Vec<String>,
    pub safety_checks: Vec<String>,
}

impl Default for ValidationCriteria {
    fn default() -> Self {
        Self {
            must_have: vec![],
            should_avoid: vec!["dangerous commands".to_string()],
            safety_checks: vec![
                "no file deletion".to_string(),
                "no system damage".to_string(),
            ],
        }
    }
}

pub struct TesterAgent {
    agent: Agent,
    criteria: ValidationCriteria,
}

impl TesterAgent {
    pub fn new(client: OllamaClient) -> Self {
        let config = AgentConfig {
            name: "Tester".to_string(),
            role: AgentRole::Tester,
            model: "qwen2.5".to_string(),
            temperature: 0.3,
            max_tokens: 4096,
        };

        Self {
            agent: Agent::new(config, client),
            criteria: ValidationCriteria::default(),
        }
    }

    pub fn with_rag_service(mut self, rag_service: Arc<RagService>) -> Self {
        self.agent = self.agent.with_rag_service(rag_service);
        self
    }

    pub fn with_criteria(mut self, criteria: ValidationCriteria) -> Self {
        self.criteria = criteria;
        self
    }

    pub async fn validate(&self, solution: &CandidateSolution) -> Result<TestResult> {
        let prompt = format!(
            r#"Validate the following solution against the criteria.

Solution ID: {}
Content: {}

Validation Criteria:
- Must have: {}
- Should avoid: {}
- Safety checks: {}

Provide validation results in JSON format:
{{
  "solution_id": "1",
  "passed": true,
  "tests": [
    {{"name": "check1", "passed": true, "details": "description"}}
  ],
  "summary": "overall summary",
  "safety_issues": ["issue1"]
}}"#,
            solution.id,
            solution.content,
            self.criteria.must_have.join(", "),
            self.criteria.should_avoid.join(", "),
            self.criteria.safety_checks.join(", ")
        );

        let response = self.agent.generate(&prompt).await?;
        self.parse_test_result(&response, &solution.id)
    }

    pub async fn validate_batch(&self, solutions: &[CandidateSolution]) -> Result<Vec<TestResult>> {
        let mut results = Vec::new();
        
        for solution in solutions {
            match self.validate(solution).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(TestResult {
                        solution_id: solution.id.clone(),
                        passed: false,
                        tests: vec![],
                        summary: format!("Validation failed: {}", e),
                        safety_issues: vec![],
                    });
                }
            }
        }

        Ok(results)
    }

    fn parse_test_result(&self, response: &str, solution_id: &str) -> Result<TestResult> {
        let trimmed = response.trim();
        
        if let Ok(result) = serde_json::from_str::<TestResult>(trimmed) {
            return Ok(result);
        }

        if let Some(json_start) = trimmed.find('{') {
            if let Some(json_end) = trimmed.rfind('}') {
                let json_str = &trimmed[json_start..=json_end];
                if let Ok(result) = serde_json::from_str::<TestResult>(json_str) {
                    return Ok(result);
                }
            }
        }

        let passed = response.to_lowercase().contains("passed") 
            && !response.to_lowercase().contains("failed");

        Ok(TestResult {
            solution_id: solution_id.to_string(),
            passed,
            tests: vec![],
            summary: response.to_string(),
            safety_issues: vec![],
        })
    }

    pub async fn aggregate_results(&self, results: &[TestResult]) -> Result<String> {
        let passed_count = results.iter().filter(|r| r.passed).count();
        let total = results.len();
        
        let mut safety_concerns: Vec<String> = results
            .iter()
            .flat_map(|r| r.safety_issues.clone())
            .collect();
        
        safety_concerns.sort();
        safety_concerns.dedup();

        let summary = format!(
            "Test Results: {}/{} passed\nSafety Concerns: {}",
            passed_count,
            total,
            if safety_concerns.is_empty() {
                "None".to_string()
            } else {
                safety_concerns.join(", ")
            }
        );

        Ok(summary)
    }
}

impl Clone for TesterAgent {
    fn clone(&self) -> Self {
        Self {
            agent: self.agent.clone(),
            criteria: self.criteria.clone(),
        }
    }
}
