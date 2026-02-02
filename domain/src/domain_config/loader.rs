// Domain configuration loader with $ref support for shared entities

use crate::domain_config::types::*;
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
        let content = fs::read_to_string(&domain_path)?;

        // Parse domain.json with $ref resolution
        let mut domain: DomainManifest = serde_json::from_str(&content)?;

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
                    let entity_content = fs::read_to_string(entry.path())?;
                    let entity: Entity = serde_json::from_str(&entity_content)?;

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
            let content = fs::read_to_string(&relationships_path)?;
            let relationships: Vec<Relationship> = serde_json::from_str(&content)?;
            domain.relationships = relationships;
        }

        // Load operations
        let operations_path = base.join(name).join("operations.json");
        if operations_path.exists() {
            let content = fs::read_to_string(&operations_path)?;
            let operations: Vec<Operation> = serde_json::from_str(&content)?;
            domain.common_operations = operations;
        }

        // Load inference rules
        let rules_path = base.join(name).join("inference_rules.json");
        if rules_path.exists() {
            let content = fs::read_to_string(&rules_path)?;
            let rules: Vec<InferenceRule> = serde_json::from_str(&content)?;
            domain.inference_rules = rules;
        }

        // Load troubleshooting patterns
        let troubleshoot_path = base.join(name).join("troubleshooting.json");
        if troubleshoot_path.exists() {
            let content = fs::read_to_string(&troubleshoot_path)?;
            let patterns: Vec<TroubleshootingPattern> = serde_json::from_str(&content)?;
            domain.troubleshooting_patterns = patterns;
        }

        // Load reasoning templates
        let templates_path = base.join(name).join("reasoning_templates.json");
        if templates_path.exists() {
            let content = fs::read_to_string(&templates_path)?;
            let templates: Vec<ReasoningTemplate> = serde_json::from_str(&content)?;
            domain.reasoning_templates = templates;
        }

        Ok(Domain {
            id: domain.domain,
            version: domain.version,
            description: domain.description,
            entities: domain.entities,
            relationships: domain.relationships,
            operations: domain.common_operations,
            inference_rules: domain.inference_rules,
            troubleshooting_patterns: domain.troubleshooting_patterns,
            reasoning_templates: domain.reasoning_templates,
            depends_on: domain.depends_on,
            priority: domain.priority,
            enabled: domain.enabled,
        })
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
                let user_prop_names: std::collections::HashSet<String> = entity
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

                merged_entities.insert(
                    name.clone(),
                    Entity {
                        name: entity.name,
                        description: entity.description,
                        core_properties: merged_props,
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
            entities: merged_entities,
            relationships: user_domain
                .relationships
                .clone()
                .into_iter()
                .chain(prebuilt.relationships.clone().into_iter())
                .collect(),
            operations: merged_ops,
            inference_rules: merged_rules,
            troubleshooting_patterns: user_domain
                .troubleshooting_patterns
                .clone()
                .into_iter()
                .chain(prebuilt.troubleshooting_patterns.clone().into_iter())
                .collect(),
            reasoning_templates: user_domain
                .reasoning_templates
                .clone()
                .into_iter()
                .chain(prebuilt.reasoning_templates.clone().into_iter())
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

                let content = fs::read_to_string(&shared_path)?;
                let mut shared: Entity = serde_json::from_str(&content)?;

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

                return Ok(shared);
            }
        }
        Ok(entity)
    }
}

/// Domain manifest (parsed before entities are loaded)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainManifest {
    #[serde(rename = "domain")]
    pub domain: String,

    #[serde(rename = "version")]
    pub version: String,

    #[serde(rename = "description")]
    pub description: String,

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
