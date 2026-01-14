use crate::tools::{ToolArgs, ToolError, ToolOutput, ToolRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Command interpreter that translates natural language to safe tool calls
pub struct SafeCommandInterpreter {
    tool_registry: ToolRegistry,
    command_patterns: HashMap<String, CommandPattern>,
}

#[derive(Debug, Clone)]
pub struct CommandPattern {
    pub tool_name: String,
    pub parameter_mapping: HashMap<String, ParameterExtractor>,
    pub confidence_score: f32,
}

#[derive(Debug, Clone)]
pub enum ParameterExtractor {
    RegexCapture(String),
    FixedValue(String),
    UserInput(String),
    PathFromText,
    ContentFromText,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InterpretedCommand {
    pub tool_name: String,
    pub args: ToolArgs,
    pub confidence: f32,
    pub explanation: String,
}

impl SafeCommandInterpreter {
    pub fn new() -> Self {
        let tool_registry = ToolRegistry::new();

        let mut command_patterns = HashMap::new();

        // Define command patterns for file operations
        command_patterns.insert(
            "read_file".to_string(),
            CommandPattern {
                tool_name: "file_read".to_string(),
                parameter_mapping: HashMap::from([(
                    "path".to_string(),
                    ParameterExtractor::PathFromText,
                )]),
                confidence_score: 0.9,
            },
        );

        command_patterns.insert(
            "write_file".to_string(),
            CommandPattern {
                tool_name: "file_write".to_string(),
                parameter_mapping: HashMap::from([
                    ("path".to_string(), ParameterExtractor::PathFromText),
                    ("content".to_string(), ParameterExtractor::ContentFromText),
                ]),
                confidence_score: 0.8,
            },
        );

        command_patterns.insert(
            "list_directory".to_string(),
            CommandPattern {
                tool_name: "directory_list".to_string(),
                parameter_mapping: HashMap::from([(
                    "path".to_string(),
                    ParameterExtractor::PathFromText,
                )]),
                confidence_score: 0.9,
            },
        );

        command_patterns.insert(
            "show_processes".to_string(),
            CommandPattern {
                tool_name: "process_list".to_string(),
                parameter_mapping: HashMap::new(),
                confidence_score: 0.95,
            },
        );

        Self {
            tool_registry,
            command_patterns,
        }
    }

    /// Interpret natural language command into safe tool execution
    pub async fn interpret_command(
        &self,
        user_input: &str,
    ) -> Result<InterpretedCommand, InterpretationError> {
        // Clean and normalize input
        let input = user_input.trim().to_lowercase();

        // Try to match against known patterns
        for (pattern_name, pattern) in &self.command_patterns {
            if let Some(interpreted) = self.try_match_pattern(&input, pattern_name, pattern)? {
                return Ok(interpreted);
            }
        }

        // Fallback: attempt basic interpretation
        self.fallback_interpretation(&input)
    }

    fn try_match_pattern(
        &self,
        input: &str,
        pattern_name: &str,
        pattern: &CommandPattern,
    ) -> Result<Option<InterpretedCommand>, InterpretationError> {
        // Simple keyword matching for now (can be enhanced with NLP later)
        let keywords = match pattern_name {
            "read_file" => vec!["read", "show", "display", "cat", "view"],
            "write_file" => vec!["write", "create", "save", "echo"],
            "list_directory" => vec!["list", "ls", "dir", "show files"],
            "show_processes" => vec!["ps", "processes", "running", "top"],
            _ => vec![],
        };

        let matches_keywords = keywords.iter().any(|kw| input.contains(kw));
        if !matches_keywords {
            return Ok(None);
        }

        // Extract parameters based on pattern
        let mut parameters = HashMap::new();
        for (param_name, extractor) in &pattern.parameter_mapping {
            match extractor {
                ParameterExtractor::PathFromText => {
                    if let Some(path) = self.extract_path_from_text(input) {
                        parameters.insert(param_name.clone(), path);
                    }
                }
                ParameterExtractor::ContentFromText => {
                    if let Some(content) = self.extract_content_from_text(input) {
                        parameters.insert(param_name.clone(), content);
                    }
                }
                ParameterExtractor::FixedValue(value) => {
                    parameters.insert(param_name.clone(), value.clone());
                }
                ParameterExtractor::UserInput(prompt) => {
                    // This would require interactive input, skip for now
                    continue;
                }
                ParameterExtractor::RegexCapture(regex) => {
                    // Simple regex-like matching (can be enhanced)
                    if let Some(value) = self.extract_by_pattern(input, regex) {
                        parameters.insert(param_name.clone(), value);
                    }
                }
            }
        }

        let args = ToolArgs {
            parameters,
            timeout: Some(std::time::Duration::from_secs(30)),
            working_directory: None,
        };

        let explanation = format!(
            "Interpreted as {} using tool '{}'",
            pattern_name, pattern.tool_name
        );

        Ok(Some(InterpretedCommand {
            tool_name: pattern.tool_name.clone(),
            args,
            confidence: pattern.confidence_score,
            explanation,
        }))
    }

    fn fallback_interpretation(
        &self,
        input: &str,
    ) -> Result<InterpretedCommand, InterpretationError> {
        // Basic fallback - try to extract a file path and assume read operation
        if let Some(path) = self.extract_path_from_text(input) {
            let mut parameters = HashMap::new();
            parameters.insert("path".to_string(), path);

            return Ok(InterpretedCommand {
                tool_name: "file_read".to_string(),
                args: ToolArgs {
                    parameters,
                    timeout: Some(std::time::Duration::from_secs(30)),
                    working_directory: None,
                },
                confidence: 0.5,
                explanation: "Fallback: assuming file read operation".to_string(),
            });
        }

        Err(InterpretationError::UninterpretableCommand(
            input.to_string(),
        ))
    }

    fn extract_path_from_text(&self, text: &str) -> Option<String> {
        // Simple path extraction - look for file-like patterns
        let path_patterns = vec![
            r"/[\w/\.-]+",         // Unix paths
            r"[a-zA-Z]:[\w\\.-]+", // Windows paths
            r"[\w\.-/]+",          // Relative paths
        ];

        for pattern in path_patterns {
            if let Some(path_match) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.find(text))
                .map(|m| m.as_str().to_string())
            {
                // Basic validation - should contain file extension or be a directory
                if path_match.contains('.')
                    || path_match.ends_with('/')
                    || path_match == "."
                    || path_match == ".."
                {
                    return Some(path_match);
                }
            }
        }
        None
    }

    fn extract_content_from_text(&self, text: &str) -> Option<String> {
        // Look for content after keywords like "with", "containing", "write"
        let content_keywords = ["with", "containing", "write", "content"];

        for keyword in &content_keywords {
            if let Some(pos) = text.find(keyword) {
                let content_start = pos + keyword.len();
                if content_start < text.len() {
                    let content = text[content_start..].trim();
                    if !content.is_empty() && content.len() < 10000 {
                        // Reasonable content limit
                        return Some(content.to_string());
                    }
                }
            }
        }
        None
    }

    fn extract_by_pattern(&self, text: &str, pattern: &str) -> Option<String> {
        // Simple pattern matching (can be enhanced with regex)
        if pattern.contains("file") && text.contains("file") {
            return self.extract_path_from_text(text);
        }
        None
    }

    /// Execute interpreted command safely
    pub async fn execute_interpreted_command(
        &self,
        interpreted: InterpretedCommand,
    ) -> Result<ToolOutput, ToolError> {
        self.tool_registry
            .execute_tool(&interpreted.tool_name, interpreted.args)
            .await
    }

    /// Get available tools
    pub fn list_available_tools(&self) -> Vec<String> {
        self.tool_registry.list_tools()
    }
}

#[derive(Debug, Clone)]
pub enum InterpretationError {
    UninterpretableCommand(String),
    InvalidParameters(String),
    ToolNotAvailable(String),
}

impl std::fmt::Display for InterpretationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpretationError::UninterpretableCommand(cmd) => {
                write!(f, "Could not interpret command: {}", cmd)
            }
            InterpretationError::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            InterpretationError::ToolNotAvailable(tool) => {
                write!(f, "Tool not available: {}", tool)
            }
        }
    }
}

impl std::error::Error for InterpretationError {}

/// Integration function for backward compatibility
pub async fn interpret_and_execute_safe_command(
    user_input: &str,
) -> Result<ToolOutput, Box<dyn std::error::Error>> {
    let interpreter = SafeCommandInterpreter::new();

    let interpreted = interpreter.interpret_command(user_input).await?;

    // Log interpretation for debugging
    println!("Interpreted command: {}", interpreted.explanation);
    println!("Using tool: {}", interpreted.tool_name);
    println!("Confidence: {:.2}", interpreted.confidence);

    let result = interpreter.execute_interpreted_command(interpreted).await?;

    Ok(result)
}
