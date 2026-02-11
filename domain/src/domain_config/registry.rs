// Domain registry for loading and querying domains

use crate::domain_config::loader::DomainLoader;
use crate::domain_config::types::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

// Cached regex patterns to avoid recompilation on every extraction call
static REGEX_PATH: OnceLock<Regex> = OnceLock::new();
static REGEX_SERVICE: OnceLock<Regex> = OnceLock::new();
static REGEX_QUOTED_PATTERN: OnceLock<Regex> = OnceLock::new();
static REGEX_CONTAINS_PATTERN: OnceLock<Regex> = OnceLock::new();
static REGEX_LOG_PATH: OnceLock<Regex> = OnceLock::new();
static REGEX_PROCESS_FILTER: OnceLock<Regex> = OnceLock::new();
static REGEX_FILE_MODE: OnceLock<Regex> = OnceLock::new();
static REGEX_OWNER: OnceLock<Regex> = OnceLock::new();
static REGEX_GROUP: OnceLock<Regex> = OnceLock::new();
static REGEX_TARGET: OnceLock<Regex> = OnceLock::new();
static REGEX_SIZE: OnceLock<Regex> = OnceLock::new();
static REGEX_NAME: OnceLock<Regex> = OnceLock::new();

fn get_path_regex() -> &'static Regex {
    REGEX_PATH.get_or_init(|| Regex::new(r"(/[^\\s]+)").expect("Invalid path regex"))
}

fn get_service_regex() -> &'static Regex {
    REGEX_SERVICE
        .get_or_init(|| Regex::new(r"service\s+([a-z0-9._-]+)").expect("Invalid service regex"))
}

fn get_quoted_pattern_regex() -> &'static Regex {
    REGEX_QUOTED_PATTERN
        .get_or_init(|| Regex::new(r#""([^"]+)"|'([^']+)'"#).expect("Invalid quoted pattern regex"))
}

fn get_contains_pattern_regex() -> &'static Regex {
    REGEX_CONTAINS_PATTERN.get_or_init(|| {
        Regex::new(r"(?:contains|containing|match(?:ing)?|grep)\s+([a-z0-9._-]+)")
            .expect("Invalid contains pattern regex")
    })
}

fn get_log_path_regex() -> &'static Regex {
    REGEX_LOG_PATH
        .get_or_init(|| Regex::new(r"/var/log/([a-z0-9._-]+)").expect("Invalid log path regex"))
}

fn get_process_filter_regex() -> &'static Regex {
    REGEX_PROCESS_FILTER.get_or_init(|| {
        Regex::new(r"process(?:es)?\s+([a-z0-9._-]+)").expect("Invalid process filter regex")
    })
}

fn get_file_mode_regex() -> &'static Regex {
    REGEX_FILE_MODE
        .get_or_init(|| Regex::new(r"\b([0-7]{3,4})\b").expect("Invalid file mode regex"))
}

fn get_owner_regex() -> &'static Regex {
    REGEX_OWNER.get_or_init(|| {
        Regex::new(r"(?:owner|user)\s+([a-z0-9._-]+)").expect("Invalid owner regex")
    })
}

fn get_group_regex() -> &'static Regex {
    REGEX_GROUP
        .get_or_init(|| Regex::new(r"(?:group)\s+([a-z0-9._-]+)").expect("Invalid group regex"))
}

fn get_target_regex() -> &'static Regex {
    REGEX_TARGET.get_or_init(|| {
        Regex::new(r"(?:pid|process)\s+([a-z0-9._-]+)").expect("Invalid target regex")
    })
}

fn get_size_regex() -> &'static Regex {
    REGEX_SIZE
        .get_or_init(|| Regex::new(r"(\+?\d+(?:\.\d+)?\s*[kmgt]?b)").expect("Invalid size regex"))
}

fn get_name_regex() -> &'static Regex {
    REGEX_NAME.get_or_init(|| {
        Regex::new(r"(?:named|called)\s+([a-z0-9._-]+)").expect("Invalid name regex")
    })
}

