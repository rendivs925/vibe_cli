// Domain configuration loader with $ref support for shared entities

use crate::domain_config::types::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Loader for domain configuration files
pub struct DomainLoader {
    prebuilt_base: PathBuf,
    user_base: PathBuf,
    shared_base: PathBuf,
}

impl DomainLoader {
    pub fn new(prebuilt_base: PathBuf, user_base: PathBuf, shared_base: PathBuf) -> Self {
        Self {
            prebuilt_base,
            user_base,
            shared_base,
        }
    }

    /// Load all domains (user overrides prebuilt)
    pub fn load_all(&self) -> Result<HashMap<String, Domain>, DomainError> {
        let mut domains = HashMap::new();

        // Load prebuilt domains (optional, may not exist)
        if self.prebuilt_base.exists() {
            for entry in fs::read_dir(&self.prebuilt_base)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    if let Some(name) = entry
                        .path()
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                    {
                        let domain = self.load_domain(&name, true)?;
                        domains.insert(name, domain);
                    }
                }
            }
        }

        // Load user overrides (optional, may not exist)
        if self.user_base.exists() {
            for entry in fs::read_dir(&self.user_base)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    if let Some(name) = entry
                        .path()
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                    {
                        if let Some(prebuilt) = domains.get(&name) {
                            // Merge user override with prebuilt
                            let merged = self.merge_domains(prebuilt, &entry.path())?;
                            domains.insert(name, merged);
                        } else {
                            // New domain from user
                            let domain = self.load_domain(&name, false)?;
                            domains.insert(name, domain);
                        }
                    }
                }
            }
        }

        Ok(domains)
    }

    /// Load a single domain
    fn load_domain(&self, name: &str, is_prebuilt: bool) -> Result<Domain, DomainError> {
        let base = if is_prebuilt {
            &self.prebuilt_base
        } else {
            &self.user_base
        };

        let domain_path = base.join(name).join("domain.json");
        // Parse domain.json with $ref resolution
        let mut domain: DomainManifest = read_json_file(&domain_path)?;

        // Load entities
        let entities_path = base.join(name).join("entities");
        if entities_path.exists() {
            for entry in fs::read_dir(&entities_path)? {
                let entry = entry?;
                if entry.path().is_file()
                    && entry
                        .path()
                        .extension()
                        .map(|e| e == "json")
                        .unwrap_or(false)
                {
                    let entity: Entity = read_json_file(&entry.path())?;

                    // Handle $ref to shared entities
                    let entity = if entity.extends.is_some() {
                        self.resolve_entity_ref(entity)?
                    } else {
                        entity
                    };

                    domain.entities.insert(entity.name.clone(), entity);
                }
            }
        }

        // Load relationships
        let relationships_path = base.join(name).join("relationships.json");
        if relationships_path.exists() {
            let relationships: Vec<Relationship> = read_json_file(&relationships_path)?;
            domain.relationships = relationships;
        }

        // Load operations
        let operations_path = base.join(name).join("operations.json");
        if operations_path.exists() {
            let operations: Vec<Operation> = read_json_file(&operations_path)?;
            domain.common_operations = operations;
        }

        // Load inference rules
        let rules_path = base.join(name).join("inference_rules.json");
        if rules_path.exists() {
            let rules: Vec<InferenceRule> = read_json_file(&rules_path)?;
            domain.inference_rules = rules;
        }

        // Load troubleshooting patterns
        let troubleshoot_path = base.join(name).join("troubleshooting.json");
        if troubleshoot_path.exists() {
            let patterns: Vec<TroubleshootingPattern> = read_json_file(&troubleshoot_path)?;
            domain.troubleshooting_patterns = patterns;
        }

        // Load reasoning templates
        let templates_path = base.join(name).join("reasoning_templates.json");
        if templates_path.exists() {
            let templates: Vec<ReasoningTemplate> = read_json_file(&templates_path)?;
            domain.reasoning_templates = templates;
        }

        let domain = Domain {
            id: domain.domain,
            version: domain.version,
            description: domain.description,
            author: domain.author,
            tags: domain.tags,
            entities: domain.entities,
            relationships: domain.relationships,
            operations: domain.common_operations,
            inference_rules: domain.inference_rules,
            troubleshooting_patterns: domain.troubleshooting_patterns,
            reasoning_templates: domain.reasoning_templates,
            depends_on: domain.depends_on,
            priority: domain.priority,
            enabled: domain.enabled,
        };

        self.validate_domain(&domain)?;

        Ok(domain)
    }

    /// Merge user domain with prebuilt (user takes precedence)
    fn merge_domains(&self, prebuilt: &Domain, user_path: &Path) -> Result<Domain, DomainError> {
        let user_domain = self.load_domain(
            &user_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            false,
        )?;

        // Merge entities (user can add new or override)
        let mut merged_entities = prebuilt.entities.clone();
        for (name, entity) in user_domain.entities {
            if let Some(prebuilt_entity) = merged_entities.get(&name) {
                // Merge properties (user can add/override)
                let mut merged_props = prebuilt_entity.core_properties.clone();
                let _user_prop_names: std::collections::HashSet<String> = entity
                    .core_properties
                    .iter()
                    .map(|p| p.name.clone())
                    .collect();

                for prop in entity.core_properties {
                    if let Some(existing) = merged_props.iter_mut().find(|p| p.name == prop.name) {
                        *existing = prop; // User override
                    } else {
                        merged_props.push(prop); // New property
                    }
                }

                let mut merged_derived = prebuilt_entity.derived_properties.clone();
                for derived in entity.derived_properties {
                    if let Some(existing) =
                        merged_derived.iter_mut().find(|p| p.name == derived.name)
                    {
                        *existing = derived;
                    } else {
                        merged_derived.push(derived);
                    }
                }

                merged_entities.insert(
                    name.clone(),
                    Entity {
                        name: entity.name,
                        description: entity.description,
                        core_properties: merged_props,
                        derived_properties: merged_derived,
                        extends: entity.extends.or(prebuilt_entity.extends.clone()),
                    },
                );
            } else {
                // New entity from user
                merged_entities.insert(name, entity);
            }
        }

        // Merge operations (append user operations to prebuilt)
        let mut merged_ops = prebuilt.operations.clone();
        let prebuilt_op_ids: std::collections::HashSet<String> =
            prebuilt.operations.iter().map(|o| o.id.clone()).collect();
        for op in user_domain.operations {
            if !prebuilt_op_ids.contains(&op.id) {
                merged_ops.push(op);
            }
        }

        // Merge inference rules
        let mut merged_rules = prebuilt.inference_rules.clone();
        let prebuilt_rule_ids: std::collections::HashSet<String> = prebuilt
            .inference_rules
            .iter()
            .map(|r| r.id.clone())
            .collect();
        for rule in user_domain.inference_rules {
            if !prebuilt_rule_ids.contains(&rule.id) {
                merged_rules.push(rule);
            }
        }

        // Use user domain for other fields
        Ok(Domain {
            id: user_domain.id,
            version: user_domain.version,
            description: user_domain.description,
            author: user_domain.author.or(prebuilt.author.clone()),
            tags: if user_domain.tags.is_empty() {
                prebuilt.tags.clone()
            } else {
                user_domain.tags
            },
            entities: merged_entities,
            relationships: user_domain
                .relationships
                .clone()
                .into_iter()
                .chain(prebuilt.relationships.clone())
                .collect(),
            operations: merged_ops,
            inference_rules: merged_rules,
            troubleshooting_patterns: user_domain
                .troubleshooting_patterns
                .clone()
                .into_iter()
                .chain(prebuilt.troubleshooting_patterns.clone())
                .collect(),
            reasoning_templates: user_domain
                .reasoning_templates
                .clone()
                .into_iter()
                .chain(prebuilt.reasoning_templates.clone())
                .collect(),
            depends_on: user_domain.depends_on,
            priority: user_domain.priority,
            enabled: user_domain.enabled,
        })
    }

    /// Resolve $ref to shared entities
    fn resolve_entity_ref(&self, entity: Entity) -> Result<Entity, DomainError> {
        if let Some(ref shared_name) = entity.extends {
            if shared_name.starts_with("shared://") {
                let filename = shared_name.strip_prefix("shared://").unwrap();
                let shared_path = self.shared_base.join(format!("{}.json", filename));

                if !shared_path.exists() {
                    return Err(DomainError::InvalidReference(format!(
                        "Shared entity not found: {} (tried: {:?})",
                        shared_name, shared_path
                    )));
                }

                let mut shared: Entity = read_json_file(&shared_path)?;

                // Merge user's additions into shared entity
                shared.description = entity.description;
                for prop in entity.core_properties {
                    if let Some(existing) = shared
                        .core_properties
                        .iter_mut()
                        .find(|p| p.name == prop.name)
                    {
                        *existing = prop;
                    } else {
                        shared.core_properties.push(prop);
                    }
                }

                for derived in entity.derived_properties {
                    if let Some(existing) = shared
                        .derived_properties
                        .iter_mut()
                        .find(|p| p.name == derived.name)
                    {
                        *existing = derived;
                    } else {
                        shared.derived_properties.push(derived);
                    }
                }

                return Ok(shared);
            }
        }
        Ok(entity)
    }
}

fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T, DomainError> {
    let content = fs::read_to_string(path)?;
    let parsed = serde_json::from_str(&content)?;
    Ok(parsed)
}

/// Domain manifest (parsed before entities are loaded)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainManifest {
    #[serde(rename = "domain")]
    pub domain: String,

    #[serde(rename = "version")]
    pub version: String,

    #[serde(rename = "description")]
    pub description: String,

    #[serde(rename = "author", default)]
    pub author: Option<String>,

    #[serde(rename = "tags", default)]
    pub tags: Vec<String>,

    #[serde(rename = "entities", default)]
    pub entities: HashMap<String, Entity>,

    #[serde(rename = "relationships", default)]
    pub relationships: Vec<Relationship>,

    #[serde(rename = "common_operations", default)]
    pub common_operations: Vec<Operation>,

    #[serde(rename = "inference_rules", default)]
    pub inference_rules: Vec<InferenceRule>,

    #[serde(rename = "troubleshooting_patterns", default)]
    pub troubleshooting_patterns: Vec<TroubleshootingPattern>,

    #[serde(rename = "reasoning_templates", default)]
    pub reasoning_templates: Vec<ReasoningTemplate>,

    #[serde(rename = "depends_on", default)]
    pub depends_on: Vec<String>,

    #[serde(rename = "priority", default = "default_priority")]
    pub priority: i32,

    #[serde(rename = "enabled", default = "default_enabled")]
    pub enabled: bool,
}

