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
    safety::{SafetyEngine, SafetyReport},
    services::{ProofGenerator, SafetyProof},
};
use infrastructure::{
    storage::{
        experience_buffer::{ExperienceBuffer, FailureType},
        induction_engine::InductionEngine,
        knowledge_graph::KnowledgeGraph,
        risk_scorer::{RiskLevel, RiskProfile, RiskScorer},
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

#[derive(Debug, Clone)]
pub struct IntentSuggestion {
    pub intent: String,
    pub action: Option<String>,
    pub target: Option<String>,
    pub objects: Vec<String>,
    pub constraints: Vec<String>,
    pub params: std::collections::HashMap<String, String>,
    pub reasoning: String,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct SymbolicCommandSuggestion {
    pub op_id: String,
    pub op_name: String,
    pub commands: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct DomainCommandValidation {
    pub is_valid: bool,
    pub reason: Option<String>,
    pub suggestion: Option<SymbolicCommandSuggestion>,
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
    /// Structured reasoning template (if available)
    pub reasoning_template: Option<domain::domain_config::types::ReasoningTemplate>,
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
        output.push_str(&format!("Safety: {}\n", self.safety_report.overall_risk));

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
            if self.reasoning_template.is_some() {
                output.push_str(&format!("Risk: {}\n", profile.risk_level.as_str()));
            } else {
                output.push_str(&format!(
                    "Risk: {} ({:.2})\n",
                    profile.risk_level.as_str(),
                    profile.overall_score
                ));
            }
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

        let domain_registry_result =
            DomainRegistry::new(domains_dir.clone(), domains_dir.clone(), shared_dir.clone());
        let domain_registry = match domain_registry_result {
            Ok(registry) => {
                eprintln!(
                    "Domain registry loaded successfully with {} domains",
                    registry.list_domains().len()
                );
                Some(registry)
            }
            Err(e) => {
                eprintln!("Failed to load domain registry: {:?}", e);
                None
            }
        };

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

    pub fn suggest_intent_from_domains(&self, query: &str) -> Option<IntentSuggestion> {
        let registry = self.domain_registry.as_ref()?;
        let fql = self.fql_parser.parse(query)?;
        let resolved = registry.resolve_operation(query, Some(&fql))?;

        let action = Some(fql.action.to_string());
        let (target, objects) = self.target_to_category_and_objects(&fql.target);
        let intent = target.clone().unwrap_or_else(|| "system_info".to_string());

        let reasoning = format!(
            "Matched operation '{}' in domain '{}' (confidence {:.0}%)",
            resolved.op_id,
            resolved.domain_id,
            resolved.confidence * 100.0
        );

        Some(IntentSuggestion {
            intent,
            action,
            target,
            objects,
            constraints: Vec::new(),
            params: std::collections::HashMap::new(),
            reasoning,
            confidence: resolved.confidence,
        })
    }

    pub fn suggest_commands_from_domains(&self, query: &str) -> Option<SymbolicCommandSuggestion> {
        let registry = self.domain_registry.as_ref()?;
        let fql = self.fql_parser.parse(query)?;
        let resolved = registry.resolve_operation(query, Some(&fql))?;
        if let Some((action_score, target_score, _total)) = registry.match_scores(&fql, &resolved.op_id) {
            if target_score < 0.6 || action_score < 0.5 {
                return None;
            }
        }
        let operation = registry.get_operation(&resolved.op_id)?.1;
        let generated = registry
            .command_generator()
            .generate(operation, &resolved.inputs);
        let mut commands: Vec<String> = generated.into_iter().map(|g| g.command).collect();
        commands.sort();
        commands.dedup();

        Some(SymbolicCommandSuggestion {
            op_id: resolved.op_id,
            op_name: operation.name.clone(),
            commands,
            confidence: resolved.confidence,
        })
    }

    pub fn learning_context(&self, query: &str) -> Result<Option<String>> {
        if !self.config.enable_learning {
            return Ok(None);
        }
        let context = self.learning_service.format_learning_context(query)?;
        if context.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(context))
        }
    }

    pub fn failed_commands_for_query(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        if !self.config.enable_learning {
            return Ok(Vec::new());
        }
        self.learning_service.get_failed_commands(query, limit)
    }

    pub fn validate_command_against_domain(
        &self,
        query: &str,
        command: &str,
    ) -> DomainCommandValidation {
        let suggestion = self.suggest_commands_from_domains(query);
        let Some(suggestion) = suggestion else {
            return DomainCommandValidation {
                is_valid: false,
                reason: Some("no matching symbolic operation".to_string()),
                suggestion: None,
            };
        };
        self.validate_command_against_suggestion(command, &suggestion)
    }

    pub fn validate_command_against_suggestion(
        &self,
        command: &str,
        suggestion: &SymbolicCommandSuggestion,
    ) -> DomainCommandValidation {
        let normalized = normalize_command(command);
        let mut match_reason: Option<String> = None;
        let mut matches = false;

        let mut first_reason: Option<String> = None;
        for candidate in &suggestion.commands {
            let cand_norm = normalize_command(candidate);
            if normalized == cand_norm || strip_sudo(&normalized) == strip_sudo(&cand_norm) {
                matches = true;
                match_reason = Some("exact match".to_string());
                break;
            }

            if command_matches_template(command, candidate) {
                matches = true;
                match_reason = Some("tool and flag match".to_string());
                break;
            }

            if first_reason.is_none() {
                first_reason = mismatch_reason(command, candidate);
            }
        }

        if matches {
            DomainCommandValidation {
                is_valid: true,
                reason: match_reason,
                suggestion: Some(suggestion.clone()),
            }
        } else {
            DomainCommandValidation {
                is_valid: false,
                reason: Some(
                    first_reason
                        .unwrap_or_else(|| "command not in symbolic operation templates".to_string()),
                ),
                suggestion: Some(suggestion.clone()),
            }
        }
    }


    pub fn reload_domain_registry(&mut self) -> Result<()> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let base_dir = PathBuf::from(home).join(".config/vibe_cli");
        let domains_dir = base_dir.join("domains");
        let shared_dir = base_dir.join("shared_entities");
        let _ = std::fs::create_dir_all(&domains_dir);
        let _ = std::fs::create_dir_all(&shared_dir);

        self.domain_registry =
            DomainRegistry::new(domains_dir.clone(), domains_dir, shared_dir).ok();
        Ok(())
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
        let reasoning_template = self
            .domain_registry
            .as_ref()
            .and_then(|registry| registry.resolve_reasoning_template(query, fql.as_ref()))
            .map(|template| self.render_reasoning_template(&template, fql.as_ref(), intent));
        let command = self.generate_command(
            query,
            fql.as_ref(),
            learning_context.as_deref(),
            &mut trace,
        )?;
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
        let (mut syntax_valid, mut invalid_flags) = if self.config.enable_manpage_validation {
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

        // Retry once by stripping invalid flags if syntax is invalid
        if self.config.enable_manpage_validation && !syntax_valid && !invalid_flags.is_empty() {
            if let Some(cleaned) = self.strip_invalid_flags(&command, &invalid_flags) {
                trace.push(format!("  Retrying without invalid flags: {}", cleaned));
                let retry = self.syntax_validator.validate(&cleaned);
                syntax_valid = retry.is_valid;
                invalid_flags = retry.invalid_flags.clone();
                if syntax_valid {
                    trace.push("  Syntax valid after retry".to_string());
                } else if !invalid_flags.is_empty() {
                    trace.push(format!("  Still invalid flags: {:?}", invalid_flags));
                }
            }
        }

        // Step 5.5: Risk Assessment
        let risk_profile = Some(self.risk_scorer.assess(&command, query));

        // Step 5.6: Formal Proof for critical operations
        let safety_proof = risk_profile
            .as_ref()
            .filter(|profile| matches!(profile.risk_level, RiskLevel::High | RiskLevel::Critical))
            .map(|_| {
                self.proof_generator
                    .generate_safety_proof(&command, &safety_report, fql.as_ref())
            });

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
            reasoning_template,
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
                let mut min_confidence = 0.6;
                if let Some(fql) = fql {
                    if matches!(fql.target, FqlTarget::Resource(_) | FqlTarget::Entity(_)) {
                        min_confidence = 0.6;
                    } else {
                        min_confidence = 0.4;
                    }
                }
                if resolved.confidence < min_confidence {
                    return Err(anyhow!("Low confidence neurosymbolic match"));
                }
                if let Some(query_fql) = fql {
                    self.self_critique_operation(registry, query_fql, &resolved.op_id, trace)?;
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

    fn self_critique_operation(
        &self,
        registry: &domain::domain_config::registry::DomainRegistry,
        query_fql: &FqlQuery,
        op_id: &str,
        trace: &mut Vec<String>,
    ) -> Result<()> {
        let Some(scores) = registry.match_scores(query_fql, op_id) else {
            trace.push("  Self-critique: no signature scores available".to_string());
            return Ok(());
        };

        let (action_score, target_score, total_score) = scores;
        trace.push(format!(
            "  Self-critique: action {:.0}%, target {:.0}% (total {:.0}%)",
            action_score * 100.0,
            target_score * 100.0,
            total_score * 100.0
        ));

        if target_score < 0.7 {
            return Err(anyhow!("Self-critique: target mismatch"));
        }

        if action_score < 0.6 {
            return Err(anyhow!("Self-critique: action mismatch"));
        }

        Ok(())
    }

    fn render_reasoning_template(
        &self,
        template: &domain::domain_config::types::ReasoningTemplate,
        fql: Option<&FqlQuery>,
        intent: Option<&IntentSignal>,
    ) -> domain::domain_config::types::ReasoningTemplate {
        let mut context: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        if let Some(fql) = fql {
            match &fql.target {
                FqlTarget::Service(svc) if svc != "*" => {
                    context.insert("service".to_string(), svc.clone());
                }
                FqlTarget::Process(proc_name) if proc_name != "*" => {
                    context.insert("process".to_string(), proc_name.clone());
                }
                FqlTarget::Path(path) | FqlTarget::Directory(path) | FqlTarget::File(path) => {
                    context.insert("path".to_string(), path.clone());
                }
                FqlTarget::Log(log) if log != "*" => {
                    context.insert("log".to_string(), log.clone());
                }
                FqlTarget::NetworkInterface(iface) if iface != "*" => {
                    context.insert("interface".to_string(), iface.clone());
                }
                FqlTarget::User(user) if user != "*" => {
                    context.insert("user".to_string(), user.clone());
                }
                FqlTarget::Package(pkg) if pkg != "*" => {
                    context.insert("package".to_string(), pkg.clone());
                }
                _ => {}
            }

            if let Some(pattern) = &fql.pattern {
                context.insert("pattern".to_string(), pattern.to_string());
            }

            for constraint in &fql.constraints {
                if let domain::formal_query_language::FqlConstraint::Limit(n) = constraint {
                    context.insert("lines".to_string(), n.to_string());
                }
            }
        }

        if let Some(intent) = intent {
            for (k, v) in &intent.params {
                if !v.is_empty() && !context.contains_key(k) {
                    context.insert(k.clone(), v.clone());
                }
            }
        }

        let replace_vars = |input: &str, ctx: &std::collections::HashMap<String, String>| {
            let mut out = input.to_string();
            for (k, v) in ctx {
                out = out.replace(&format!("{{{{{}}}}}", k), v);
            }
            out
        };

        let mut rendered = template.clone();
        rendered.steps = rendered
            .steps
            .into_iter()
            .map(|mut step| {
                step.check = replace_vars(&step.check, &context);
                step.logic = replace_vars(&step.logic, &context);
                step.next = step
                    .next
                    .into_iter()
                    .map(|n| replace_vars(&n, &context))
                    .collect();
                step
            })
            .collect();

        rendered
    }

    fn strip_invalid_flags(&self, command: &str, invalid_flags: &[String]) -> Option<String> {
        if invalid_flags.is_empty() {
            return None;
        }

        let invalid: std::collections::HashSet<&str> =
            invalid_flags.iter().map(String::as_str).collect();
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let mut cleaned: Vec<&str> = Vec::with_capacity(parts.len());
        let mut skip_next = false;

        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                cleaned.push(part);
                continue;
            }
            if skip_next {
                skip_next = false;
                continue;
            }
            if invalid.contains(*part) {
                if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                    skip_next = true;
                }
                continue;
            }
            cleaned.push(part);
        }

        let result = cleaned.join(" ");
        if result.trim() == command.trim() {
            None
        } else {
            Some(result)
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
                        fql.constraints
                            .push(domain::formal_query_language::FqlConstraint::Limit(lines));
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

    fn target_to_category_and_objects(&self, target: &FqlTarget) -> (Option<String>, Vec<String>) {
        match target {
            FqlTarget::Process(v) => (Some("process".to_string()), vec![v.clone()]),
            FqlTarget::Service(v) => (Some("service".to_string()), vec![v.clone()]),
            FqlTarget::Package(v) => (Some("package".to_string()), vec![v.clone()]),
            FqlTarget::User(v) => (Some("user".to_string()), vec![v.clone()]),
            FqlTarget::Group(v) => (Some("user".to_string()), vec![v.clone()]),
            FqlTarget::NetworkInterface(v) => (Some("network".to_string()), vec![v.clone()]),
            FqlTarget::Port(_) | FqlTarget::Host(_) | FqlTarget::Url(_) => {
                (Some("network".to_string()), Vec::new())
            }
            FqlTarget::Memory => (Some("memory".to_string()), Vec::new()),
            FqlTarget::Cpu => (Some("system_info".to_string()), Vec::new()),
            FqlTarget::Disk(v) | FqlTarget::Filesystem(v) => {
                (Some("disk".to_string()), vec![v.clone()])
            }
            FqlTarget::Log(v) => (Some("log".to_string()), vec![v.clone()]),
            FqlTarget::Configuration(v)
            | FqlTarget::Variable(v)
            | FqlTarget::File(v)
            | FqlTarget::Directory(v)
            | FqlTarget::Path(v) => (Some("file".to_string()), vec![v.clone()]),
            FqlTarget::Database(v) | FqlTarget::Table(v) | FqlTarget::Record(v) => {
                (Some("system_info".to_string()), vec![v.clone()])
            }
            FqlTarget::Resource(v) | FqlTarget::Component(v) | FqlTarget::Entity(v) => {
                (Some("system_info".to_string()), vec![v.clone()])
            }
        }
    }

    fn map_constraint(
        &self,
        constraint: &str,
    ) -> Option<domain::formal_query_language::FqlConstraint> {
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

fn normalize_command(command: &str) -> String {
    let trimmed = command.trim().trim_end_matches(';').trim();
    trimmed
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join(" ")
}

fn strip_sudo(command: &str) -> String {
    command
        .strip_prefix("sudo ")
        .unwrap_or(command)
        .to_string()
}

#[derive(Debug)]
struct CommandSegment {
    cmd: String,
    flags: Vec<String>,
}

fn command_matches_template(command: &str, template: &str) -> bool {
    let cmd_segments = parse_segments(command);
    let tpl_segments = parse_segments(template);
    if cmd_segments.is_empty() || tpl_segments.is_empty() {
        return false;
    }
    if cmd_segments.len() != tpl_segments.len() {
        return false;
    }

    for (cmd_seg, tpl_seg) in cmd_segments.iter().zip(tpl_segments.iter()) {
        if cmd_seg.cmd != tpl_seg.cmd {
            return false;
        }
        if !flags_subset(&tpl_seg.flags, &cmd_seg.flags) {
            return false;
        }
    }

    true
}

fn mismatch_reason(command: &str, template: &str) -> Option<String> {
    let cmd_segments = parse_segments(command);
    let tpl_segments = parse_segments(template);
    if cmd_segments.is_empty() || tpl_segments.is_empty() {
        return Some("unable to parse command segments".to_string());
    }
    if cmd_segments.len() != tpl_segments.len() {
        return Some(format!(
            "segment count mismatch (got {}, expected {})",
            cmd_segments.len(),
            tpl_segments.len()
        ));
    }

    for (cmd_seg, tpl_seg) in cmd_segments.iter().zip(tpl_segments.iter()) {
        if cmd_seg.cmd != tpl_seg.cmd {
            return Some(format!(
                "tool mismatch (got '{}', expected '{}')",
                cmd_seg.cmd, tpl_seg.cmd
            ));
        }
        let missing = missing_flags(&tpl_seg.flags, &cmd_seg.flags);
        if !missing.is_empty() {
            return Some(format!("missing flags: {}", missing.join(", ")));
        }
    }

    None
}

fn parse_segments(command: &str) -> Vec<CommandSegment> {
    let normalized = normalize_command(command);
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = normalized.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '|' {
            segments.push(current.trim().to_string());
            current.clear();
            continue;
        }
        if ch == ';' {
            segments.push(current.trim().to_string());
            current.clear();
            continue;
        }
        if ch == '&' {
            if matches!(chars.peek(), Some('&')) {
                chars.next();
                segments.push(current.trim().to_string());
                current.clear();
                continue;
            }
        }
        current.push(ch);
    }

    if !current.trim().is_empty() {
        segments.push(current.trim().to_string());
    }

    segments
        .into_iter()
        .filter_map(|seg| parse_segment(&seg))
        .collect()
}

fn parse_segment(segment: &str) -> Option<CommandSegment> {
    let mut parts: Vec<&str> = segment.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    if parts[0] == "sudo" {
        parts.remove(0);
    }

    if parts.is_empty() {
        return None;
    }

    let cmd = parts[0].to_string();
    let mut flags = Vec::new();
    let mut skip_next = false;

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }

        if part.starts_with('-') && *part != "-" {
            if !part.starts_with("--") && part.len() > 2 {
                for c in part.chars().skip(1) {
                    flags.push(format!("-{}", c));
                }
            } else {
                flags.push(part.to_string());
            }

            if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                skip_next = true;
            }
        }
    }

    Some(CommandSegment { cmd, flags })
}

fn flags_subset(required: &[String], actual: &[String]) -> bool {
    if required.is_empty() {
        return true;
    }
    let set: std::collections::HashSet<&str> =
        actual.iter().map(|s| s.as_str()).collect();
    required.iter().all(|f| set.contains(f.as_str()))
}

fn missing_flags(required: &[String], actual: &[String]) -> Vec<String> {
    if required.is_empty() {
        return Vec::new();
    }
    let set: std::collections::HashSet<&str> =
        actual.iter().map(|s| s.as_str()).collect();
    required
        .iter()
        .filter(|f| !set.contains(f.as_str()))
        .cloned()
        .collect()
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