/// Intent match result with detailed scoring
#[derive(Debug, Clone)]
pub struct IntentMatch {
    pub domain: String,
    pub confidence: f32,
    pub matched_on: MatchSource,
    pub matched_value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchSource {
    DomainId,
    DomainDescription,
    OperationIntent,
    OperationName,
    OperationDescription,
    OperationExample,
    EntityName,
    Relationship,
    TroubleshootingPattern,
}

impl std::fmt::Display for MatchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchSource::DomainId => write!(f, "domain ID"),
            MatchSource::DomainDescription => write!(f, "domain description"),
            MatchSource::OperationIntent => write!(f, "operation intent"),
            MatchSource::OperationName => write!(f, "operation name"),
            MatchSource::OperationDescription => write!(f, "operation description"),
            MatchSource::OperationExample => write!(f, "operation example"),
            MatchSource::EntityName => write!(f, "entity name"),
            MatchSource::Relationship => write!(f, "relationship"),
            MatchSource::TroubleshootingPattern => write!(f, "troubleshooting pattern"),
        }
    }
}

impl Default for MatchSource {
    fn default() -> Self {
        MatchSource::OperationIntent
    }
}

/// Registry for all loaded domains
#[derive(Debug, Clone)]
pub struct DomainRegistry {
    domains: HashMap<String, Domain>,
    entities: HashMap<String, Entity>,
    command_generator: crate::domain_config::command_generator::CommandGenerator,
}

impl DomainRegistry {
    /// Create a new registry with default paths
    pub fn new(
        prebuilt_base: PathBuf,
        user_base: PathBuf,
        shared_base: PathBuf,
    ) -> Result<Self, DomainError> {
        let loader = DomainLoader::new(prebuilt_base, user_base.clone(), shared_base.clone());
        let domains = loader.load_all()?;

        let entities = Self::collect_entities(&domains);

        Ok(Self {
            domains,
            entities,
            command_generator: crate::domain_config::command_generator::CommandGenerator::new(),
        })
    }

    /// Get a domain by ID
    pub fn get(&self, id: &str) -> Option<&Domain> {
        self.domains.get(id)
    }

    /// Get all enabled domains
    pub fn enabled_domains(&self) -> Vec<&Domain> {
        self.domains.values().filter(|d| d.enabled).collect()
    }

    /// Query domains by intent with detailed matching
    pub fn query_intent_detailed(&self, intent: &str) -> Vec<IntentMatch> {
        let mut matches = Vec::new();

        for domain in self.enabled_domains() {
            if let Some(m) = self.match_intent_detailed(domain, intent) {
                matches.push(m);
            }
        }

        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches
    }

    /// Query domains by intent (return domains sorted by priority)
    pub fn query_intent(&self, intent: &str) -> Vec<&Domain> {
        self.query_intent_detailed(intent)
            .into_iter()
            .filter_map(|m| self.domains.get(&m.domain))
            .collect()
    }

    /// Match an intent to a domain with detailed scoring
    fn match_intent_detailed(&self, domain: &Domain, intent: &str) -> Option<IntentMatch> {
        let intent_lower = intent.to_lowercase();

        if intent_lower.contains(&domain.id.to_lowercase()) {
            return Some(IntentMatch {
                domain: domain.id.clone(),
                confidence: 1.0,
                matched_on: MatchSource::DomainId,
                matched_value: domain.id.clone(),
            });
        }

        if intent_lower.contains(&domain.description.to_lowercase()) {
            return Some(IntentMatch {
                domain: domain.id.clone(),
                confidence: 0.9,
                matched_on: MatchSource::DomainDescription,
                matched_value: domain.description.clone(),
            });
        }

        // Find best matching operation by fuzzy intent similarity
        if let Some((confidence, op)) = self.best_operation_match(intent, &domain.operations) {
            return Some(IntentMatch {
                domain: domain.id.clone(),
                confidence,
                matched_on: self.best_match_source(intent, op),
                matched_value: op.name.clone(),
            });
        }

        for entity in domain.entities.values() {
            if intent_lower.contains(&entity.name.to_lowercase()) {
                return Some(IntentMatch {
                    domain: domain.id.clone(),
                    confidence: 0.6,
                    matched_on: MatchSource::EntityName,
                    matched_value: entity.name.clone(),
                });
            }
        }

        None
    }

    /// Match an intent to a domain and return confidence
    #[allow(dead_code)]
    fn match_intent(&self, domain: &Domain, intent: &str) -> i32 {
        self.query_intent_detailed(intent)
            .into_iter()
            .find(|m| m.domain == domain.id)
            .map(|m| (m.confidence * 100.0) as i32)
            .unwrap_or(0)
    }

