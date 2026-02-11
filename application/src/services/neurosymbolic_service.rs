//! Integrated Neurosymbolic Service
//!
//! Orchestrates all autonomous neurosymbolic components:
//! 1. Safety Validation (hard rules)
//! 2. Command Generation
//! 3. Manpage Validation (syntax checking)
//! 4. Learning Integration (RAG context)
//! 5. Execution with feedback loop

use crate::services::graph_builder::GraphBuilder;
use crate::services::learning_service::LearningService;
use anyhow::anyhow;
use domain::{
    domain_config::{types::GeneratedCommand, DomainRegistry},
    safety::{SafetyEngine, SafetyReport},
    services::{ProofGenerator, SafetyProof},
};
use infrastructure::{
    storage::{
        experience_buffer::{ExperienceBuffer, FailureType},
        induction_engine::InductionEngine,
        knowledge_graph::KnowledgeGraph,
        knowledge_graph_entities::EntityType,
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
    /// Enable safety validation
    pub enable_safety: bool,
    /// Enable manpage validation
    pub enable_manpage_validation: bool,
    /// Enable learning/RAG
    pub enable_learning: bool,
    /// Require confirmation for safety warnings (dangerous commands always blocked)
    pub block_on_safety: bool,
    /// Block on invalid syntax
    pub block_on_invalid_syntax: bool,
}

impl Default for NeurosymbolicConfig {
    fn default() -> Self {
        Self {
            enable_safety: true,
            enable_manpage_validation: true,
            enable_learning: true,
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
    /// Induced rule warnings
    pub induced_warnings: Vec<String>,
}

impl NeurosymbolicResult {
    /// Format result for display
    pub fn format_display(&self) -> String {
        let mut output = String::new();

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

        if !self.induced_warnings.is_empty() {
            output.push_str("Induced Warnings:\n");
            for warning in &self.induced_warnings {
                output.push_str(&format!("  - {}\n", warning));
            }
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
        let (cache_dir, domains_dir, shared_dir) = Self::config_dirs();
        Self::with_paths(config, cache_dir, domains_dir, shared_dir)
    }

    /// Create with explicit paths for testing (avoids environment variable manipulation)
    pub fn with_paths(
        config: NeurosymbolicConfig,
        cache_dir: PathBuf,
        domains_dir: PathBuf,
        shared_dir: PathBuf,
    ) -> Result<Self> {
        let _ = std::fs::create_dir_all(&cache_dir);
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
            Ok(registry) => Some(registry),
            Err(e) => {
                eprintln!("Failed to load domain registry: {:?}", e);
                None
            }
        };

        // Lazy discovery: only populate KG on-demand based on user queries.

        Ok(Self {
            config: config.clone(),
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
        let resolved = registry.resolve_operation(query)?;
        let intent = "system_info".to_string();

        let reasoning = format!(
            "Matched operation '{}' in domain '{}' (confidence {:.0}%)",
            resolved.op_id,
            resolved.domain_id,
            resolved.confidence * 100.0
        );

        Some(IntentSuggestion {
            intent,
            action: None,
            target: None,
            objects: Vec::new(),
            constraints: Vec::new(),
            params: std::collections::HashMap::new(),
            reasoning,
            confidence: resolved.confidence,
        })
    }

    pub fn has_enabled_domains(&self) -> bool {
        self.domain_registry
            .as_ref()
            .map(|registry| !registry.enabled_domains().is_empty())
            .unwrap_or(false)
    }

    pub fn suggest_commands_from_domains(&self, query: &str) -> Option<SymbolicCommandSuggestion> {
        let registry = self.domain_registry.as_ref()?;
        let resolved = registry.resolve_operation(query)?;
        let operation = registry.get_operation(&resolved.op_id)?.1;
        let generated = registry
            .command_generator()
            .generate(operation, &resolved.inputs);
        let mut commands: Vec<String> = generated
            .into_iter()
            .filter(|g| registry.is_tool_available(&g.tool))
            .map(|g| g.command)
            .collect();
        commands.sort();
        commands.dedup();
        if commands.is_empty() {
            return None;
        }

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
                    first_reason.unwrap_or_else(|| {
                        "command not in symbolic operation templates".to_string()
                    }),
                ),
                suggestion: Some(suggestion.clone()),
            }
        }
    }

    pub fn reload_domain_registry(&mut self) -> Result<()> {
        let (_base_dir, domains_dir, shared_dir) = Self::config_dirs();
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
        if self.direct_answer(query).is_some() {
            return Ok(self.direct_answer_result(query));
        }
        let mut trace = vec![];
        trace.push(format!("Processing query: '{}'", query));

        // Step 2: Learning - Get context from past experiences
        let learning_context = self.get_learning_context(query, &mut trace)?;

        self.apply_knowledge_graph_hints(query, &mut trace);

        // Step 3: Generate command (simplified - would use domain config)
        trace.push("Step 2: Generating command...".to_string());
        let reasoning_template = self
            .domain_registry
            .as_ref()
            .and_then(|registry| registry.resolve_reasoning_template(query))
            .map(|template| self.render_reasoning_template(&template, intent));
        let (command, safety_report, syntax_valid, invalid_flags, induced_warnings) =
            self.generate_with_backtracking(query, learning_context.as_deref(), &mut trace)?;
        trace.push(format!("  Selected: {}", command));

        // Step 5.5: Risk Assessment
        let risk_profile = Some(self.risk_scorer.assess(&command, query));

        // Step 5.6: Formal Proof for critical operations
        let safety_proof = risk_profile
            .as_ref()
            .filter(|profile| matches!(profile.risk_level, RiskLevel::High | RiskLevel::Critical))
            .map(|_| {
                self.proof_generator
                    .generate_safety_proof(&command, &safety_report)
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
            induced_warnings,
        })
    }

    fn direct_answer_result(&self, query: &str) -> NeurosymbolicResult {
        NeurosymbolicResult {
            query: query.to_string(),
            safety_report: SafetyReport::safe(""),
            command: String::new(),
            syntax_valid: true,
            invalid_flags: Vec::new(),
            learning_context: None,
            risk_profile: None,
            safety_proof: None,
            reasoning_template: None,
            can_execute: false,
            block_reason: Some("answered from knowledge graph".to_string()),
            trace: vec![
                format!("Processing query: '{}'", query),
                "KnowledgeGraph: direct answer".to_string(),
            ],
            induced_warnings: Vec::new(),
        }
    }

    fn get_learning_context(&self, query: &str, trace: &mut Vec<String>) -> Result<Option<String>> {
        if !self.config.enable_learning {
            return Ok(None);
        }

        trace.push("Step 1: Retrieving learning context...".to_string());
        let context = self.learning_service.get_context_for_query(query)?;
        if context.is_some() {
            trace.push("  Found relevant past experiences".to_string());
        }
        Ok(context)
    }

    fn config_dirs() -> (PathBuf, PathBuf, PathBuf) {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let base_dir = PathBuf::from(home).join(".config/vibe_cli");
        let domains_dir = base_dir.join("domains");
        let shared_dir = base_dir.join("shared_entities");
        (base_dir, domains_dir, shared_dir)
    }

    fn generate_candidates(
        &mut self,
        query: &str,
        learning_context: Option<&str>,
        trace: &mut Vec<String>,
    ) -> Result<Vec<GeneratedCommand>> {
        self.ensure_knowledge_graph_for_query(query);

        let registry = self
            .domain_registry
            .as_ref()
            .ok_or_else(|| anyhow!("Domain registry not available"))?;

        let resolved = registry
            .resolve_operation(query)
            .ok_or_else(|| anyhow!("No neurosymbolic operation match"))?;

        if resolved.confidence < 0.6 {
            return Err(anyhow!("Low confidence neurosymbolic match"));
        }

        trace.push(format!(
            "  Resolved operation: {} ({:.0}%)",
            resolved.op_id,
            resolved.confidence * 100.0
        ));

        let (_, operation) = registry
            .get_operation(&resolved.op_id)
            .ok_or_else(|| anyhow!("Resolved operation not found"))?;

        if let Some(service) = resolved.inputs.get("service").and_then(|v| v.as_str()) {
            if operation.input_schema.contains_key("service") && !self.is_service_known(service) {
                trace.push(format!(
                    "  KnowledgeGraph: service '{}' not found; blocking",
                    service
                ));
                return Err(anyhow!("Unknown service in knowledge graph: {}", service));
            }
        }

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

        Ok(generated)
    }

    fn generate_with_backtracking(
        &mut self,
        query: &str,
        learning_context: Option<&str>,
        trace: &mut Vec<String>,
    ) -> Result<(String, SafetyReport, bool, Vec<String>, Vec<String>)> {
        let mut candidates = self.generate_candidates(query, learning_context, trace)?;

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut first_error: Option<String> = None;

        for candidate in candidates.iter() {
            let mut command = candidate.command.clone();
            let mut induced_warnings = Vec::new();
            trace.push(format!("  Candidate: {}", command));

            if !self.is_tool_available(&candidate.tool) {
                trace.push(format!("  Skipping unavailable tool: {}", candidate.tool));
                continue;
            }

            if let Some(engine) = &self.induction_engine {
                if let Ok(result) = engine.evaluate_command(&command) {
                    command = result.command;
                    induced_warnings.extend(result.warnings);
                    for note in result.notes {
                        trace.push(format!("  Induction: {}", note));
                    }
                    if let Some(reason) = result.blocked_reason {
                        trace.push(format!("  Induction blocked: {}", reason));
                        first_error = first_error.or(Some(reason));
                        continue;
                    }
                }
            }

            let safety_report = if self.config.enable_safety {
                trace.push("  Validating safety...".to_string());
                let report = self.safety_engine.analyze(&command);
                if report.is_blocked() {
                    trace.push(format!("  Blocked by safety: {}", report.summary));
                    first_error = first_error.or(Some(report.summary.clone()));
                    continue;
                }
                report
            } else {
                SafetyReport::safe(&command)
            };

            let (mut syntax_valid, mut invalid_flags) = if self.config.enable_manpage_validation {
                trace.push("  Validating syntax...".to_string());
                let validation = self.syntax_validator.validate(&command);
                let valid = validation.is_valid;
                let invalid = validation.invalid_flags.clone();
                (valid, invalid)
            } else {
                (true, vec![])
            };

            if self.config.enable_manpage_validation && !syntax_valid && !invalid_flags.is_empty() {
                if let Some(cleaned) = self.strip_invalid_flags(&command, &invalid_flags) {
                    trace.push(format!("  Retrying without invalid flags: {}", cleaned));
                    let retry = self.syntax_validator.validate(&cleaned);
                    syntax_valid = retry.is_valid;
                    invalid_flags = retry.invalid_flags.clone();
                    if syntax_valid {
                        command = cleaned;
                    }
                }
            }

            if self.config.block_on_invalid_syntax && !syntax_valid && !invalid_flags.is_empty() {
                first_error =
                    first_error.or(Some(format!("Invalid flags: {}", invalid_flags.join(", "))));
                trace.push("  Syntax invalid; backtracking".to_string());
                continue;
            }

            return Ok((
                command,
                safety_report,
                syntax_valid,
                invalid_flags,
                induced_warnings,
            ));
        }

        Err(anyhow!(
            "No valid command candidates{}",
            first_error
                .as_ref()
                .map(|e| format!(" ({})", e))
                .unwrap_or_default()
        ))
    }

    fn apply_knowledge_graph_hints(&self, query: &str, trace: &mut Vec<String>) {
        self.ensure_knowledge_graph_for_query(query);

        let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) else {
            return;
        };
        let query_lower = query.to_lowercase();
        let services = [
            "nginx", "apache", "mysql", "postgres", "redis", "docker", "ssh",
        ];
        for svc in services {
            if query_lower.contains(svc) {
                if let Ok(Some(entity)) = graph.find_entity(EntityType::Service, svc) {
                    trace.push(format!(
                        "  KnowledgeGraph: service '{}' known (id {})",
                        entity.name, entity.id
                    ));
                }
            }
        }

        if query_lower.contains("distro")
            || query_lower.contains("distribution")
            || query_lower.contains("os release")
        {
            if let Ok(entities) = graph.get_entities_by_type(EntityType::Distribution) {
                if let Some(distro) = entities.first() {
                    let version = distro
                        .attributes
                        .get("version")
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());
                    trace.push(format!(
                        "  KnowledgeGraph: distribution '{}' version {}",
                        distro.name, version
                    ));
                }
            }
        }
    }

    fn is_service_known(&self, service: &str) -> bool {
        self.ensure_knowledge_graph_for_services();

        let known_services = [
            "nginx",
            "apache",
            "mysql",
            "postgres",
            "redis",
            "docker",
            "ssh",
            "postgresql",
            "httpd",
            "php-fpm",
            "celery",
            "gunicorn",
        ];
        if known_services.contains(&service) {
            return true;
        }

        let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) else {
            return true; // do not block if KG unavailable
        };
        if let Ok(Some(_)) = graph.find_entity(EntityType::Service, service) {
            return true;
        }
        false
    }

    fn ensure_knowledge_graph_for_query(&self, query: &str) {
        let query_lower = query.to_lowercase();
        if query_lower.contains("distro")
            || query_lower.contains("distribution")
            || query_lower.contains("os release")
            || query_lower.contains("kernel")
            || query_lower.contains("os ")
        {
            self.ensure_knowledge_graph_for_os();
        }

        if query_lower.contains("service")
            || query_lower.contains("nginx")
            || query_lower.contains("apache")
            || query_lower.contains("mysql")
            || query_lower.contains("postgres")
            || query_lower.contains("redis")
            || query_lower.contains("docker")
            || query_lower.contains("ssh")
        {
            self.ensure_knowledge_graph_for_services();
        }

        if query_lower.contains("container")
            || query_lower.contains("docker")
            || query_lower.contains("podman")
        {
            self.ensure_knowledge_graph_for_containers();
        }

        if query_lower.contains("disk")
            || query_lower.contains("filesystem")
            || query_lower.contains("mount")
            || query_lower.contains("storage")
        {
            self.ensure_knowledge_graph_for_filesystems();
        }

        if query_lower.contains("network")
            || query_lower.contains("interface")
            || query_lower.contains("ip ")
        {
            self.ensure_knowledge_graph_for_network();
        }

        if query_lower.contains("cpu")
            || query_lower.contains("memory")
            || query_lower.contains("ram")
            || query_lower.contains("hardware")
        {
            self.ensure_knowledge_graph_for_hardware();
        }
    }

    fn ensure_knowledge_graph_for_os(&self) {
        let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) else {
            return;
        };
        if let Ok(entities) = graph.get_entities_by_type(EntityType::Distribution) {
            if !entities.is_empty() {
                return;
            }
        }
        if let Ok(builder) = GraphBuilder::new() {
            let _ = builder.discover_os();
        }
    }

    fn ensure_knowledge_graph_for_services(&self) {
        let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) else {
            return;
        };
        if let Ok(entities) = graph.get_entities_by_type(EntityType::Service) {
            if !entities.is_empty() {
                return;
            }
        }
        if let Ok(builder) = GraphBuilder::new() {
            let _ = builder.discover_services();
        }
    }

    fn ensure_knowledge_graph_for_containers(&self) {
        let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) else {
            return;
        };
        if let Ok(entities) = graph.get_entities_by_type(EntityType::Container) {
            if !entities.is_empty() {
                return;
            }
        }
        if let Ok(builder) = GraphBuilder::new() {
            let _ = builder.discover_containers();
        }
    }

    fn ensure_knowledge_graph_for_filesystems(&self) {
        let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) else {
            return;
        };
        if let Ok(entities) = graph.get_entities_by_type(EntityType::Filesystem) {
            if !entities.is_empty() {
                return;
            }
        }
        if let Ok(builder) = GraphBuilder::new() {
            let _ = builder.discover_filesystems();
        }
    }

    fn ensure_knowledge_graph_for_network(&self) {
        let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) else {
            return;
        };
        if let Ok(entities) = graph.get_entities_by_type(EntityType::NetworkInterface) {
            if !entities.is_empty() {
                return;
            }
        }
        if let Ok(builder) = GraphBuilder::new() {
            let _ = builder.discover_network_interfaces();
        }
    }

    fn ensure_knowledge_graph_for_hardware(&self) {
        let Ok(graph) = KnowledgeGraph::new(&self.knowledge_graph_path) else {
            return;
        };
        if let Ok(entities) = graph.get_entities_by_type(EntityType::Cpu) {
            if !entities.is_empty() {
                return;
            }
        }
        if let Ok(builder) = GraphBuilder::new() {
            let _ = builder.discover_hardware();
        }
    }

    fn is_tool_available(&self, tool: &str) -> bool {
        self.domain_registry
            .as_ref()
            .map(|registry| registry.is_tool_available(tool))
            .unwrap_or(true)
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

    pub fn direct_answer(&self, query: &str) -> Option<String> {
        let query_lower = query.to_lowercase();
        let wants_system_info = query_lower.contains("system information")
            || query_lower.contains("system info")
            || query_lower.contains("system status");
        let wants_os = query_lower.contains("distro")
            || query_lower.contains("distribution")
            || query_lower.contains("os release")
            || query_lower.contains("kernel")
            || query_lower.contains("os ");

        if !(wants_system_info || wants_os) {
            return None;
        }

        self.ensure_knowledge_graph_for_query(query);
        let graph = KnowledgeGraph::new(&self.knowledge_graph_path).ok()?;

        let mut lines: Vec<String> = Vec::new();

        if let Ok(distros) = graph.get_entities_by_type(EntityType::Distribution) {
            if let Some(distro) = distros.first() {
                let version = distro
                    .attributes
                    .get("version")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
                let id_like = distro
                    .attributes
                    .get("id_like")
                    .cloned()
                    .unwrap_or_default();
                if id_like.is_empty() {
                    lines.push(format!("Distro: {} {}", distro.name, version));
                } else {
                    lines.push(format!(
                        "Distro: {} {} (like: {})",
                        distro.name, version, id_like
                    ));
                }
            }
        }

        if let Ok(os_entities) = graph.get_entities_by_type(EntityType::OperatingSystem) {
            if let Some(os) = os_entities.first() {
                if let Some(hostname) = os.attributes.get("hostname") {
                    lines.push(format!("Hostname: {}", hostname));
                }
                if let Some(kernel) = os.attributes.get("kernel") {
                    lines.push(format!("Kernel: {}", kernel));
                }
            }
        }

        if wants_system_info {
            if let Ok(cpus) = graph.get_entities_by_type(EntityType::Cpu) {
                if let Some(cpu) = cpus.first() {
                    let model = cpu
                        .attributes
                        .get("model")
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());
                    let cores = cpu
                        .attributes
                        .get("cores")
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());
                    lines.push(format!("CPU: {} ({} cores)", model, cores));
                }
            }

            if let Ok(mem) = graph.get_entities_by_type(EntityType::Memory) {
                if let Some(memory) = mem.first() {
                    if let Some(total) = memory.attributes.get("MemTotal") {
                        lines.push(format!("Memory: {} kB total", total));
                    }
                }
            }
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    fn render_reasoning_template(
        &self,
        template: &domain::domain_config::types::ReasoningTemplate,
        intent: Option<&IntentSignal>,
    ) -> domain::domain_config::types::ReasoningTemplate {
        let mut context: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

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
            self.learning_service.record_success(query, command, None)?;
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
            self.learning_service
                .record_failure(query, command, failure_type, error_message)?;

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
    command.strip_prefix("sudo ").unwrap_or(command).to_string()
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
    let set: std::collections::HashSet<&str> = actual.iter().map(|s| s.as_str()).collect();
    required.iter().all(|f| set.contains(f.as_str()))
}

fn missing_flags(required: &[String], actual: &[String]) -> Vec<String> {
    if required.is_empty() {
        return Vec::new();
    }
    let set: std::collections::HashSet<&str> = actual.iter().map(|s| s.as_str()).collect();
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
    use infrastructure::storage::knowledge_graph::KnowledgeGraph;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};
    use std::time::{SystemTime, UNIX_EPOCH};

    static HOME_MUTEX: Mutex<()> = Mutex::new(());

    struct HomeGuard {
        original: Option<String>,
        temp_home: PathBuf,
        _lock: MutexGuard<'static, ()>,
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            // SAFETY: This is test code that runs with RUST_TEST_THREADS=1
            // or under a mutex. We restore the original HOME value here.
            unsafe {
                if let Some(value) = &self.original {
                    std::env::set_var("HOME", value);
                } else {
                    std::env::remove_var("HOME");
                }
            }
            let _ = fs::remove_dir_all(&self.temp_home);
        }
    }

    fn setup_temp_home_with_domain() -> HomeGuard {
        let lock = HOME_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let original = std::env::var("HOME").ok();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base_home = original.clone().unwrap_or_else(|| "/tmp".to_string());
        let mut temp_home = PathBuf::from(&base_home)
            .join(".config/vibe_cli/test_homes")
            .join(format!("vibe_cli_test_home_{}", nanos));
        if fs::create_dir_all(&temp_home).is_err() {
            temp_home = PathBuf::from("/tmp")
                .join("vibe_cli_test_homes")
                .join(format!("vibe_cli_test_home_{}", nanos));
            fs::create_dir_all(&temp_home).unwrap();
        }
        // SAFETY: This is test code that runs with RUST_TEST_THREADS=1
        // or under a mutex, so no other threads will race on environment access.
        // Production code should use explicit path configuration instead.
        unsafe {
            std::env::set_var("HOME", &temp_home);
        }

        let domain_dir = temp_home.join(".config/vibe_cli/domains/linux");
        let _ = fs::create_dir_all(domain_dir.join("entities"));

        let domain_json = r#"{
            "domain": "linux",
            "version": "1.0.0",
            "description": "Test Linux domain",
            "depends_on": [],
            "priority": 10,
            "enabled": true
        }"#;
        fs::write(domain_dir.join("domain.json"), domain_json).unwrap();

        let operations_json = r#"[
            {
                "op_id": "list_files",
                "name": "list files",
                "description": "list files in directory",
                "intent": "list files",
                "input_schema": {},
                "generators": [
                    {"name": "ls_all", "tool": "ls", "template": "ls -la", "when": []}
                ],
                "examples": [{"description": "list files", "inputs": {}}]
            },
            {
                "op_id": "delete_root",
                "name": "delete root",
                "description": "delete root filesystem",
                "intent": "delete root",
                "input_schema": {},
                "generators": [
                    {"name": "rm_root", "tool": "rm", "template": "rm -rf /", "when": []}
                ],
                "examples": [{"description": "delete root", "inputs": {}}]
            },
            {
                "op_id": "danger_then_safe",
                "name": "danger then safe",
                "description": "two generators to test backtracking",
                "intent": "danger then safe",
                "input_schema": {},
                "generators": [
                    {"name": "rm_root", "tool": "rm", "template": "rm -rf /", "when": [], "preference_score": 1.0},
                    {"name": "ls_all", "tool": "ls", "template": "ls -la", "when": [], "preference_score": 0.5}
                ],
                "examples": [{"description": "danger then safe", "inputs": {}}]
            },
            {
                "op_id": "touch_file",
                "name": "touch file",
                "description": "access /opt to trigger permission learning",
                "intent": "touch file",
                "input_schema": {},
                "generators": [
                    {"name": "ls_opt", "tool": "ls", "template": "ls /opt/demo", "when": []}
                ],
                "examples": [{"description": "touch file", "inputs": {}}]
            },
            {
                "op_id": "check_distribution",
                "name": "check distribution",
                "description": "show linux distribution",
                "intent": "check distribution",
                "input_schema": {},
                "generators": [
                    {"name": "os_release", "tool": "cat", "template": "cat /etc/os-release", "when": []}
                ],
                "examples": [{"description": "show distro", "inputs": {}}]
            },
            {
                "op_id": "service_status",
                "name": "service status",
                "description": "check service status",
                "intent": "service status",
                "input_schema": {
                    "service": {"type": "string", "optional": false},
                    "action": {"type": "string", "optional": false}
                },
                "generators": [
                    {"name": "svc_status", "tool": "systemctl", "template": "systemctl status {{service}}", "when": [{"name": "service"}, {"name": "action"}]}
                ],
                "examples": [{"description": "service status", "inputs": {"service": "nginx", "action": "status"}}]
            }
        ]"#;
        fs::write(domain_dir.join("operations.json"), operations_json).unwrap();

        HomeGuard {
            original,
            temp_home,
            _lock: lock,
        }
    }

    #[test]
    fn test_process_safe_command() {
        let _guard = setup_temp_home_with_domain();
        let mut service = IntegratedNeurosymbolicService::with_config(NeurosymbolicConfig {
            enable_safety: true,
            enable_manpage_validation: false,
            enable_learning: false,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        })
        .unwrap();
        service.reload_domain_registry().unwrap();
        let result = service.process("list files").unwrap();

        assert!(result.can_execute);
        assert!(result.safety_report.is_safe());
    }

    #[test]
    fn test_process_dangerous_command() {
        let _guard = setup_temp_home_with_domain();
        let mut service = IntegratedNeurosymbolicService::with_config(NeurosymbolicConfig {
            enable_safety: true,
            enable_manpage_validation: false,
            enable_learning: false,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        })
        .unwrap();
        service.reload_domain_registry().unwrap();
        let result = service.process("delete root");
        assert!(result.is_err(), "Expected blocked command to error");
    }

    #[test]
    fn test_backtracking_on_safety_block() {
        let _guard = setup_temp_home_with_domain();
        let mut service = IntegratedNeurosymbolicService::with_config(NeurosymbolicConfig {
            enable_safety: true,
            enable_manpage_validation: false,
            enable_learning: false,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        })
        .unwrap();
        service.reload_domain_registry().unwrap();

        let result = service.process("danger then safe").unwrap();
        assert!(result.command.contains("ls -la"));
        assert!(result.can_execute);
    }

    #[test]
    fn test_induced_rule_warnings_and_prefix() {
        let _guard = setup_temp_home_with_domain();

        let home = std::env::var("HOME").unwrap();
        let base = PathBuf::from(home).join(".config/vibe_cli");
        let induction_path = base.join("induction.db");
        let engine = InductionEngine::new(induction_path).unwrap();

        let buffer = ExperienceBuffer::new(base.join("experience.db")).unwrap();
        let _ = buffer.log_failure(
            "s1",
            "touch file",
            "ls /opt/demo",
            FailureType::PermissionDenied,
            Some("permission denied"),
        );
        let _ = buffer.log_failure(
            "s2",
            "touch file",
            "ls /opt/demo",
            FailureType::PermissionDenied,
            Some("permission denied"),
        );
        let _ = buffer.log_failure(
            "s3",
            "touch file",
            "ls /opt/demo",
            FailureType::PermissionDenied,
            Some("permission denied"),
        );

        let patterns = engine.mine_patterns(&buffer).unwrap();
        let graph_path = base.join("knowledge_graph.db");
        let graph = KnowledgeGraph::new(graph_path).unwrap();
        let _ = engine.apply_rules_to_graph(&graph, &patterns);

        let mut service = IntegratedNeurosymbolicService::with_config(NeurosymbolicConfig {
            enable_safety: true,
            enable_manpage_validation: false,
            enable_learning: false,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        })
        .unwrap();
        service.reload_domain_registry().unwrap();

        let result = service.process("touch file").unwrap();
        assert!(result.command.starts_with("sudo "));
        assert!(result
            .induced_warnings
            .iter()
            .any(|w| w.contains("Operations on /opt")));
    }

    #[test]
    fn test_knowledge_graph_hints_in_trace() {
        let _guard = setup_temp_home_with_domain();
        let home = std::env::var("HOME").unwrap();
        let graph_path = PathBuf::from(home).join(".config/vibe_cli/knowledge_graph.db");
        let graph = KnowledgeGraph::new(graph_path).unwrap();
        graph
            .add_entity(EntityType::Service, "nginx", HashMap::new())
            .unwrap();
        graph
            .add_entity(EntityType::OperatingSystem, "linux", HashMap::new())
            .unwrap();

        let mut service = IntegratedNeurosymbolicService::with_config(NeurosymbolicConfig {
            enable_safety: true,
            enable_manpage_validation: false,
            enable_learning: false,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        })
        .unwrap();
        service.reload_domain_registry().unwrap();

        let result = service.process("service nginx status").unwrap();
        assert!(
            result
                .trace
                .iter()
                .any(|t| t.contains("KnowledgeGraph: service 'nginx' known")),
            "Expected knowledge graph hint in trace"
        );
    }

    #[test]
    fn test_distribution_hint_in_trace() {
        let _guard = setup_temp_home_with_domain();
        let home = std::env::var("HOME").unwrap();
        let graph_path = PathBuf::from(home).join(".config/vibe_cli/knowledge_graph.db");
        let graph = KnowledgeGraph::new(graph_path).unwrap();
        let mut distro_attrs = HashMap::new();
        distro_attrs.insert("version".to_string(), "9".to_string());
        graph
            .add_entity(EntityType::Distribution, "TestDistro", distro_attrs)
            .unwrap();
        graph
            .add_entity(EntityType::OperatingSystem, "linux", HashMap::new())
            .unwrap();

        let mut service = IntegratedNeurosymbolicService::with_config(NeurosymbolicConfig {
            enable_safety: true,
            enable_manpage_validation: false,
            enable_learning: false,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        })
        .unwrap();
        service.reload_domain_registry().unwrap();

        let answer = service
            .direct_answer("check distribution")
            .unwrap_or_default();
        assert!(
            answer.contains("TestDistro") && answer.contains("9"),
            "Expected distribution in direct answer"
        );
    }

    #[test]
    fn test_unknown_service_blocked_by_knowledge_graph() {
        let _guard = setup_temp_home_with_domain();
        let home = std::env::var("HOME").unwrap();
        let graph_path = PathBuf::from(home).join(".config/vibe_cli/knowledge_graph.db");
        let graph = KnowledgeGraph::new(graph_path).unwrap();
        graph
            .add_entity(EntityType::OperatingSystem, "linux", HashMap::new())
            .unwrap();

        let mut service = IntegratedNeurosymbolicService::with_config(NeurosymbolicConfig {
            enable_safety: true,
            enable_manpage_validation: false,
            enable_learning: false,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        })
        .unwrap();
        service.reload_domain_registry().unwrap();

        let result = service.process("service status definitelynotaservice123");
        assert!(result.is_err(), "Expected unknown service to be blocked");
    }

    #[test]
    fn test_config_defaults() {
        let config = NeurosymbolicConfig::default();
        assert!(config.enable_safety);
        assert!(config.enable_learning);
    }
}
