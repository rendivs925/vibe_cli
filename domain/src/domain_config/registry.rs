// Domain registry for loading and querying domains

use crate::domain_config::loader::DomainLoader;
use crate::domain_config::types::*;
use crate::formal_query_language::{FqlAction, FqlConstraint, FqlParser, FqlQuery, FqlTarget};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct OperationSignature {
    fql: FqlQuery,
    #[allow(dead_code)]
    confidence: f32,
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
    #[allow(dead_code)]
    inverted_index: HashMap<String, HashSet<String>>,
    operation_signatures: HashMap<String, OperationSignature>,
    fql_parser: FqlParser,
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

        let mut entities = HashMap::new();
        let mut inverted_index: HashMap<String, HashSet<String>> = HashMap::new();
        let fql_parser = FqlParser::new();
        let mut operation_signatures: HashMap<String, OperationSignature> = HashMap::new();

        for (name, domain) in &domains {
            for (entity_name, entity) in &domain.entities {
                if !entities.contains_key(entity_name) {
                    entities.insert(entity_name.clone(), entity.clone());
                }
                for prop in &entity.core_properties {
                    Self::add_to_index(
                        &mut inverted_index,
                        &prop.name.to_lowercase(),
                        &entity_name,
                    );
                    Self::add_to_index(
                        &mut inverted_index,
                        &prop.meaning.to_lowercase(),
                        &entity_name,
                    );
                }
            }
            for op in &domain.operations {
                Self::add_to_index(&mut inverted_index, &op.id.to_lowercase(), &op.name);
                Self::add_to_index(&mut inverted_index, &op.name.to_lowercase(), &op.id);
                Self::add_to_index(&mut inverted_index, &op.description.to_lowercase(), &op.id);
                for example in &op.examples {
                    for word in example.description.to_lowercase().split_whitespace() {
                        Self::add_to_index(&mut inverted_index, word, &op.id);
                    }
                }

                if let Some(signature) = Self::build_operation_signature(&fql_parser, op) {
                    let key = Self::operation_key(name, &op.id);
                    operation_signatures.insert(key, signature);
                }
            }
        }

        Ok(Self {
            domains,
            entities,
            command_generator: crate::domain_config::command_generator::CommandGenerator::new(),
            inverted_index,
            operation_signatures,
            fql_parser,
        })
    }

    fn add_to_index(index: &mut HashMap<String, HashSet<String>>, word: &str, value: &str) {
        if word.len() > 2 {
            index
                .entry(word.to_string())
                .or_insert_with(HashSet::new)
                .insert(value.to_string());
        }
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
        let query_fql = match self.fql_parser.parse(intent) {
            Some(fql) => fql,
            None => return matches,
        };

        for domain in self.enabled_domains() {
            if let Some(m) = self.match_intent_detailed(domain, intent, &query_fql) {
                matches.push(m);
            }
        }

        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        matches
    }

    /// Query domains by intent (return domains sorted by priority)
    pub fn query_intent(&self, intent: &str) -> Vec<&Domain> {
        self.query_intent_detailed(intent)
            .into_iter()
            .map(|m| self.domains.get(&m.domain).unwrap())
            .collect()
    }

    /// Match an intent to a domain with detailed scoring
    fn match_intent_detailed(
        &self,
        domain: &Domain,
        intent: &str,
        query_fql: &FqlQuery,
    ) -> Option<IntentMatch> {
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

        // Find best matching operation by semantic intent signature
        let mut best_match: Option<(f32, &Operation)> = None;

        for op in &domain.operations {
            let key = Self::operation_key(&domain.id, &op.id);
            let signature = match self.operation_signatures.get(&key) {
                Some(sig) => sig,
                None => continue,
            };

            let confidence = self.score_fql_match(query_fql, &signature.fql);
            if confidence <= 0.0 {
                continue;
            }

            if best_match.map(|(c, _)| confidence > c).unwrap_or(true) {
                best_match = Some((confidence, op));
            }
        }

        if let Some((confidence, op)) = best_match {
            return Some(IntentMatch {
                domain: domain.id.clone(),
                confidence,
                matched_on: MatchSource::OperationIntent,
                matched_value: op.id.clone(),
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
        let resolved = self.resolve_operation(intent, None)?;
        let (domain, op) = self.get_operation(&resolved.op_id)?;
        Some((domain, op, resolved.confidence))
    }

    /// Get operation by ID from any domain
    pub fn get_operation(&self, op_id: &str) -> Option<(&Domain, &Operation)> {
        for domain in self.enabled_domains() {
            if let Some(op) = domain.operations.iter().find(|o| o.id == op_id) {
                return Some((domain, op));
            }
            if let Some(op) = domain
                .operations
                .iter()
                .find(|o| o.name.to_lowercase() == op_id.to_lowercase())
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
        for domain in self.enabled_domains() {
            if let Some(rel) = domain
                .relationships
                .iter()
                .find(|r| r.name.to_lowercase() == name.to_lowercase())
            {
                return Some(rel);
            }
        }
        None
    }

    /// Find related entities through relationships
    pub fn get_related_entities(&self, entity_name: &str) -> Vec<&Entity> {
        let mut related = Vec::new();

        for domain in self.enabled_domains() {
            for rel in &domain.relationships {
                if rel.from_entity.to_lowercase() == entity_name.to_lowercase() {
                    if let Some(e) = self.entities.get(&rel.to_entity) {
                        related.push(e);
                    }
                }
                if rel.to_entity.to_lowercase() == entity_name.to_lowercase() {
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
        for domain in self.enabled_domains() {
            if let Some(pattern) = domain
                .troubleshooting_patterns
                .iter()
                .find(|p| p.id.to_lowercase() == pattern_id.to_lowercase())
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
        for domain in self.enabled_domains() {
            if let Some(template) = domain
                .reasoning_templates
                .iter()
                .find(|t| t.id.to_lowercase() == template_id.to_lowercase())
            {
                return Some((domain, template));
            }
        }
        None
    }

    /// Resolve the best reasoning template for a query using FQL matching
    pub fn resolve_reasoning_template(
        &self,
        query: &str,
        fql: Option<&FqlQuery>,
    ) -> Option<ReasoningTemplate> {
        let query_fql = match fql {
            Some(fql) => fql.clone(),
            None => self.fql_parser.parse(query)?,
        };

        let mut best: Option<(f32, ReasoningTemplate)> = None;

        for domain in self.enabled_domains() {
            for template in &domain.reasoning_templates {
                let Some(template_fql) = self.fql_parser.parse(&template.goal) else {
                    continue;
                };
                let score = self.score_fql_match(&query_fql, &template_fql);
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
    pub fn resolve_operation(
        &self,
        query: &str,
        fql: Option<&FqlQuery>,
    ) -> Option<ResolvedOperation> {
        let query_fql = match fql {
            Some(fql) => fql.clone(),
            None => self.fql_parser.parse(query)?,
        };

        let mut best: Option<(f32, String, String, MatchSource, String)> = None;

        for domain in self.enabled_domains() {
            for op in &domain.operations {
                let key = Self::operation_key(&domain.id, &op.id);
                let signature = match self.operation_signatures.get(&key) {
                    Some(sig) => sig,
                    None => continue,
                };

                let score = self.score_fql_match(&query_fql, &signature.fql);
                if score <= 0.0 {
                    continue;
                }

                if best
                    .as_ref()
                    .map(|(c, _, _, _, _)| score > *c)
                    .unwrap_or(true)
                {
                    best = Some((
                        score,
                        domain.id.clone(),
                        op.id.clone(),
                        MatchSource::OperationIntent,
                        op.name.clone(),
                    ));
                }
            }
        }

        let (confidence, domain_id, op_id, matched_on, matched_value) = best?;
        let operation = self.get_operation(&op_id)?;
        let inputs = self.extract_inputs(operation.1, query, Some(&query_fql));

        Some(ResolvedOperation {
            domain_id,
            op_id,
            confidence,
            inputs,
            matched_on,
            matched_value,
        })
    }

    pub fn match_scores(
        &self,
        query_fql: &FqlQuery,
        op_id: &str,
    ) -> Option<(f32, f32, f32)> {
        let (domain, op) = self.get_operation(op_id)?;
        let key = Self::operation_key(&domain.id, &op.id);
        let signature = self.operation_signatures.get(&key)?;

        let action_score = self.action_similarity(&query_fql.action, &signature.fql.action);
        let target_score = self.target_similarity(&query_fql.target, &signature.fql.target);
        let total_score = self.score_fql_match(query_fql, &signature.fql);
        Some((action_score, target_score, total_score))
    }

    fn operation_key(domain_id: &str, op_id: &str) -> String {
        format!("{}.{}", domain_id, op_id)
    }

    fn build_operation_signature(parser: &FqlParser, op: &Operation) -> Option<OperationSignature> {
        let mut candidates: Vec<(FqlQuery, f32)> = Vec::new();
        let mut texts = Vec::new();

        if !op.intent.trim().is_empty() {
            texts.push(op.intent.clone());
        }
        if !op.name.trim().is_empty() {
            texts.push(op.name.clone());
        }
        if !op.description.trim().is_empty() {
            texts.push(op.description.clone());
        }
        for example in &op.examples {
            if !example.description.trim().is_empty() {
                texts.push(example.description.clone());
            }
        }

        for text in texts {
            if let Some(fql) = parser.parse(&text) {
                let confidence = parser.confidence_score(&text, &fql);
                candidates.push((fql, confidence));
            }
        }

        candidates
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(fql, confidence)| OperationSignature { fql, confidence })
    }

    fn score_fql_match(&self, query: &FqlQuery, op: &FqlQuery) -> f32 {
        let action_score = self.action_similarity(&query.action, &op.action);
        let target_score = self.target_similarity(&query.target, &op.target);

        let mut score = 0.0;
        score += action_score * 0.55;
        score += target_score * 0.40;

        if query.pattern.is_some() && op.pattern.is_some() {
            score += 0.05;
        }

        score.clamp(0.0, 1.0)
    }

    fn action_similarity(&self, a: &FqlAction, b: &FqlAction) -> f32 {
        if a == b {
            return 1.0;
        }

        let group = |action: &FqlAction| match action {
            FqlAction::List | FqlAction::Show | FqlAction::Display => 1,
            FqlAction::Check | FqlAction::Monitor | FqlAction::Verify | FqlAction::Validate => 2,
            FqlAction::Start
            | FqlAction::Stop
            | FqlAction::Restart
            | FqlAction::Enable
            | FqlAction::Disable => 3,
            FqlAction::Find | FqlAction::Search | FqlAction::Locate | FqlAction::Grep => 4,
            _ => 9,
        };

        if group(a) == group(b) {
            0.7
        } else {
            0.0
        }
    }

    fn target_similarity(&self, a: &FqlTarget, b: &FqlTarget) -> f32 {
        if a == b {
            return 1.0;
        }

        let (a_cat, a_val) = self.target_category(a);
        let (b_cat, b_val) = self.target_category(b);

        if a_cat == b_cat {
            if let (Some(av), Some(bv)) = (a_val, b_val) {
                if av == bv {
                    1.0
                } else if av == "*" || bv == "*" {
                    0.8
                } else {
                    0.6
                }
            } else {
                0.7
            }
        } else if a_cat == TargetCategory::Resource || b_cat == TargetCategory::Resource {
            0.4
        } else {
            0.0
        }
    }

    fn target_category(&self, target: &FqlTarget) -> (TargetCategory, Option<String>) {
        match target {
            FqlTarget::File(v) => (TargetCategory::File, Some(v.clone())),
            FqlTarget::Directory(v) => (TargetCategory::Directory, Some(v.clone())),
            FqlTarget::Path(v) => (TargetCategory::Path, Some(v.clone())),
            FqlTarget::Process(v) => (TargetCategory::Process, Some(v.clone())),
            FqlTarget::Service(v) => (TargetCategory::Service, Some(v.clone())),
            FqlTarget::Package(v) => (TargetCategory::Package, Some(v.clone())),
            FqlTarget::User(v) => (TargetCategory::User, Some(v.clone())),
            FqlTarget::Group(v) => (TargetCategory::Group, Some(v.clone())),
            FqlTarget::NetworkInterface(v) => (TargetCategory::Network, Some(v.clone())),
            FqlTarget::Port(_) => (TargetCategory::Network, None),
            FqlTarget::Host(v) => (TargetCategory::Network, Some(v.clone())),
            FqlTarget::Url(v) => (TargetCategory::Network, Some(v.clone())),
            FqlTarget::Memory => (TargetCategory::Memory, None),
            FqlTarget::Cpu => (TargetCategory::Cpu, None),
            FqlTarget::Disk(v) => (TargetCategory::Disk, Some(v.clone())),
            FqlTarget::Filesystem(v) => (TargetCategory::Filesystem, Some(v.clone())),
            FqlTarget::Log(v) => (TargetCategory::Log, Some(v.clone())),
            FqlTarget::Configuration(v) => (TargetCategory::Config, Some(v.clone())),
            FqlTarget::Variable(v) => (TargetCategory::Variable, Some(v.clone())),
            FqlTarget::Resource(v) => (TargetCategory::Resource, Some(v.clone())),
            FqlTarget::Component(v) => (TargetCategory::Component, Some(v.clone())),
            FqlTarget::Entity(v) => (TargetCategory::Entity, Some(v.clone())),
            _ => (TargetCategory::Other, None),
        }
    }

    fn extract_inputs(
        &self,
        op: &Operation,
        query: &str,
        fql: Option<&FqlQuery>,
    ) -> HashMap<String, serde_json::Value> {
        let mut inputs = HashMap::new();
        let query_lower = query.to_lowercase();

        for (name, _) in &op.input_schema {
            if let Some(value) = self.extract_input_value(name, &query_lower, query, fql) {
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
        fql: Option<&FqlQuery>,
    ) -> Option<serde_json::Value> {
        match name {
            "lines" => self
                .extract_lines(query_lower, fql)
                .map(serde_json::Value::from),
            "path" => self.extract_path(query, fql).map(serde_json::Value::from),
            "service" => self
                .extract_service(query_lower, fql)
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
                .extract_filter(query_lower, fql)
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

    fn extract_lines(&self, query_lower: &str, fql: Option<&FqlQuery>) -> Option<u64> {
        if let Some(fql) = fql {
            for constraint in &fql.constraints {
                if let FqlConstraint::Limit(limit) = constraint {
                    return Some(*limit);
                }
            }
        }

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

    fn extract_path(&self, query: &str, fql: Option<&FqlQuery>) -> Option<String> {
        if let Some(fql) = fql {
            match &fql.target {
                FqlTarget::Path(p) | FqlTarget::Directory(p) | FqlTarget::File(p) => {
                    return Some(p.clone())
                }
                _ => {}
            }
        }

        let re = Regex::new(r"(/[^\\s]+)").ok()?;
        re.captures(query)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_service(&self, query_lower: &str, fql: Option<&FqlQuery>) -> Option<String> {
        if let Some(fql) = fql {
            if let FqlTarget::Service(svc) = &fql.target {
                if !svc.is_empty() && svc != "*" {
                    return Some(svc.clone());
                }
            }
        }

        let re = Regex::new(r"service\s+([a-z0-9._-]+)").ok()?;
        if let Some(cap) = re.captures(query_lower) {
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
        let re = Regex::new(r#""([^"]+)"|'([^']+)'"#).ok()?;
        if let Some(cap) = re.captures(query) {
            if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
                return Some(m.as_str().to_string());
            }
        }

        let re = Regex::new(r"(?:contains|containing|match(?:ing)?|grep)\s+([a-z0-9._-]+)").ok()?;
        re.captures(&query.to_lowercase())
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_log(&self, query_lower: &str, query: &str) -> Option<String> {
        if query_lower.contains("syslog") {
            return Some("syslog".to_string());
        }
        if query_lower.contains("messages") {
            return Some("messages".to_string());
        }

        let re = Regex::new(r"/var/log/([a-z0-9._-]+)").ok()?;
        re.captures(query)
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

    fn extract_filter(&self, query_lower: &str, fql: Option<&FqlQuery>) -> Option<String> {
        if let Some(fql) = fql {
            if let FqlTarget::Process(proc_name) = &fql.target {
                if !proc_name.is_empty() && proc_name != "*" {
                    return Some(proc_name.clone());
                }
            }
        }

        let re = Regex::new(r"process(?:es)?\s+([a-z0-9._-]+)").ok()?;
        re.captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_mode(&self, query_lower: &str) -> Option<String> {
        let re = Regex::new(r"\b([0-7]{3,4})\b").ok()?;
        re.captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_owner(&self, query_lower: &str) -> Option<String> {
        let re = Regex::new(r"(?:owner|user)\s+([a-z0-9._-]+)").ok()?;
        re.captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_group(&self, query_lower: &str) -> Option<String> {
        let re = Regex::new(r"(?:group)\s+([a-z0-9._-]+)").ok()?;
        re.captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_target(&self, query_lower: &str) -> Option<String> {
        let re = Regex::new(r"(?:pid|process)\s+([a-z0-9._-]+)").ok()?;
        re.captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
    }

    fn extract_size(&self, query_lower: &str) -> Option<String> {
        let re = Regex::new(r"(\+?\d+(?:\.\d+)?\s*[kmgt]?b)").ok()?;
        re.captures(query_lower)
            .and_then(|cap| cap.get(1).map(|m| m.as_str().replace(' ', "")))
    }

    fn extract_name(&self, query: &str) -> Option<String> {
        let re = Regex::new(r"(?:named|called)\s+([a-z0-9._-]+)").ok()?;
        re.captures(&query.to_lowercase())
            .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetCategory {
    File,
    Directory,
    Path,
    Process,
    Service,
    Package,
    User,
    Group,
    Network,
    Memory,
    Cpu,
    Disk,
    Filesystem,
    Log,
    Config,
    Variable,
    Resource,
    Component,
    Entity,
    Other,
}