    /// Find best operation for intent
    pub fn find_operation(&self, intent: &str) -> Option<(&Domain, &Operation, f32)> {
        let resolved = self.resolve_operation(intent)?;
        let (domain, op) = self.get_operation(&resolved.op_id)?;
        Some((domain, op, resolved.confidence))
    }

    /// Get operation by ID from any domain
    pub fn get_operation(&self, op_id: &str) -> Option<(&Domain, &Operation)> {
        let op_lower = op_id.to_lowercase();
        for domain in self.enabled_domains() {
            if let Some(op) = domain.operations.iter().find(|o| o.id == op_id) {
                return Some((domain, op));
            }
            if let Some(op) = domain
                .operations
                .iter()
                .find(|o| o.name.to_lowercase() == op_lower)
            {
                return Some((domain, op));
            }
        }
        None
    }

    #[allow(dead_code)]
    fn get_operation_by_id(&self, op_id: &str) -> Option<(&Domain, &Operation)> {
        self.get_operation(op_id)
    }

    /// Get entity by name from any domain
    pub fn get_entity(&self, name: &str) -> Option<&Entity> {
        self.entities.get(name)
    }

    /// Find entities matching a pattern
    pub fn find_entities(&self, pattern: &str) -> Vec<&Entity> {
        let pattern_lower = pattern.to_lowercase();
        self.entities
            .values()
            .filter(|e| {
                e.name.to_lowercase().contains(&pattern_lower)
                    || e.description.to_lowercase().contains(&pattern_lower)
            })
            .collect()
    }

    /// Get relationship by name
    pub fn get_relationship(&self, name: &str) -> Option<&Relationship> {
        let name_lower = name.to_lowercase();
        for domain in self.enabled_domains() {
            if let Some(rel) = domain
                .relationships
                .iter()
                .find(|r| r.name.to_lowercase() == name_lower)
            {
                return Some(rel);
            }
        }
        None
    }

    /// Find related entities through relationships
    pub fn get_related_entities(&self, entity_name: &str) -> Vec<&Entity> {
        let mut related = Vec::new();
        let entity_lower = entity_name.to_lowercase();

        for domain in self.enabled_domains() {
            for rel in &domain.relationships {
                if rel.from_entity.to_lowercase() == entity_lower {
                    if let Some(e) = self.entities.get(&rel.to_entity) {
                        related.push(e);
                    }
                }
                if rel.to_entity.to_lowercase() == entity_lower {
                    if let Some(e) = self.entities.get(&rel.from_entity) {
                        related.push(e);
                    }
                }
            }
        }

        related
    }

    /// Get inference rule by ID
    pub fn get_inference_rule(&self, rule_id: &str) -> Option<(&Domain, &InferenceRule)> {
        for domain in self.enabled_domains() {
            if let Some(rule) = domain.inference_rules.iter().find(|r| r.id == rule_id) {
                return Some((domain, rule));
            }
        }
        None
    }

    /// Apply inference rules to extract additional constraints
    pub fn apply_inference_rules(
        &self,
        context: &HashMap<String, serde_json::Value>,
    ) -> Vec<serde_json::Value> {
        let mut inferences = Vec::new();

        for domain in self.enabled_domains() {
            for rule in &domain.inference_rules {
                let matches = rule.if_.iter().all(|condition| {
                    if let Some(value) = context.get(&condition.entity) {
                        let prop_value = value.as_str().map(|s| s.to_lowercase());
                        if let Some(pv) = prop_value {
                            return pv.contains(
                                &condition
                                    .equals
                                    .as_ref()
                                    .unwrap_or(&serde_json::Value::Null)
                                    .to_string()
                                    .to_lowercase()
                                    .as_str(),
                            );
                        }
                    }
                    false
                });

                if matches {
                    for conclusion in &rule.then {
                        inferences.push(serde_json::json!({
                            "conclude": conclusion.conclusion,
                            "confidence": conclusion.confidence
                        }));
                    }
                }
            }
        }

        inferences
    }

    /// Get troubleshooting pattern by ID
    pub fn get_troubleshooting_pattern(
        &self,
        pattern_id: &str,
    ) -> Option<(&Domain, &TroubleshootingPattern)> {
        let pattern_lower = pattern_id.to_lowercase();
        for domain in self.enabled_domains() {
            if let Some(pattern) = domain
                .troubleshooting_patterns
                .iter()
                .find(|p| p.id.to_lowercase() == pattern_lower)
            {
                return Some((domain, pattern));
            }
        }
        None
    }

