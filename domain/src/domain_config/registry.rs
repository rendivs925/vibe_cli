// Domain registry for loading and querying domains

use crate::domain_config::loader::DomainLoader;
use crate::domain_config::types::*;
use std::collections::HashMap;
use std::path::PathBuf;

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

        // Collect all shared entities
        let mut entities = HashMap::new();
        for (name, domain) in &domains {
            for (entity_name, entity) in &domain.entities {
                if !entities.contains_key(entity_name) {
                    entities.insert(entity_name.clone(), entity.clone());
                }
            }
        }

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

    /// Query domains by intent (return domains sorted by priority)
    pub fn query_intent(&self, intent: &str) -> Vec<&Domain> {
        let mut scored: Vec<(i32, &Domain)> = self
            .enabled_domains()
            .iter()
            .map(|domain| {
                let confidence = self.match_intent(domain, intent);
                (confidence, *domain)
            })
            .filter(|(confidence, _)| *confidence > 0)
            .map(|(confidence, domain)| (domain.priority, domain))
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, d)| d).collect()
    }

    /// Match an intent to a domain and return confidence
    fn match_intent(&self, domain: &Domain, intent: &str) -> i32 {
        let intent_lower = intent.to_lowercase();

        // Check domain ID and name
        if intent_lower.contains(&domain.id.to_lowercase())
            || intent_lower.contains(&domain.description.to_lowercase())
        {
            return 100;
        }

        // Check operations (by name and description)
        for op in &domain.operations {
            let op_lower = op.name.to_lowercase();
            let desc_lower = op.description.to_lowercase();
            let intent_lower = intent_lower.clone();

            if intent_lower.contains(&op_lower) || intent_lower.contains(&desc_lower) {
                return 80;
            }

            // Check operation examples
            for example in &op.examples {
                if intent_lower.contains(&example.description.to_lowercase()) {
                    return 70;
                }
            }
        }

        // Check entity names
        for entity in domain.entities.values() {
            if intent_lower.contains(&entity.name.to_lowercase()) {
                return 50;
            }
        }

        // Check relationships
        for rel in &domain.relationships {
            if intent_lower.contains(&rel.name.to_lowercase())
                || intent_lower.contains(&rel.from_entity.to_lowercase())
                || intent_lower.contains(&rel.to_entity.to_lowercase())
            {
                return 40;
            }
        }

        // Check troubleshooting patterns
        for pattern in &domain.troubleshooting_patterns {
            if intent_lower.contains(&pattern.id.to_lowercase()) {
                return 60;
            }
            for symptom in &pattern.symptoms {
                if intent_lower.contains(&symptom.metric.to_lowercase())
                    || intent_lower.contains(&symptom.observation.to_lowercase())
                {
                    return 55;
                }
            }
        }

        0
    }

    /// Get operation by ID from any domain
    pub fn get_operation(&self, op_id: &str) -> Option<(&Domain, &Operation)> {
        for domain in self.enabled_domains() {
            if let Some(op) = domain.operations.iter().find(|o| o.id == op_id) {
                return Some((domain, op));
            }
        }
        None
    }

    /// Get entity by name from any domain
    pub fn get_entity(&self, name: &str) -> Option<&Entity> {
        self.entities.get(name)
    }

    /// Get relationship by name
    pub fn get_relationship(&self, name: &str) -> Option<&Relationship> {
        for domain in self.enabled_domains() {
            if let Some(rel) = domain.relationships.iter().find(|r| r.name == name) {
                return Some(rel);
            }
        }
        None
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

    /// Get troubleshooting pattern by ID
    pub fn get_troubleshooting_pattern(
        &self,
        pattern_id: &str,
    ) -> Option<(&Domain, &TroubleshootingPattern)> {
        for domain in self.enabled_domains() {
            if let Some(pattern) = domain
                .troubleshooting_patterns
                .iter()
                .find(|p| p.id == pattern_id)
            {
                return Some((domain, pattern));
            }
        }
        None
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
                .find(|t| t.id == template_id)
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