fn default_priority() -> i32 {
    10
}

fn default_enabled() -> bool {
    true
}

impl DomainLoader {
    fn validate_domain(&self, domain: &Domain) -> Result<(), DomainError> {
        self.require_non_empty("domain.domain", &domain.id)?;
        self.require_non_empty("domain.version", &domain.version)?;
        self.require_non_empty("domain.description", &domain.description)?;

        for (name, entity) in &domain.entities {
            self.require_non_empty(&format!("entity.{}.name", name), &entity.name)?;
            self.require_non_empty(&format!("entity.{}.description", name), &entity.description)?;
            if entity.core_properties.is_empty() {
                return Err(DomainError::MissingField(format!(
                    "entity.{}.core_properties",
                    name
                )));
            }
            for prop in &entity.core_properties {
                self.require_non_empty("property.name", &prop.name)?;
                self.require_non_empty("property.type", &prop.type_)?;
                self.require_non_empty("property.meaning", &prop.meaning)?;
            }
            for prop in &entity.derived_properties {
                self.require_non_empty("derived_property.name", &prop.name)?;
                self.require_non_empty("derived_property.expression", &prop.expression)?;
            }
        }

        for rel in &domain.relationships {
            self.require_non_empty("relationship.name", &rel.name)?;
            self.require_non_empty("relationship.type", &rel.rel_type)?;
            self.require_non_empty("relationship.from", &rel.from_entity)?;
            self.require_non_empty("relationship.to", &rel.to_entity)?;
            self.require_non_empty("relationship.meaning", &rel.meaning)?;
        }

        for op in &domain.operations {
            self.require_non_empty("operation.op_id", &op.id)?;
            self.require_non_empty("operation.name", &op.name)?;
            if op.generators.is_empty() {
                return Err(DomainError::MissingField(format!(
                    "operation.{}.generators",
                    op.id
                )));
            }
            for gen in &op.generators {
                self.require_non_empty("generator.name", &gen.name)?;
                self.require_non_empty("generator.tool", &gen.tool)?;
                self.require_non_empty("generator.template", &gen.template)?;
                for req in &gen.when {
                    self.require_non_empty("generator.when.name", &req.name)?;
                }
                for req in &gen.optional {
                    self.require_non_empty("generator.optional.name", &req.name)?;
                }
            }
            for ex in &op.examples {
                self.require_non_empty("operation.example.description", &ex.description)?;
            }
            for (name, spec) in &op.input_schema {
                self.require_non_empty(&format!("input_schema.{}.type", name), &spec.type_)?;
            }
        }

        for rule in &domain.inference_rules {
            self.require_non_empty("inference.rule_id", &rule.id)?;
            if rule.if_.is_empty() {
                return Err(DomainError::MissingField(format!(
                    "inference.{}.if",
                    rule.id
                )));
            }
            if rule.then.is_empty() {
                return Err(DomainError::MissingField(format!(
                    "inference.{}.then",
                    rule.id
                )));
            }
            for cond in &rule.if_ {
                self.require_non_empty("rule_condition.entity", &cond.entity)?;
                self.require_non_empty("rule_condition.prop", &cond.prop)?;
                let has_condition = cond.equals.is_some()
                    || cond.gt.is_some()
                    || cond.lt.is_some()
                    || cond.gte.is_some()
                    || cond.matches.is_some();
                if !has_condition {
                    return Err(DomainError::MissingField(format!(
                        "rule_condition (entity={}, prop={}) has no predicate",
                        cond.entity, cond.prop
                    )));
                }
            }
            for conclusion in &rule.then {
                self.require_non_empty("rule_conclusion.conclude", &conclusion.conclusion)?;
                if !(0.0..=1.0).contains(&conclusion.confidence) {
                    return Err(DomainError::InvalidField(format!(
                        "rule_conclusion.confidence out of range: {}",
                        conclusion.confidence
                    )));
                }
            }
        }

        for pattern in &domain.troubleshooting_patterns {
            self.require_non_empty("troubleshooting.pattern_id", &pattern.id)?;
            if pattern.symptoms.is_empty() {
                return Err(DomainError::MissingField(format!(
                    "troubleshooting.{}.symptoms",
                    pattern.id
                )));
            }
            if pattern.checks.is_empty() {
                return Err(DomainError::MissingField(format!(
                    "troubleshooting.{}.checks",
                    pattern.id
                )));
            }
            for symptom in &pattern.symptoms {
                if symptom.metric.trim().is_empty() && symptom.observation.trim().is_empty() {
                    return Err(DomainError::MissingField(
                        "symptom.metric or symptom.observation".to_string(),
                    ));
                }
            }
            for check in &pattern.checks {
                self.require_non_empty("troubleshoot_check.step", &check.step)?;
                if check.command.trim().is_empty() && check.commands.is_empty() {
                    return Err(DomainError::MissingField(
                        "troubleshoot_check.command or commands".to_string(),
                    ));
                }
            }
            for action in &pattern.actions {
                self.require_non_empty("troubleshoot_action.action", &action.action)?;
            }
            for cause in &pattern.likely_causes {
                self.require_non_empty("likely_cause.cause", &cause.cause)?;
                if let Some(prob) = cause.probability {
                    if !(0.0..=1.0).contains(&prob) {
                        return Err(DomainError::InvalidField(format!(
                            "likely_cause.probability out of range: {}",
                            prob
                        )));
                    }
                }
            }
        }

        for template in &domain.reasoning_templates {
            self.require_non_empty("reasoning.template_id", &template.id)?;
            self.require_non_empty("reasoning.goal", &template.goal)?;
            if template.steps.is_empty() {
                return Err(DomainError::MissingField(format!(
                    "reasoning.{}.steps",
                    template.id
                )));
            }
            for input in &template.inputs {
                self.require_non_empty("template_input.name", &input.name)?;
                self.require_non_empty("template_input.type", &input.type_)?;
            }
            for step in &template.steps {
                if step.step <= 0 {
                    return Err(DomainError::InvalidField(format!(
                        "template_step.step must be > 0 (template {})",
                        template.id
                    )));
                }
                self.require_non_empty("template_step.check", &step.check)?;
            }
            for out in &template.outputs {
                self.require_non_empty("template_output.name", &out.name)?;
                self.require_non_empty("template_output.type", &out.type_)?;
            }
        }

        Ok(())
    }

    fn require_non_empty(&self, field: &str, value: &str) -> Result<(), DomainError> {
        if value.trim().is_empty() {
            Err(DomainError::MissingField(field.to_string()))
        } else {
            Ok(())
        }
    }
}