    /// Find troubleshooting patterns matching symptoms
    pub fn find_troubleshooting_patterns(
        &self,
        symptoms: &[String],
    ) -> Vec<(&Domain, &TroubleshootingPattern)> {
        let mut matches = Vec::new();

        for domain in self.enabled_domains() {
            for pattern in &domain.troubleshooting_patterns {
                for symptom in &pattern.symptoms {
                    for input_symptom in symptoms {
                        if input_symptom
                            .to_lowercase()
                            .contains(&symptom.observation.to_lowercase())
                            || input_symptom
                                .to_lowercase()
                                .contains(&symptom.metric.to_lowercase())
                        {
                            matches.push((domain, pattern));
                            break;
                        }
                    }
                }
            }
        }

        matches
    }

    /// Get reasoning template by ID
    pub fn get_reasoning_template(
        &self,
        template_id: &str,
    ) -> Option<(&Domain, &ReasoningTemplate)> {
        let template_lower = template_id.to_lowercase();
        for domain in self.enabled_domains() {
            if let Some(template) = domain
                .reasoning_templates
                .iter()
                .find(|t| t.id.to_lowercase() == template_lower)
            {
                return Some((domain, template));
            }
        }
        None
    }

    /// Resolve the best reasoning template for a query using fuzzy matching
    pub fn resolve_reasoning_template(&self, query: &str) -> Option<ReasoningTemplate> {
        let mut best: Option<(f32, ReasoningTemplate)> = None;

        for domain in self.enabled_domains() {
            for template in &domain.reasoning_templates {
                let score = self.score_template_match(query, template);
                if score <= 0.0 {
                    continue;
                }
                if best.as_ref().map(|(c, _)| score > *c).unwrap_or(true) {
                    best = Some((score, template.clone()));
                }
            }
        }

        best.map(|(_, t)| t)
    }

    /// Get command generator
    pub fn command_generator(&self) -> &crate::domain_config::command_generator::CommandGenerator {
        &self.command_generator
    }

    /// List all operation IDs across all domains
    pub fn list_operations(&self) -> Vec<String> {
        let mut ops = Vec::new();
        for domain in self.enabled_domains() {
            for op in &domain.operations {
                ops.push(format!("{}.{}", domain.id, op.id));
            }
        }
        ops
    }

    /// List all domain IDs
    pub fn list_domains(&self) -> Vec<String> {
        self.domains.keys().cloned().collect()
    }

    /// Check if a tool is available
    pub fn is_tool_available(&self, tool: &str) -> bool {
        self.command_generator.is_tool_available(tool)
    }

    /// Get list of unavailable tools
    pub fn unavailable_tools(&self) -> Vec<String> {
        self.command_generator.unavailable_tools()
    }

    /// Resolve the best operation for a query using semantic intent matching
    pub fn resolve_operation(&self, query: &str) -> Option<ResolvedOperation> {
        let (confidence, domain_id, op_id, matched_on, matched_value) =
            self.best_operation_match_across_domains(query)?;
        let operation = self.get_operation(&op_id)?;
        let inputs = self.extract_inputs(operation.1, query);

        Some(ResolvedOperation {
            domain_id,
            op_id,
            confidence,
            inputs,
            matched_on,
            matched_value,
        })
    }
    fn extract_inputs(&self, op: &Operation, query: &str) -> HashMap<String, serde_json::Value> {
        let mut inputs = HashMap::new();
        let query_lower = query.to_lowercase();

        for (name, _) in &op.input_schema {
            if let Some(value) = self.extract_input_value(name, &query_lower, query) {
                inputs.insert(name.clone(), value);
            }
        }

        inputs
    }

