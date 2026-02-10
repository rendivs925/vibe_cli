// Command generator with scoring-based generator selection

use crate::domain_config::types::*;
use std::collections::HashMap;
use std::path::PathBuf;

const DEFAULT_TIMEOUT: u64 = 30;

/// Command generator for dynamic command resolution
#[derive(Debug, Clone)]
pub struct CommandGenerator {
    tool_registry: HashMap<String, ToolInfo>,
}

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub path: PathBuf,
    pub available: bool,
}

impl Default for CommandGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandGenerator {
    pub fn new() -> Self {
        let mut registry = HashMap::new();

        // Register common Linux tools
        let tools = [
            "ps",
            "pgrep",
            "kill",
            "cat",
            "ls",
            "stat",
            "chmod",
            "chown",
            "systemctl",
            "ss",
            "netstat",
            "df",
            "free",
            "uptime",
            "journalctl",
            "tail",
            "grep",
            "awk",
            "sed",
            "find",
            "useradd",
            "usermod",
            "groupadd",
            "getent",
            "id",
            "crontab",
            "bash",
            "sh",
            "echo",
            "date",
        ];

        for tool in tools {
            let path = which::which(tool)
                .ok()
                .unwrap_or_else(|| PathBuf::from(tool));
            registry.insert(
                tool.to_string(),
                ToolInfo {
                    name: tool.to_string(),
                    path: path.clone(),
                    available: path.exists() || which::which(tool).is_ok(),
                },
            );
        }

        Self {
            tool_registry: registry,
        }
    }

