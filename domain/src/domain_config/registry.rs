// Domain registry for loading and querying domains

use crate::domain_config::loader::DomainLoader;
use crate::domain_config::types::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

static INTENT_SYNONYMS: &[(&str, &[&str])] = &[
    ("list", &["display", "get", "view", "print", "ls", "ps"]),
    ("show", &["display", "view", "print", "cat"]),
    (
        "processes",
        &["process", "proc", "tasks", "programs", "services"],
    ),
    (
        "files",
        &["file", "filesystem", "fs", "directory", "dir", "folder"],
    ),
    ("disk", &["storage", "space", "df", "du", "partition"]),
    ("memory", &["ram", "mem", "free", "usage"]),
    (
        "network",
        &["net", "connection", "socket", "port", "ss", "netstat"],
    ),
    ("service", &["services", "daemon", "systemd", " systemctl"]),
    ("user", &["users", "account", "passwd", "who"]),
    (
        "permission",
        &["permissions", "chmod", "chown", "acl", "access"],
    ),
    ("search", &["find", "grep", "locate", "whereis", "which"]),
    ("kill", &["terminate", "stop", "end", "pkill", "killall"]),
    (
        "start",
        &["run", "execute", "begin", "launch", "enable", "start"],
    ),
    ("status", &["state", "health", "check", "verify", "info"]),
    ("restart", &["reload", "reboot", "refresh", "reopen"]),
    ("cpu", &["processor", "load", "top", "htop"]),
    ("log", &["logs", "journalctl", "syslog", "tail"]),
    (
        "hardware",
        &[
            "gpu", "graphics", "nvidia", "amd", "vga", "card", "lspci", "lshw", "device", "specs",
        ],
    ),
    (
        "gpu",
        &["graphics", "nvidia", "amd", "display", "vga", "card"],
    ),
];

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
    OperationName,
    OperationDescription,
    OperationExample,
    EntityName,
    Relationship,
    TroubleshootingPattern,
    Keyword,
}

impl std::fmt::Display for MatchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchSource::DomainId => write!(f, "domain ID"),
            MatchSource::DomainDescription => write!(f, "domain description"),
            MatchSource::OperationName => write!(f, "operation name"),
            MatchSource::OperationDescription => write!(f, "operation description"),
            MatchSource::OperationExample => write!(f, "operation example"),
            MatchSource::EntityName => write!(f, "entity name"),
            MatchSource::Relationship => write!(f, "relationship"),
            MatchSource::TroubleshootingPattern => write!(f, "troubleshooting pattern"),
            MatchSource::Keyword => write!(f, "keyword"),
        }
    }
}

impl Default for MatchSource {
    fn default() -> Self {
        MatchSource::Keyword
    }
}