    fn extract_input_value(
        &self,
        name: &str,
        query_lower: &str,
        query: &str,
    ) -> Option<serde_json::Value> {
        match name {
            "lines" => self.extract_lines(query_lower).map(serde_json::Value::from),
            "path" => self.extract_path(query).map(serde_json::Value::from),
            "service" => self
                .extract_service(query_lower)
                .map(serde_json::Value::from),
            "action" => self
                .extract_action(query_lower)
                .map(serde_json::Value::from),
            "pattern" => self.extract_pattern(query).map(serde_json::Value::from),
            "log" => self
                .extract_log(query_lower, query)
                .map(serde_json::Value::from),
            "protocol" => self
                .extract_protocol(query_lower)
                .map(serde_json::Value::from),
            "filter" => self
                .extract_filter(query_lower)
                .map(serde_json::Value::from),
            "mode" => self.extract_mode(query_lower).map(serde_json::Value::from),
            "owner" => self.extract_owner(query_lower).map(serde_json::Value::from),
            "group" => self.extract_group(query_lower).map(serde_json::Value::from),
            "target" => self
                .extract_target(query_lower)
                .map(serde_json::Value::from),
            "size" => self.extract_size(query_lower).map(serde_json::Value::from),
            "name" => self.extract_name(query).map(serde_json::Value::from),
            _ => None,
        }
    }

    fn extract_lines(&self, query_lower: &str) -> Option<u64> {
        let patterns = [
            r"(?:last|tail|recent|past|previous)\s+(\d+)\s+lines?",
            r"-n\s*(\d+)",
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(query_lower) {
                    if let Ok(val) = cap[1].parse::<u64>() {
                        return Some(val);
                    }
                }
            }
        }

