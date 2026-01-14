use crate::ollama_client::OllamaClient;
use serde::Deserialize;
use shared::types::Result;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// LLM-powered input classification system
pub struct InputClassifier {
    ollama_client: Arc<OllamaClient>,
    cache: RwLock<HashMap<String, ClassificationResult>>,
    cache_ttl: Duration,
    heuristic_classifier: HeuristicClassifier,
}

#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub input_type: InputType,
    pub confidence: f32,
    pub reasoning: String,
    pub suggested_action: String,
    pub metadata: HashMap<String, String>,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputType {
    Command,       // Shell command request
    Question,      // Question about code/project
    Conversation,  // General conversation
    CodeSnippet,   // Code to analyze/explain
    FileOperation, // File operations (read, write, etc.)
    SystemQuery,   // System information queries
    Ambiguous,     // Cannot determine with confidence
}

#[derive(Debug)]
struct HeuristicClassifier {
    command_keywords: Vec<String>,
    question_keywords: Vec<String>,
    code_patterns: Vec<String>,
}

impl InputClassifier {
    /// Create new input classifier
    pub fn new(ollama_client: Arc<OllamaClient>) -> Self {
        Self {
            ollama_client,
            cache: RwLock::new(HashMap::new()),
            cache_ttl: Duration::from_secs(3600), // 1 hour TTL
            heuristic_classifier: HeuristicClassifier::new(),
        }
    }

    /// Classify input with caching and fallback
    pub async fn classify_input(&self, input: &str) -> Result<ClassificationResult> {
        // Check cache first
        if let Some(cached) = self.get_cached_result(input).await {
            return Ok(cached);
        }

        // Try LLM classification first
        match self.llm_classify(input).await {
            Ok(result) => {
                if result.confidence >= 0.8 {
                    self.cache_result(input.to_string(), result.clone()).await;
                    return Ok(result);
                }
            }
            Err(e) => {
                eprintln!(
                    "LLM classification failed: {}, falling back to heuristics",
                    e
                );
            }
        }

        // Fallback to heuristic classification
        let result = self.heuristic_classify(input).await;
        self.cache_result(input.to_string(), result.clone()).await;
        Ok(result)
    }

    /// LLM-based classification
    async fn llm_classify(&self, input: &str) -> Result<ClassificationResult> {
        let prompt = format!(
            "Classify the following user input into one of these categories:
- Command: Shell command or system operation request
- Question: Question about code, project, or programming
- Conversation: General chat or discussion
- CodeSnippet: Code that needs analysis or explanation
- FileOperation: File read/write/create operations
- SystemQuery: System information or status queries

Input: \"{}\"

Respond with JSON in this format:
{{
  \"category\": \"Command|Question|Conversation|CodeSnippet|FileOperation|SystemQuery\",
  \"confidence\": 0.0-1.0,
  \"reasoning\": \"brief explanation\",
  \"suggested_action\": \"what the system should do\"
}}",
            input
        );

        let response = self.ollama_client.generate_response(&prompt).await?;

        // Parse JSON response
        self.parse_llm_response(&response, input)
    }

    /// Parse LLM response into ClassificationResult
    fn parse_llm_response(
        &self,
        response: &str,
        _original_input: &str,
    ) -> Result<ClassificationResult> {
        // Extract JSON from response (LLMs might add extra text)
        let json_start = response.find('{').unwrap_or(0);
        let json_end = response.rfind('}').unwrap_or(response.len());
        let json_str = &response[json_start..=json_end];

        #[derive(Deserialize)]
        struct LlmResponse {
            category: String,
            confidence: f32,
            reasoning: String,
            suggested_action: String,
        }

        let parsed: LlmResponse = serde_json::from_str(json_str)?;

        let input_type = match parsed.category.as_str() {
            "Command" => InputType::Command,
            "Question" => InputType::Question,
            "Conversation" => InputType::Conversation,
            "CodeSnippet" => InputType::CodeSnippet,
            "FileOperation" => InputType::FileOperation,
            "SystemQuery" => InputType::SystemQuery,
            _ => InputType::Ambiguous,
        };

        let mut metadata = HashMap::new();
        metadata.insert("classification_method".to_string(), "llm".to_string());
        metadata.insert("raw_response".to_string(), response.to_string());

        Ok(ClassificationResult {
            input_type,
            confidence: parsed.confidence,
            reasoning: parsed.reasoning,
            suggested_action: parsed.suggested_action,
            metadata,
            timestamp: Instant::now(),
        })
    }

