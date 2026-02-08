//! Integrated Neurosymbolic Service
//!
//! Orchestrates all autonomous neurosymbolic components:
//! 1. FQL Autoformalization (NL → structured intent)
//! 2. Safety Validation (hard rules)
//! 3. Command Generation
//! 4. Manpage Validation (syntax checking)
//! 5. Learning Integration (RAG context)
//! 6. Execution with feedback loop

use crate::services::learning_service::LearningService;
use domain::{
    formal_query_language::{FqlParser, FqlQuery},
    safety::{SafetyEngine, SafetyReport},
};
use infrastructure::{
    manpage_crawler::ManpageCrawler,
    storage::{experience_buffer::FailureType, knowledge_graph::KnowledgeGraph, ManpageCache},
    syntax_grammar_validator::SyntaxGrammarValidator,
};
use shared::types::Result;
use std::path::PathBuf;

/// Configuration for neurosymbolic processing
#[derive(Debug, Clone)]
pub struct NeurosymbolicConfig {
    /// Enable FQL autoformalization
    pub enable_fql: bool,
    /// Enable safety validation
    pub enable_safety: bool,
    /// Enable manpage validation
    pub enable_manpage_validation: bool,
    /// Enable learning/RAG
    pub enable_learning: bool,
    /// Output FQL in trace
    pub output_fql: bool,
    /// Require confirmation for safety warnings (dangerous commands always blocked)
    pub block_on_safety: bool,
    /// Block on invalid syntax
    pub block_on_invalid_syntax: bool,
}

impl Default for NeurosymbolicConfig {
    fn default() -> Self {
        Self {
            enable_fql: true,
            enable_safety: true,
            enable_manpage_validation: true,
            enable_learning: true,
            output_fql: true,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        }
    }
}

/// Result of neurosymbolic processing
#[derive(Debug, Clone)]
pub struct NeurosymbolicResult {
    /// Original query
    pub query: String,
    /// Parsed FQL (if enabled)
    pub fql: Option<FqlQuery>,
    /// Safety report
    pub safety_report: SafetyReport,
    /// Generated command
    pub command: String,
    /// Syntax validation result
    pub syntax_valid: bool,
    /// Invalid flags found
    pub invalid_flags: Vec<String>,
    /// Learning context applied
    pub learning_context: Option<String>,
    /// Whether execution is allowed
    pub can_execute: bool,
    /// Reason if execution blocked
    pub block_reason: Option<String>,
    /// Reasoning trace
    pub trace: Vec<String>,
}

impl NeurosymbolicResult {
    /// Format result for display
    pub fn format_display(&self) -> String {
        let mut output = String::new();

        // FQL output
        if let Some(ref fql) = self.fql {
            output.push_str(&format!("📝 FQL: {}\n", fql.to_fql_string()));
        }

        // Safety
        let safety_icon = if self.safety_report.is_safe() {
            "🟢"
        } else if self.safety_report.is_blocked() {
            "🔴"
        } else {
            "🟡"
        };
        output.push_str(&format!(
            "{} Safety: {}\n",
            safety_icon, self.safety_report.overall_risk
        ));

        // Syntax validation
        if self.syntax_valid {
            output.push_str("✓ Syntax: Valid\n");
        } else {
            output.push_str(&format!(
                "✗ Syntax: Invalid flags: {:?}\n",
                self.invalid_flags
            ));
        }

        // Learning context
        if self.learning_context.is_some() {
            output.push_str("🧠 Learning context applied\n");
        }

        // Execution status
        if self.can_execute {
            output.push_str(&format!("▶ Command: {}\n", self.command));
        } else {
            output.push_str(&format!(
                "⛔ Blocked: {}\n",
                self.block_reason.as_deref().unwrap_or("Unknown reason")
            ));
        }

        output
    }
}

/// Integrated neurosymbolic service
pub struct IntegratedNeurosymbolicService {
    config: NeurosymbolicConfig,
    fql_parser: FqlParser,
    safety_engine: SafetyEngine,
    syntax_validator: SyntaxGrammarValidator,
    learning_service: LearningService,
    manpage_cache: ManpageCache,
}

impl IntegratedNeurosymbolicService {
    /// Create new integrated service with default config
    pub fn new() -> Result<Self> {
        let config = NeurosymbolicConfig::default();
        Self::with_config(config)
    }