        None
    }

    fn extract_path(&self, query: &str) -> Option<String> {
        get_path_regex()
            .captures(query)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_service(&self, query_lower: &str) -> Option<String> {
        if let Some(cap) = get_service_regex().captures(query_lower) {
            return Some(cap[1].to_string());
        }

        for svc in [
            "nginx", "apache", "mysql", "postgres", "redis", "docker", "ssh",
        ] {
            if query_lower.contains(svc) {
                return Some(svc.to_string());
            }
        }

        None
    }

    fn extract_action(&self, query_lower: &str) -> Option<String> {
        for action in [
            "status", "start", "stop", "restart", "reload", "enable", "disable", "list",
        ] {
            if query_lower.contains(action) {
                return Some(action.to_string());
            }
        }
        None
    }

    fn extract_pattern(&self, query: &str) -> Option<String> {
        if let Some(cap) = get_quoted_pattern_regex().captures(query) {
            if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
                return Some(m.as_str().to_string());
            }
        }

        get_contains_pattern_regex()
            .captures(&query.to_lowercase())
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_log(&self, query_lower: &str, query: &str) -> Option<String> {
        if query_lower.contains("syslog") {
            return Some("syslog".to_string());
        }
        if query_lower.contains("messages") {
            return Some("messages".to_string());
        }

        get_log_path_regex()
            .captures(query)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_protocol(&self, query_lower: &str) -> Option<String> {
        if query_lower.contains("tcp") {
            return Some("tcp".to_string());
        }
        if query_lower.contains("udp") {
            return Some("udp".to_string());
        }
        None
    }

    fn extract_filter(&self, query_lower: &str) -> Option<String> {
        get_process_filter_regex()
            .captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_mode(&self, query_lower: &str) -> Option<String> {
        get_file_mode_regex()
            .captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_owner(&self, query_lower: &str) -> Option<String> {
        get_owner_regex()
            .captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_group(&self, query_lower: &str) -> Option<String> {
        get_group_regex()
            .captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_target(&self, query_lower: &str) -> Option<String> {
        get_target_regex()
            .captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_size(&self, query_lower: &str) -> Option<String> {
        get_size_regex()
            .captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().replace(' ', "")))
    }

    fn extract_name(&self, query: &str) -> Option<String> {
        get_name_regex()
            .captures(&query.to_lowercase())
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn score_text_match(&self, query: &str, op: &Operation) -> f32 {
        let query_tokens = Self::tokenize(query);
        if query_tokens.is_empty() {
            return 0.0;
        }

        let op_tokens = self.operation_tokens(op);
        if op_tokens.is_empty() {
            return 0.0;
        }

        let overlap = query_tokens
            .iter()
            .filter(|t| op_tokens.contains(*t))
            .count();
        if overlap == 0 {
            return 0.0;
        }

        let recall = overlap as f32 / query_tokens.len() as f32;
        let precision = overlap as f32 / op_tokens.len() as f32;
        let mut score = (recall * 0.7) + (precision * 0.3);

        let query_lower = query.to_lowercase();
        if !op.name.is_empty() && query_lower.contains(&op.name.to_lowercase()) {
            score += 0.2;
        } else if !op.id.is_empty() && query_lower.contains(&op.id.to_lowercase()) {
            score += 0.15;
        } else if !op.intent.is_empty() && query_lower.contains(&op.intent.to_lowercase()) {
            score += 0.1;
        }

        score.clamp(0.0, 1.0)
    }

    fn score_template_match(&self, query: &str, template: &ReasoningTemplate) -> f32 {
        let query_tokens = Self::tokenize(query);
        if query_tokens.is_empty() {
            return 0.0;
        }

        let template_tokens: HashSet<String> = Self::tokenize(&template.goal).into_iter().collect();
        if template_tokens.is_empty() {
            return 0.0;
        }

        let overlap = query_tokens
            .iter()
            .filter(|t| template_tokens.contains(*t))
            .count();
        if overlap == 0 {
            return 0.0;
        }

        let recall = overlap as f32 / query_tokens.len() as f32;
        let precision = overlap as f32 / template_tokens.len() as f32;
        let mut score = (recall * 0.7) + (precision * 0.3);

        let query_lower = query.to_lowercase();
        if !template.goal.is_empty() && query_lower.contains(&template.goal.to_lowercase()) {
            score += 0.15;
        }

        score.clamp(0.0, 1.0)
    }

    fn best_match_source(&self, query: &str, op: &Operation) -> MatchSource {
        let query_lower = query.to_lowercase();
        if !op.name.is_empty() && query_lower.contains(&op.name.to_lowercase()) {
            return MatchSource::OperationName;
        }
        if !op.id.is_empty() && query_lower.contains(&op.id.to_lowercase()) {
            return MatchSource::OperationName;
        }
        if !op.intent.is_empty() && query_lower.contains(&op.intent.to_lowercase()) {
            return MatchSource::OperationIntent;
        }
        if !op.description.is_empty() && query_lower.contains(&op.description.to_lowercase()) {
            return MatchSource::OperationDescription;
        }
        if op.examples.iter().any(|ex| {
            !ex.description.is_empty() && query_lower.contains(&ex.description.to_lowercase())
        }) {
            return MatchSource::OperationExample;
        }
        MatchSource::OperationIntent
    }

    fn operation_tokens(&self, op: &Operation) -> HashSet<String> {
        let mut tokens: HashSet<String> = HashSet::new();
        for text in [
            op.id.as_str(),
            op.name.as_str(),
            op.intent.as_str(),
            op.description.as_str(),
        ] {
            for token in Self::tokenize(text) {
                tokens.insert(token);
            }
        }
        for example in &op.examples {
            for token in Self::tokenize(&example.description) {
                tokens.insert(token);
            }
        }
        tokens
    }

    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| t.len() > 2)
            .map(|t| t.to_string())
            .collect()
    }

    fn collect_entities(domains: &HashMap<String, Domain>) -> HashMap<String, Entity> {
        let mut entities = HashMap::new();
        for domain in domains.values() {
            for (entity_name, entity) in &domain.entities {
                entities
                    .entry(entity_name.clone())
                    .or_insert_with(|| entity.clone());
            }
        }
        entities
    }

    fn best_operation_match<'a>(
        &self,
        query: &str,
        operations: &'a [Operation],
    ) -> Option<(f32, &'a Operation)> {
        let mut best: Option<(f32, &'a Operation)> = None;
        for op in operations {
            let confidence = self.score_text_match(query, op);
            if confidence <= 0.0 {
                continue;
            }
            if best.map(|(c, _)| confidence > c).unwrap_or(true) {
                best = Some((confidence, op));
            }
        }
        best
    }

    fn best_operation_match_across_domains(
        &self,
        query: &str,
    ) -> Option<(f32, String, String, MatchSource, String)> {
        let mut best: Option<(f32, String, String, MatchSource, String)> = None;
        for domain in self.enabled_domains() {
            if let Some((score, op)) = self.best_operation_match(query, &domain.operations) {
                if best
                    .as_ref()
                    .map(|(c, _, _, _, _)| score > *c)
                    .unwrap_or(true)
                {
                    best = Some((
                        score,
                        domain.id.clone(),
                        op.id.clone(),
                        self.best_match_source(query, op),
                        op.name.clone(),
                    ));
                }
            }
        }
        best
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedOperation {
    pub domain_id: String,
    pub op_id: String,
    pub confidence: f32,
    pub inputs: HashMap<String, serde_json::Value>,
    pub matched_on: MatchSource,
    pub matched_value: String,
}