    /// Heuristic-based classification fallback
    async fn heuristic_classify(&self, input: &str) -> ClassificationResult {
        let (input_type, confidence, reasoning) = self.heuristic_classifier.classify(input);

        let suggested_action = match input_type {
            InputType::Command => {
                "Execute the requested shell command with safety checks".to_string()
            }
            InputType::Question => "Search codebase and provide relevant information".to_string(),
            InputType::Conversation => "Engage in conversation and provide assistance".to_string(),
            InputType::CodeSnippet => "Analyze and explain the provided code".to_string(),
            InputType::FileOperation => "Perform the requested file operation safely".to_string(),
            InputType::SystemQuery => "Query system information and provide status".to_string(),
            InputType::Ambiguous => "Ask user for clarification on their request".to_string(),
        };

        let mut metadata = HashMap::new();
        metadata.insert("classification_method".to_string(), "heuristic".to_string());

        ClassificationResult {
            input_type,
            confidence,
            reasoning,
            suggested_action,
            metadata,
            timestamp: Instant::now(),
        }
    }

    /// Get cached result if valid
    async fn get_cached_result(&self, input: &str) -> Option<ClassificationResult> {
        let cache = self.cache.read().await;
        cache
            .get(input)
            .filter(|result| result.timestamp.elapsed() < self.cache_ttl)
            .cloned()
    }

    /// Cache classification result
    async fn cache_result(&self, input: String, result: ClassificationResult) {
        let mut cache = self.cache.write().await;
        cache.insert(input, result);
    }

    /// Clear expired cache entries
    pub async fn cleanup_cache(&self) {
        let mut cache = self.cache.write().await;
        let now = Instant::now();

        cache.retain(|_, result| now.duration_since(result.timestamp) < self.cache_ttl);
    }

    /// Get classification statistics
    pub async fn get_stats(&self) -> HashMap<String, String> {
        let cache = self.cache.read().await;
        let mut stats = HashMap::new();

        let total_classifications = cache.len();
        stats.insert(
            "cached_classifications".to_string(),
            total_classifications.to_string(),
        );

        let mut method_counts = HashMap::new();
        let mut type_counts = HashMap::new();

        for result in cache.values() {
            let method = result
                .metadata
                .get("classification_method")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            *method_counts.entry(method).or_insert(0) += 1;

            let type_str = format!("{:?}", result.input_type);
            *type_counts.entry(type_str).or_insert(0) += 1;
        }

        for (method, count) in method_counts {
            stats.insert(format!("method_{}", method), count.to_string());
        }

        for (type_str, count) in type_counts {
            stats.insert(format!("type_{}", type_str), count.to_string());
        }

        stats
    }

    /// Retrain classifier with user corrections
    pub async fn learn_from_correction(&self, input: &str, correct_type: InputType) -> Result<()> {
        // In a real implementation, this would update a learning model
        // For now, just update the cache with the correction
        let mut corrected_result = self.heuristic_classify(input).await;
        corrected_result.input_type = correct_type;
        corrected_result.confidence = 1.0;
        corrected_result.reasoning = "User corrected classification".to_string();

        self.cache_result(input.to_string(), corrected_result).await;
        Ok(())
    }