/// Registry for all loaded domains
#[derive(Debug, Clone)]
pub struct DomainRegistry {
    domains: HashMap<String, Domain>,
    entities: HashMap<String, Entity>,
    command_generator: crate::domain_config::command_generator::CommandGenerator,
    inverted_index: HashMap<String, HashSet<String>>,
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
            }
        }

        Ok(Self {
            domains,
            entities,
            command_generator: crate::domain_config::command_generator::CommandGenerator::new(),
            inverted_index,
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
        let expanded_intent = self.expand_synonyms(intent);

        for domain in self.enabled_domains() {
            if let Some(m) = self.match_intent_detailed(domain, intent, &expanded_intent) {
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

    /// Expand intent with synonyms for better matching
    fn expand_synonyms(&self, intent: &str) -> String {
        let mut expanded = intent.to_lowercase();
        for (word, synonyms) in INTENT_SYNONYMS {
            for synonym in *synonyms {
                if intent.to_lowercase().contains(synonym) {
                    for s in *synonyms {
                        if !expanded.contains(s) {
                            expanded.push(' ');
                            expanded.push_str(s);
                        }
                    }
                    break;
                }
            }
        }
        expanded
    }

    /// Match an intent to a domain with detailed scoring
    fn match_intent_detailed(
        &self,
        domain: &Domain,
        intent: &str,
        expanded_intent: &str,
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

        // Priority keyword matching for hardware/system info
        let hardware_keywords = [
            "gpu", "vga", "graphics", "nvidia", "amd", "lspci", "lshw", "hardware", "card",
        ];
        let is_hardware_query = hardware_keywords.iter().any(|k| intent_lower.contains(k));

        // First pass: find best matching operation
        let mut best_match: Option<(f32, &Operation)> = None;

        for op in &domain.operations {
            let op_lower = op.name.to_lowercase();
            let desc_lower = op.description.to_lowercase();

            let confidence = if is_hardware_query && op_lower.contains("hardware") {
                // Boost hardware operations for hardware queries
                0.95
            } else if self.fuzzy_match(&intent_lower, &op_lower) {
                0.85
            } else if self.fuzzy_match(&intent_lower, &desc_lower) {
                0.80
            } else {
                continue;
            };

            if best_match.map(|(c, _)| confidence > c).unwrap_or(true) {
                best_match = Some((confidence, op));
            }

            for example in &op.examples {
                if self.fuzzy_match(&intent_lower, &example.description.to_lowercase()) {
                    let example_confidence = 0.75;
                    if best_match
                        .map(|(c, _)| example_confidence > c)
                        .unwrap_or(true)
                    {
                        best_match = Some((example_confidence, op));
                    }
                }
            }
        }

        if let Some((confidence, op)) = best_match {
            return Some(IntentMatch {
                domain: domain.id.clone(),
                confidence,
                matched_on: MatchSource::OperationName,
                matched_value: op.name.clone(),
            });
        }

        for entity in domain.entities.values() {
            if self.fuzzy_match(&intent_lower, &entity.name.to_lowercase()) {
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

    /// Fuzzy string matching with Levenshtein distance
    fn fuzzy_match(&self, intent: &str, target: &str) -> bool {
        if intent.contains(target) || target.contains(intent) {
            return true;
        }

        let intent_words: Vec<&str> = intent.split_whitespace().collect();
        let target_words: Vec<&str> = target.split_whitespace().collect();

        let intersection_count = intent_words
            .iter()
            .filter(|&&iw| target_words.iter().any(|&tw| iw == tw))
            .count();

        if intersection_count >= 2 {
            return true;
        }

        for &i_word in &intent_words {
            for &t_word in &target_words {
                if i_word.len() > 3 && t_word.len() > 3 {
                    let dist = self.levenshtein_distance(i_word, t_word);
                    let max_len = i_word.len().max(t_word.len());
                    let ratio = (dist as f64) / (max_len as f64);
                    if max_len > 0 && ratio < 0.3 {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn levenshtein_distance(&self, a: &str, b: &str) -> usize {
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();
        let (m, n) = (a_bytes.len(), b_bytes.len());

        if m == 0 {
            return n;
        }
        if n == 0 {
            return m;
        }

        let mut prev_row: Vec<usize> = (0..=n).collect();
        let mut curr_row = vec![0; n + 1];

        for i in 1..=m {
            curr_row[0] = i;
            for j in 1..=n {
                let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
                    0
                } else {
                    1
                };
                curr_row[j] = curr_row[j - 1].min(prev_row[j]).min(prev_row[j - 1] + cost);
            }
            std::mem::swap(&mut prev_row, &mut curr_row);
        }

        prev_row[n]
    }

    /// Match an intent to a domain and return confidence
    fn match_intent(&self, domain: &Domain, intent: &str) -> i32 {
        self.query_intent_detailed(intent)
            .into_iter()
            .find(|m| m.domain == domain.id)
            .map(|m| (m.confidence * 100.0) as i32)
            .unwrap_or(0)
    }

    /// Find best operation for intent
    pub fn find_operation(&self, intent: &str) -> Option<(&Domain, &Operation, f32)> {
        let matches = self.query_intent_detailed(intent);

        for m in matches {
            if m.matched_on == MatchSource::OperationName
                || m.matched_on == MatchSource::OperationExample
            {
                if let Some((domain, op)) = self.get_operation_by_id(&m.matched_value) {
                    return Some((domain, op, m.confidence));
                }
            }
        }

        for domain in self.enabled_domains() {
            for op in &domain.operations {
                if self.fuzzy_match(&intent.to_lowercase(), &op.name.to_lowercase())
                    || self.fuzzy_match(&intent.to_lowercase(), &op.description.to_lowercase())
                {
                    return Some((domain, op, 0.7));
                }
            }
        }

        None
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
}