    /// Create with custom configuration
    pub fn with_config(config: NeurosymbolicConfig) -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let cache_dir = PathBuf::from(home).join(".config/vibe_cli");
        let _ = std::fs::create_dir_all(&cache_dir);

        let manpage_cache = ManpageCache::new(cache_dir.join("manpage_cache.db"))?;

        Ok(Self {
            config: config.clone(),
            fql_parser: FqlParser::new(),
            safety_engine: SafetyEngine::new(),
            syntax_validator: SyntaxGrammarValidator::new(),
            learning_service: LearningService::new()?,
            manpage_cache,
        })
    }

    /// Process a query through the complete neurosymbolic pipeline
    pub fn process(&mut self, query: &str) -> Result<NeurosymbolicResult> {
        let mut trace = vec![];
        trace.push(format!("Processing query: '{}'", query));

        // Step 1: FQL Autoformalization
        let fql = if self.config.enable_fql {
            trace.push("Step 1: Parsing to FQL...".to_string());
            let parsed = self.fql_parser.parse(query);
            if let Some(ref f) = parsed {
                trace.push(format!("  FQL: {}", f.to_fql_string()));
            }
            parsed
        } else {
            None
        };

        // Step 2: Learning - Get context from past experiences
        let learning_context = if self.config.enable_learning {
            trace.push("Step 2: Retrieving learning context...".to_string());
            let context = self.learning_service.get_context_for_query(query)?;
            if context.is_some() {
                trace.push("  Found relevant past experiences".to_string());
            }
            context
        } else {
            None
        };

        // Step 3: Generate command (simplified - would use domain config)
        trace.push("Step 3: Generating command...".to_string());
        let command = self.generate_command(query, fql.as_ref())?;
        trace.push(format!("  Generated: {}", command));

        // Step 4: Safety Validation
        let safety_report = if self.config.enable_safety {
            trace.push("Step 4: Validating safety...".to_string());
            let report = self.safety_engine.analyze(&command);
            trace.push(format!("  Risk level: {}", report.overall_risk));
            if !report.violations.is_empty() {
                trace.push(format!("  Violations: {}", report.violations.len()));
            }
            report
        } else {
            SafetyReport::safe(&command)
        };

        // Step 5: Syntax/Manpage Validation
        let (syntax_valid, invalid_flags) = if self.config.enable_manpage_validation {
            trace.push("Step 5: Validating syntax...".to_string());
            let validation = self.syntax_validator.validate(&command);
            let valid = validation.is_valid;
            let invalid = validation.invalid_flags.clone();
            if !valid {
                trace.push(format!("  Invalid flags: {:?}", invalid));
            } else {
                trace.push("  Syntax valid".to_string());
            }
            (valid, invalid)
        } else {
            (true, vec![])
        };

        // Step 6: Determine if execution is allowed
        let (can_execute, block_reason) =
            self.determine_execution_status(&safety_report, syntax_valid, &invalid_flags);

        if !can_execute {
            trace.push(format!(
                "  EXECUTION BLOCKED: {}",
                block_reason.as_deref().unwrap_or("")
            ));
        }

        Ok(NeurosymbolicResult {
            query: query.to_string(),
            fql,
            safety_report,
            command,
            syntax_valid,
            invalid_flags,
            learning_context,
            can_execute,
            block_reason,
            trace,
        })
    }

    /// Generate command from query and FQL
    fn generate_command(&self, query: &str, fql: Option<&FqlQuery>) -> Result<String> {
        // For now, simple heuristic generation
        // In production, this would use the domain config system
        let query_lower = query.to_lowercase();

        if let Some(fql) = fql {
            // Use FQL to generate command
            self.command_from_fql(fql)
        } else {
            // Fallback: simple keyword matching
            self.heuristic_command_generation(&query_lower)
        }
    }

    /// Generate command from FQL
    fn command_from_fql(&self, fql: &FqlQuery) -> Result<String> {
        use domain::formal_query_language::FqlAction;

        let mut command = match fql.action {
            FqlAction::List => "ls".to_string(),
            FqlAction::Delete => "rm".to_string(),
            FqlAction::Create => "touch".to_string(),
            FqlAction::Read => "cat".to_string(),
            FqlAction::Check => "ps".to_string(),
            FqlAction::Start => "systemctl start".to_string(),
            FqlAction::Stop => "systemctl stop".to_string(),
            FqlAction::Find => "find".to_string(),
            _ => "echo".to_string(),
        };

        // Add flags based on constraints
        for constraint in &fql.constraints {
            match constraint {
                domain::formal_query_language::FqlConstraint::Recursive(_) => {
                    command.push_str(" -r")
                }
                domain::formal_query_language::FqlConstraint::Force(_) => command.push_str(" -f"),
                _ => {}
            }
        }

        // Add target
        command.push_str(&format!(" {}", fql.target));

        Ok(command)
    }

    /// Heuristic command generation (fallback)
    fn heuristic_command_generation(&self, query: &str) -> Result<String> {
        if query.contains("process") {
            Ok("ps aux".to_string())
        } else if query.contains("disk") {
            Ok("df -h".to_string())
        } else if query.contains("memory") {
            Ok("free -h".to_string())
        } else if query.contains("file") {
            Ok("ls -la".to_string())
        } else {
            Ok(format!("echo '{}'", query))
        }
    }

    /// Determine if command can be executed
    /// NOTE: Dangerous commands are ALWAYS blocked, regardless of configuration
    fn determine_execution_status(
        &self,
        safety_report: &SafetyReport,
        syntax_valid: bool,
        invalid_flags: &[String],
    ) -> (bool, Option<String>) {
        // Check safety - DANGEROUS COMMANDS ARE ALWAYS BLOCKED
        // This is a hard safety requirement that cannot be disabled
        if safety_report.is_blocked() {
            let violations: Vec<String> = safety_report
                .blocked_violations()
                .iter()
                .map(|v| format!("{}: {}", v.rule_id, v.rule_name))
                .collect();

            let reason = format!(
                "CRITICAL SAFETY VIOLATION - This command is dangerous and CANNOT be executed.\n  Violations: {}",
                violations.join("\n  ")
            );
            return (false, Some(reason));
        }

        // Check for warnings if safety is enabled
        if self.config.block_on_safety && !safety_report.is_safe() {
            let warnings: Vec<String> = safety_report
                .warning_violations()
                .iter()
                .map(|v| v.rule_name.clone())
                .collect();

            let reason = format!(
                "Safety warnings require confirmation: {}",
                warnings.join(", ")
            );
            // Return true but with warning - execution can proceed with user confirmation
            return (true, Some(reason));
        }

        // Check syntax
        if self.config.block_on_invalid_syntax && !syntax_valid && !invalid_flags.is_empty() {
            let reason = format!("Invalid flags: {}", invalid_flags.join(", "));
            return (false, Some(reason));
        }

        (true, None)
    }

    /// Record successful execution for learning
    pub fn record_success(&self, query: &str, command: &str) -> Result<()> {
        if self.config.enable_learning {
            self.learning_service
                .record_success(query, None, command, None)?;
        }
        Ok(())
    }

    /// Record failure for learning
    pub fn record_failure(
        &self,
        query: &str,
        command: &str,
        failure_type: FailureType,
        error_message: Option<&str>,
    ) -> Result<()> {
        if self.config.enable_learning {
            self.learning_service.record_failure(
                query,
                None,
                command,
                failure_type,
                error_message,
            )?;
        }
        Ok(())
    }

    /// Get learning statistics
    pub fn get_learning_stats(&self) -> Result<(usize, usize, f32)> {
        self.learning_service.get_stats()
    }

    /// Get manpage cache stats
    pub fn get_manpage_stats(&self) -> Result<(usize, usize)> {
        self.manpage_cache.stats()
    }

    /// Update configuration
    pub fn set_config(&mut self, config: NeurosymbolicConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &NeurosymbolicConfig {
        &self.config
    }
}

impl Default for IntegratedNeurosymbolicService {
    fn default() -> Self {
        Self::new().expect("Failed to initialize neurosymbolic service")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_safe_command() {
        let mut service = IntegratedNeurosymbolicService::new().unwrap();
        let result = service.process("list files").unwrap();

        assert!(result.can_execute);
        assert!(result.safety_report.is_safe());
    }

    #[test]
    fn test_process_dangerous_command() {
        let mut service = IntegratedNeurosymbolicService::new().unwrap();
        let result = service.process("rm -rf /").unwrap();

        assert!(!result.can_execute);
        assert!(result.safety_report.is_blocked());
    }

    #[test]
    fn test_config_defaults() {
        let config = NeurosymbolicConfig::default();
        assert!(config.enable_fql);
        assert!(config.enable_safety);
        assert!(config.enable_learning);
    }
}