    /// Batch classify multiple inputs
    pub async fn batch_classify(&self, inputs: Vec<String>) -> Result<Vec<ClassificationResult>> {
        let mut results = Vec::new();

        for input in inputs {
            let result = self.classify_input(&input).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Get confidence threshold for different input types
    pub fn get_confidence_threshold(&self, input_type: &InputType) -> f32 {
        match input_type {
            InputType::Command => 0.9,       // High confidence needed for commands
            InputType::FileOperation => 0.9, // High confidence for file ops
            InputType::Question => 0.7,      // Medium confidence for questions
            InputType::CodeSnippet => 0.8,   // High confidence for code
            InputType::SystemQuery => 0.8,   // High confidence for system queries
            InputType::Conversation => 0.6,  // Lower threshold for conversation
            InputType::Ambiguous => 0.0,     // Always ambiguous
        }
    }

    /// Export classification data for analysis
    pub async fn export_classification_data(&self) -> Vec<serde_json::Value> {
        let cache = self.cache.read().await;
        let mut data = Vec::new();

        for (input, result) in cache.iter() {
            // Create JSON value manually to avoid serde issues with Instant
            let record = serde_json::json!({
                "input": input,
                "input_type": format!("{:?}", result.input_type),
                "confidence": result.confidence,
                "reasoning": result.reasoning,
                "suggested_action": result.suggested_action,
                "classification_method": result.metadata.get("classification_method"),
                "timestamp_seconds": result.timestamp.elapsed().as_secs(),
            });

            data.push(record);
        }

        data
    }
}

impl HeuristicClassifier {
    /// Create new heuristic classifier
    fn new() -> Self {
        Self {
            command_keywords: vec![
                "run", "execute", "start", "stop", "restart", "kill", "ps", "top", "ls", "cd",
                "mkdir", "rmdir", "cp", "mv", "chmod", "chown", "grep", "find", "cat", "echo",
                "sudo", "apt", "yum", "brew", "npm", "cargo", "git", "docker", "kubectl",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            question_keywords: vec![
                "what", "how", "why", "when", "where", "which", "who", "can", "does", "is", "are",
                "should", "would", "could", "explain", "help", "tell", "show", "find",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            code_patterns: vec![
                r"fn\s+\w+\s*\(".to_string(),
                r"def\s+\w+\s*\(".to_string(),
                r"class\s+\w+".to_string(),
                r"function\s+\w+\s*\(".to_string(),
                r"import\s+".to_string(),
                r"from\s+\w+".to_string(),
                r"const\s+\w+\s*=".to_string(),
                r"let\s+\w+\s*=".to_string(),
                r"var\s+\w+\s*=".to_string(),
            ],
        }
    }

    /// Classify input using heuristics
    fn classify(&self, input: &str) -> (InputType, f32, String) {
        let input_lower = input.to_lowercase();
        let input_words: Vec<&str> = input.split_whitespace().collect();

        // Check for code patterns first (high confidence)
        for pattern in &self.code_patterns {
            if regex::Regex::new(pattern).unwrap().is_match(input) {
                return (
                    InputType::CodeSnippet,
                    0.95,
                    "Contains code syntax patterns".to_string(),
                );
            }
        }

        // Check for command keywords
        let command_score = self.calculate_keyword_score(&input_words, &self.command_keywords);
        let question_score = self.calculate_keyword_score(&input_words, &self.question_keywords);

        // Check for command-like patterns
        let has_command_patterns = input.contains('$')
            || input.contains('|')
            || input.contains('>')
            || input.contains('<')
            || input.starts_with("run ")
            || input.starts_with("execute ");

        if command_score > 0.3 || has_command_patterns {
            return (
                InputType::Command,
                (command_score * 0.8 + if has_command_patterns { 0.2 } else { 0.0 }).min(0.9),
                format!("Command keywords detected (score: {:.2})", command_score),
            );
        }

        // Check for questions
        if question_score > 0.2 || input.ends_with('?') {
            return (
                InputType::Question,
                (question_score * 0.8 + if input.ends_with('?') { 0.2 } else { 0.0 }).min(0.85),
                format!("Question patterns detected (score: {:.2})", question_score),
            );
        }

        // Check for file operations
        if input_lower.contains("file")
            || input_lower.contains("read")
            || input_lower.contains("write")
            || input_lower.contains("create")
            || input_lower.contains("delete")
        {
            return (
                InputType::FileOperation,
                0.7,
                "File operation keywords detected".to_string(),
            );
        }

        // Check for system queries
        if input_lower.contains("status")
            || input_lower.contains("info")
            || input_lower.contains("version")
            || input_lower.contains("memory")
            || input_lower.contains("cpu")
        {
            return (
                InputType::SystemQuery,
                0.75,
                "System information query detected".to_string(),
            );
        }

        // Default to conversation
        (
            InputType::Conversation,
            0.6,
            "General conversational input".to_string(),
        )
    }

    /// Calculate keyword matching score
    fn calculate_keyword_score(&self, input_words: &[&str], keywords: &[String]) -> f32 {
        let mut matches = 0;
        let mut total_keywords = 0;

        for keyword in keywords {
            total_keywords += 1;
            if input_words
                .iter()
                .any(|word| word.to_lowercase().contains(&keyword.to_lowercase()))
            {
                matches += 1;
            }
        }

        if total_keywords == 0 {
            0.0
        } else {
            matches as f32 / total_keywords as f32
        }
    }
}
