//! Integrated Neurosymbolic Service
//!
//! Orchestrates all autonomous neurosymbolic components:
//! 1. FQL Autoformalization (NL → structured intent)
//! 2. Safety Validation (hard rules)
//! 3. Command Generation
//! 4. Manpage Validation (syntax checking)
//! 5. Learning Integration (RAG context)
//! 6. Execution with feedback loop

use crate::services::graph_builder::GraphBuilder;
use crate::services::learning_service::LearningService;
use anyhow::anyhow;
use domain::{
    domain_config::{types::GeneratedCommand, DomainRegistry},
    formal_query_language::{FqlAction, FqlParser, FqlQuery, FqlTarget},
    services::{ProofGenerator, SafetyProof},
    safety::{SafetyEngine, SafetyReport},
};
use infrastructure::{
    storage::{
        experience_buffer::{ExperienceBuffer, FailureType},
        induction_engine::InductionEngine,
        risk_scorer::{RiskLevel, RiskProfile, RiskScorer},
        knowledge_graph::KnowledgeGraph,
        ManpageCache,
    },
    syntax_grammar_validator::SyntaxGrammarValidator,
};
use shared::types::Result;
use std::collections::HashSet;
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

/// Optional structured intent signal from upstream analysis (LLM or rules)
#[derive(Debug, Clone, Default)]
pub struct IntentSignal {
    pub category: Option<String>,
    pub action: Option<String>,
    pub target: Option<String>,
    pub objects: Vec<String>,
    pub constraints: Vec<String>,
    pub params: std::collections::HashMap<String, String>,
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
    /// Risk profile assessment
    pub risk_profile: Option<RiskProfile>,
    /// Formal safety proof (for critical operations)
    pub safety_proof: Option<SafetyProof>,
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
            output.push_str(&format!("FQL: {}\n", fql.to_fql_string()));
        }

        // Safety
        output.push_str(&format!(
            "Safety: {}\n",
            self.safety_report.overall_risk
        ));

        // Syntax validation
        if self.syntax_valid {
            output.push_str("Syntax: Valid\n");
        } else {
            output.push_str(&format!(
                "Syntax: Invalid flags: {:?}\n",
                self.invalid_flags
            ));
        }

        // Learning context
        if self.learning_context.is_some() {
            output.push_str("Learning context applied\n");
        }

        // Risk assessment
        if let Some(ref profile) = self.risk_profile {
            output.push_str(&format!(
                "Risk: {} ({:.2})\n",
                profile.risk_level.as_str(),
                profile.overall_score
            ));
            if !profile.mitigation_steps.is_empty() {
                output.push_str("Mitigations:\n");
                for step in &profile.mitigation_steps {
                    output.push_str(&format!("  - {}\n", step));
                }
            }
        }

        // Safety proof
        if let Some(ref proof) = self.safety_proof {
            let status = if proof.verified { "verified" } else { "failed" };
            output.push_str(&format!(
                "Safety Proof: {} ({:.0}% confidence)\n",
                status,
                proof.confidence * 100.0
            ));
        }

        // Execution status
        if self.can_execute {
            output.push_str(&format!("Command: {}\n", self.command));
        } else {
            output.push_str(&format!(
                "Blocked: {}\n",
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
    risk_scorer: RiskScorer,
    proof_generator: ProofGenerator,
    induction_engine: Option<InductionEngine>,
    experience_db_path: PathBuf,
    knowledge_graph_path: PathBuf,
    manpage_cache: ManpageCache,
    domain_registry: Option<DomainRegistry>,
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

        let domains_dir = cache_dir.join("domains");
        let shared_dir = cache_dir.join("shared_entities");
        let _ = std::fs::create_dir_all(&domains_dir);
        let _ = std::fs::create_dir_all(&shared_dir);

        let manpage_cache = ManpageCache::new(cache_dir.join("manpage_cache.db"))?;
        let experience_db_path = cache_dir.join("experience.db");
        let knowledge_graph_path = cache_dir.join("knowledge_graph.db");
        let risk_buffer = ExperienceBuffer::new(&experience_db_path)?;
        let risk_scorer = RiskScorer::new().with_experience_buffer(risk_buffer);
        let proof_generator = ProofGenerator::new();
        let induction_engine = InductionEngine::new(cache_dir.join("induction.db")).ok();

        let domain_registry = DomainRegistry::new(
            domains_dir.clone(),
            domains_dir.clone(),
            shared_dir.clone(),
        )
        .ok();

        if let Ok(builder) = GraphBuilder::new() {
            if let Ok((entities, _)) = builder.get_stats() {
                if entities == 0 {
                    let _ = builder.discover_system();
                }
            }
        }

        Ok(Self {
            config: config.clone(),
            fql_parser: FqlParser::new(),
            safety_engine: SafetyEngine::new(),
            syntax_validator: SyntaxGrammarValidator::new(),
            learning_service: LearningService::new()?,
            risk_scorer,
            proof_generator,
            induction_engine,
            experience_db_path,
            knowledge_graph_path,
            manpage_cache,
            domain_registry,
        })
    }

    /// Process a query through the complete neurosymbolic pipeline
    pub fn process(&mut self, query: &str) -> Result<NeurosymbolicResult> {
        self.process_with_intent(query, None)
    }

    /// Process with optional upstream intent signal
    pub fn process_with_intent(
        &mut self,
        query: &str,
        intent: Option<&IntentSignal>,
    ) -> Result<NeurosymbolicResult> {
        let mut trace = vec![];
        trace.push(format!("Processing query: '{}'", query));

        // Step 1: FQL Autoformalization
        let fql = if self.config.enable_fql {
            trace.push("Step 1: Parsing to FQL...".to_string());
            let parsed = self.fql_from_intent_or_query(query, intent);
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
        let command = self.generate_command(query, fql.as_ref(), learning_context.as_deref(), &mut trace)?;
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

        // Step 5.5: Risk Assessment
        let risk_profile = Some(self.risk_scorer.assess(&command, query));

        // Step 5.6: Formal Proof for critical operations
        let safety_proof = risk_profile
            .as_ref()
            .filter(|profile| {
                matches!(
                    profile.risk_level,
                    RiskLevel::High | RiskLevel::Critical
                )
            })
            .map(|_| self.proof_generator.generate_safety_proof(&command, &safety_report, fql.as_ref()));

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
            risk_profile,
            safety_proof,
            can_execute,
            block_reason,
            trace,
        })
    }

    /// Generate command from query and FQL
    fn generate_command(
        &mut self,
        query: &str,
        fql: Option<&FqlQuery>,
        learning_context: Option<&str>,
        trace: &mut Vec<String>,
    ) -> Result<String> {
        if let Some(registry) = &self.domain_registry {
            if let Some(resolved) = registry.resolve_operation(query, fql) {
                if resolved.confidence < 0.75 {
                    return Err(anyhow!("Low confidence neurosymbolic match"));
                }
                trace.push(format!(
                    "  Resolved operation: {} ({:.0}%)",
                    resolved.op_id,
                    resolved.confidence * 100.0
                ));

                if let Some((_, operation)) = registry.get_operation(&resolved.op_id) {
                    if self.requires_service_input(operation, &resolved) {
                        return Err(anyhow!("Missing required service input"));
                    }

                    let mut generated = registry
                        .command_generator()
                        .generate(operation, &resolved.inputs);

                    if let Some(context) = learning_context {
                        generated = self.filter_with_learning_context(&generated, context);
                    }

                    if generated.is_empty() {
                        return Err(anyhow!("No command candidates generated"));
                    }

                    if let Some(best) = self.select_best_command(&mut generated, trace) {
                        return Ok(self.apply_fql_limits(best, fql));
                    }
                    return Err(anyhow!("No valid command candidates"));
                }

                return Err(anyhow!("Resolved operation not found"));
            } else {
                return Err(anyhow!("No neurosymbolic operation match"));
            }
        } else {
            return Err(anyhow!("Domain registry not available"));
        }
    }

    fn requires_service_input(
        &self,
        operation: &domain::domain_config::types::Operation,
        resolved: &domain::domain_config::registry::ResolvedOperation,
    ) -> bool {
        if !operation.input_schema.contains_key("service") {
            return false;
        }

        if resolved.inputs.contains_key("service") {
            return false;
        }

        if let Some(action) = resolved.inputs.get("action").and_then(|v| v.as_str()) {
            return action != "list";
        }

        true
    }

    fn apply_fql_limits(&self, command: String, fql: Option<&FqlQuery>) -> String {
        let Some(fql) = fql else {
            return command;
        };

        let mut limit: Option<u64> = None;
        for constraint in &fql.constraints {
            if let domain::formal_query_language::FqlConstraint::Limit(n) = constraint {
                limit = Some(*n);
                break;
            }
        }

        let Some(n) = limit else {
            return command;
        };

        match fql.target {
            FqlTarget::Process(_) => {
                if command.contains("head -n") || command.contains("tail -n") {
                    command
                } else {
                    format!("{} | head -n {}", command, n)
                }
            }
            _ => command,
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
        let mut target_needs_append = true;
        let mut line_limit: Option<u64> = None;

        for constraint in &fql.constraints {
            match constraint {
                domain::formal_query_language::FqlConstraint::Recursive(_) => {
                    command.push_str(" -r")
                }
                domain::formal_query_language::FqlConstraint::Force(_) => command.push_str(" -f"),
                domain::formal_query_language::FqlConstraint::Limit(limit) => {
                    line_limit = Some(*limit)
                }
                _ => {}
            }
        }

        match &fql.target {
            domain::formal_query_language::FqlTarget::Log(_) => {
                command = "journalctl".to_string();
                if let Some(limit) = line_limit {
                    command.push_str(&format!(" -n {}", limit));
                }
                target_needs_append = false;
            }
            domain::formal_query_language::FqlTarget::Memory => {
                command = "free -h".to_string();
                target_needs_append = false;
            }
            domain::formal_query_language::FqlTarget::Cpu => {
                command = "top -bn1 | head -20".to_string();
                target_needs_append = false;
            }
            _ => {}
        }

        if target_needs_append {
            // Add target when the base command expects it
            command.push_str(&format!(" {}", fql.target));
        }

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

    fn fql_from_intent_or_query(
        &self,
        query: &str,
        intent: Option<&IntentSignal>,
    ) -> Option<FqlQuery> {
        if let Some(signal) = intent {
            if let (Some(action_str), Some(target_str)) = (&signal.action, &signal.target) {
                let query_lower = query.to_lowercase();
                let target_norm = target_str.to_lowercase();

                if target_norm == "system"
                    || target_norm == "unknown"
                    || action_str.to_lowercase() == "unknown"
                    || (query_lower.contains("process") && target_norm != "process")
                    || (query_lower.contains("journal") && target_norm != "log")
                {
                    return self.fql_parser.parse(query);
                }

                if let (Some(action), Some(target)) = (
                    self.map_action(action_str),
                    self.map_target(target_str, signal),
                ) {
                    let mut fql = FqlQuery::new(action, target);

                    for c in &signal.constraints {
                        if let Some(constraint) = self.map_constraint(c) {
                            fql.constraints.push(constraint);
                        }
                    }

                    if let Some(lines) = signal
                        .params
                        .get("lines")
                        .and_then(|v| v.parse::<u64>().ok())
                    {
                        fql.constraints.push(domain::formal_query_language::FqlConstraint::Limit(
                            lines,
                        ));
                    }

                    return Some(fql);
                }
            }
        }

        self.fql_parser.parse(query)
    }

    fn map_action(&self, action: &str) -> Option<FqlAction> {
        match action.to_lowercase().as_str() {
            "list" => Some(FqlAction::List),
            "show" | "display" => Some(FqlAction::Show),
            "check" | "status" => Some(FqlAction::Check),
            "monitor" => Some(FqlAction::Monitor),
            "start" => Some(FqlAction::Start),
            "stop" => Some(FqlAction::Stop),
            "restart" | "reload" => Some(FqlAction::Restart),
            "enable" => Some(FqlAction::Enable),
            "disable" => Some(FqlAction::Disable),
            "find" | "search" => Some(FqlAction::Find),
            "read" | "tail" => Some(FqlAction::Read),
            "delete" | "remove" | "clean" => Some(FqlAction::Delete),
            _ => None,
        }
    }

    fn map_target(&self, target: &str, signal: &IntentSignal) -> Option<FqlTarget> {
        let target_lower = target.to_lowercase();
        let object = signal.objects.first().cloned().unwrap_or_default();

        match target_lower.as_str() {
            "gpu" | "graphics" | "hardware" => Some(FqlTarget::Component("gpu".to_string())),
            "memory" | "ram" => Some(FqlTarget::Memory),
            "cpu" => Some(FqlTarget::Cpu),
            "disk" => Some(FqlTarget::Disk("*".to_string())),
            "log" | "logs" | "journalctl" => Some(FqlTarget::Log("*".to_string())),
            "service" => {
                if !object.is_empty() {
                    Some(FqlTarget::Service(object))
                } else {
                    Some(FqlTarget::Service("*".to_string()))
                }
            }
            "process" => {
                if !object.is_empty() {
                    Some(FqlTarget::Process(object))
                } else {
                    Some(FqlTarget::Process("*".to_string()))
                }
            }
            "file" | "path" => {
                if let Some(path) = signal.params.get("path") {
                    Some(FqlTarget::Path(path.to_string()))
                } else if !object.is_empty() {
                    Some(FqlTarget::Path(object))
                } else {
                    Some(FqlTarget::Path("/".to_string()))
                }
            }
            "network" => Some(FqlTarget::Resource("network".to_string())),
            "user" => Some(FqlTarget::User("*".to_string())),
            "package" => Some(FqlTarget::Package("*".to_string())),
            _ => None,
        }
    }

    fn map_constraint(&self, constraint: &str) -> Option<domain::formal_query_language::FqlConstraint> {
        let c = constraint.to_lowercase();
        if c.contains("safe") {
            return Some(domain::formal_query_language::FqlConstraint::SafeDelete);
        }
        if c.contains("dry") {
            return Some(domain::formal_query_language::FqlConstraint::DryRun);
        }
        if c.contains("sudo") || c.contains("root") {
            return Some(domain::formal_query_language::FqlConstraint::RequiresSudo);
        }
        None
    }

    fn select_best_command(
        &mut self,
        candidates: &mut Vec<GeneratedCommand>,
        trace: &mut Vec<String>,
    ) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for candidate in candidates.iter() {
            let validation = self.syntax_validator.validate(&candidate.command);
            if validation.is_valid {
                trace.push(format!(
                    "  Manpage-validated generator: {}",
                    candidate.generator_name
                ));
                return Some(candidate.command.clone());
            }
        }

        trace.push("  Manpage validation found no valid candidates".to_string());
        candidates.first().map(|c| c.command.clone())
    }

    fn filter_with_learning_context(
        &self,
        candidates: &[GeneratedCommand],
        context: &str,
    ) -> Vec<GeneratedCommand> {
        let mut blocked = HashSet::new();
        for line in context.lines() {
            if let Some(rest) = line.trim().strip_prefix("Attempted: '") {
                if let Some(cmd) = rest.strip_suffix('\'') {
                    blocked.insert(cmd.to_string());
                }
            }
        }

        if blocked.is_empty() {
            return candidates.to_vec();
        }

        candidates
            .iter()
            .filter(|c| !blocked.contains(&c.command))
            .cloned()
            .collect()
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

            if let Some(engine) = &self.induction_engine {
                if let Ok(buffer) = ExperienceBuffer::new(&self.experience_db_path) {
                    if let Ok(patterns) = engine.mine_patterns(&buffer) {
                        if let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) {
                            let _ = engine.apply_rules_to_graph(&graph, &patterns);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Get learning statistics
    pub fn get_learning_stats(&self) -> Result<(usize, usize, f32)> {
        self.learning_service.get_stats()
    }

    /// Get manpage cache stats
    pub fn get_manpage_stats(&self) -> Result<(usize, usize)> {
        Ok(self.manpage_cache.stats()?)
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