    /// Generate commands for an operation with given inputs
    pub fn generate(
        &self,
        operation: &Operation,
        inputs: &HashMap<String, serde_json::Value>,
    ) -> Vec<GeneratedCommand> {
        if operation.generators.is_empty() {
            return Vec::new();
        }

        // Score each generator and select best matches
        let mut scored: Vec<(Generator, f32)> = operation
            .generators
            .iter()
            .map(|gen| {
                let score = self.score_generator(gen, inputs);
                (gen.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Generate commands for top matches (with fallback support)
        scored
            .into_iter()
            .take_while(|(_, score)| *score > 0.0)
            .map(|(gen, score)| {
                let command = self.resolve_template(&gen.template, inputs);
                GeneratedCommand {
                    tool: gen.tool.clone(),
                    command: command.clone(),
                    generator_name: gen.name.clone(),
                    score,
                    timeout_seconds: gen.timeout_seconds.or(Some(DEFAULT_TIMEOUT)),
                }
            })
            .collect()
    }

    /// Score a generator based on input completeness
    pub(crate) fn score_generator(
        &self,
        generator: &Generator,
        inputs: &HashMap<String, serde_json::Value>,
    ) -> f32 {
        let mut score = 0.0;

        // Check required inputs
        for req in &generator.when {
            if let Some(value) = inputs.get(&req.name) {
                if !value.is_null() {
                    score += 1.0;
                } else {
                    return 0.0; // Missing required input
                }
            } else {
                return 0.0; // Missing required input
            }
        }

        // Base score for generators with no required inputs (can run as-is)
        if generator.when.is_empty() {
            score = 1.0;
        }

        // Bonus for optional inputs present
        for opt in &generator.optional {
            if inputs.contains_key(&opt.name) {
                score += 0.5;
            }
        }

        // Check tool availability
        if let Some(tool) = self.tool_registry.get(&generator.tool) {
            if !tool.available {
                // Reduce score for unavailable tools, but don't eliminate
                score *= 0.3;
            }
        }

        // Add preference score
        score += generator.preference_score;

        score
    }

    /// Resolve template variables with inputs
    pub(crate) fn resolve_template(
        &self,
        template: &str,
        inputs: &HashMap<String, serde_json::Value>,
    ) -> String {
        let mut result = template.to_string();

        // Replace {{variable}} patterns
        while let Some(start) = result.find("{{") {
            if let Some(end) = result[start..].find("}}") {
                let full_match = &result[start..start + end + 2];
                let var_name = &full_match[2..full_match.len() - 2].trim();

                // Handle conditional patterns like "| grep {{filter}}"
                if var_name.starts_with('|') {
                    // This is a conditional clause
                    let clause = &var_name[1..].trim(); // Remove leading |
                    let parts: Vec<&str> = clause.split_whitespace().collect();

                    if parts.len() >= 2 && parts[0] == "grep" {
                        let pattern_var = parts.get(1).map(|s| s.trim()).unwrap_or("");
                        if let Some(value) = inputs.get(pattern_var) {
                            let replacement = format!("| grep {}", value);
                            result = result.replace(full_match, &replacement);
                        } else {
                            // Remove the clause if filter not provided
                            result = result.replace(full_match, "");
                        }
                    } else {
                        result = result.replace(full_match, "");
                    }
                } else if let Some(value) = inputs.get(&var_name.to_string()) {
                    let value_str = match value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => value.to_string(),
                    };
                    result = result.replace(full_match, &value_str);
                } else {
                    // Variable not found, remove placeholder
                    result = result.replace(full_match, "");
                }
            } else {
                break; // No closing brace found
            }
        }

        // Clean up extra whitespace
        result = result.split_whitespace().collect::<Vec<_>>().join(" ");

        result
    }

    /// Check if a tool is available
    pub fn is_tool_available(&self, tool: &str) -> bool {
        self.tool_registry
            .get(tool)
            .map(|t| t.available)
            .unwrap_or(false)
    }

    /// Get list of available tools
    pub fn available_tools(&self) -> Vec<String> {
        self.tool_registry
            .values()
            .filter(|t| t.available)
            .map(|t| t.name.clone())
            .collect()
    }

    /// Get list of unavailable tools
    pub fn unavailable_tools(&self) -> Vec<String> {
        self.tool_registry
            .values()
            .filter(|t| !t.available)
            .map(|t| t.name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_resolution() {
        let generator = CommandGenerator::new();

        let mut inputs: HashMap<String, serde_json::Value> = HashMap::new();
        inputs.insert(
            "filter".to_string(),
            serde_json::Value::String("nginx".to_string()),
        );

        let template = "ps -eo pid,ppid,cmd | grep {{filter}}";
        let result = generator.resolve_template(template, &inputs);

        assert_eq!(result, "ps -eo pid,ppid,cmd | grep nginx");
    }

    #[test]
    fn test_conditional_clause_removal() {
        let generator = CommandGenerator::new();

        let mut inputs: HashMap<String, serde_json::Value> = HashMap::new();
        inputs.insert(
            "user".to_string(),
            serde_json::Value::String("www-data".to_string()),
        );

        let template = "ps -u {{user}} {{filter}}";
        let result = generator.resolve_template(template, &inputs);

        // filter not provided, so {{filter}} should be removed
        assert_eq!(result, "ps -u www-data");
    }

    #[test]
    fn test_generator_scoring() {
        let generator = CommandGenerator::new();

        let gen = Generator {
            name: "test".to_string(),
            tool: "ps".to_string(),
            template: "ps -eo pid,cmd".to_string(),
            when: vec![RequiredInput {
                name: "filter".to_string(),
                equals: None,
            }],
            optional: vec![],
            timeout_seconds: None,
            preference_score: 0.0,
        };

        // With required input
        let mut inputs: HashMap<String, serde_json::Value> = HashMap::new();
        inputs.insert(
            "filter".to_string(),
            serde_json::Value::String("nginx".to_string()),
        );
        let score = generator.score_generator(&gen, &inputs);
        assert!(score > 0.0);

        // Without required input
        let empty_inputs: HashMap<String, serde_json::Value> = HashMap::new();
        let score = generator.score_generator(&gen, &empty_inputs);
        assert_eq!(score, 0.0);
    }
}
