// Domain configuration types for neurosymbolic reasoning
// Config-driven, extensible architecture

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main domain manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Domain {
    #[serde(rename = "domain")]
    pub id: String,

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

    #[serde(rename = "common_operations", alias = "operations", default)]
    pub operations: Vec<Operation>,

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

/// Entity definition (e.g., Process, File, User)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "description")]
    pub description: String,

    #[serde(rename = "core_properties", default)]
    pub core_properties: Vec<Property>,

    #[serde(rename = "derived_properties", default)]
    pub derived_properties: Vec<DerivedProperty>,

    #[serde(rename = "extends", default)]
    pub extends: Option<String>,
}

/// Property of an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Property {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "meaning")]
    pub meaning: String,

    #[serde(rename = "example", default)]
    pub example: Option<serde_json::Value>,

    #[serde(rename = "allowed_values", default)]
    pub allowed_values: Option<Vec<String>>,

    #[serde(rename = "required", default)]
    pub required: bool,
}

/// Derived property from core fields
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedProperty {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "expression")]
    pub expression: String,
}

/// Relationship between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "type")]
    pub rel_type: String,

    #[serde(rename = "from")]
    pub from_entity: String,

    #[serde(rename = "to")]
    pub to_entity: String,

    #[serde(rename = "meaning")]
    pub meaning: String,

    #[serde(rename = "constraints", default)]
    pub constraints: Vec<String>,

    #[serde(rename = "example", default)]
    pub example: Option<serde_json::Value>,
}

/// Abstract operation with command generators
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    #[serde(rename = "op_id")]
    pub id: String,

    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "description", default)]
    pub description: String,

    #[serde(rename = "intent", default)]
    pub intent: String,

    #[serde(rename = "input_schema", default)]
    pub input_schema: HashMap<String, InputSpec>,

    #[serde(rename = "generators", default)]
    pub generators: Vec<Generator>,

    #[serde(rename = "output_schema", default)]
    pub output_schema: Option<OutputSchema>,

    #[serde(rename = "examples", default)]
    pub examples: Vec<OperationExample>,
}

/// Specification for an input parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputSpec {
    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "meaning", default)]
    pub meaning: String,

    #[serde(rename = "optional", default)]
    pub optional: bool,

    #[serde(rename = "default", default)]
    pub default: Option<serde_json::Value>,
}

/// Command generator with template-based resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generator {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "tool")]
    pub tool: String,

    #[serde(rename = "template")]
    pub template: String,

    #[serde(rename = "when", default)]
    pub when: Vec<RequiredInput>,

    #[serde(rename = "optional", default)]
    pub optional: Vec<RequiredInput>,

    #[serde(rename = "timeout_seconds", default)]
    pub timeout_seconds: Option<u64>,

    #[serde(rename = "preference_score", default = "default_preference")]
    pub preference_score: f32,
}

fn default_preference() -> f32 {
    0.0
}

/// Required input for generator selection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredInput {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "equals", default)]
    pub equals: Option<serde_json::Value>,
}

/// Output schema for parsing command results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputSchema {
    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "items", default)]
    pub items: Option<OutputItem>,

    #[serde(rename = "properties", default)]
    pub properties: HashMap<String, OutputProperty>,

    #[serde(rename = "format", default)]
    pub format: Option<String>,

    #[serde(rename = "delimiter", default)]
    pub delimiter: Option<String>,
}

/// Item schema for array outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputItem {
    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "properties", default)]
    pub properties: HashMap<String, OutputProperty>,
}

/// Property in output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputProperty {
    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "column", default)]
    pub column: Option<usize>,

    #[serde(rename = "key", default)]
    pub key: Option<String>,
}

/// Example of operation usage
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationExample {
    #[serde(rename = "description")]
    pub description: String,

    #[serde(rename = "inputs", default)]
    pub inputs: HashMap<String, serde_json::Value>,
}

/// Inference rule for symbolic reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceRule {
    #[serde(rename = "rule_id")]
    pub id: String,

    #[serde(rename = "name", default)]
    pub name: String,

    #[serde(rename = "if")]
    pub if_: Vec<RuleCondition>,

    #[serde(rename = "then")]
    pub then: Vec<RuleConclusion>,
}

/// Condition in inference rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleCondition {
    #[serde(rename = "entity")]
    pub entity: String,

    #[serde(rename = "prop")]
    pub prop: String,

    #[serde(rename = "equals", default)]
    pub equals: Option<serde_json::Value>,

    #[serde(rename = "gt", default)]
    pub gt: Option<f64>,

    #[serde(rename = "lt", default)]
    pub lt: Option<f64>,

    #[serde(rename = "gte", default)]
    pub gte: Option<f64>,

    #[serde(rename = "matches", default)]
    pub matches: Option<String>,
}

/// Conclusion in inference rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleConclusion {
    #[serde(rename = "conclude")]
    pub conclusion: String,

    #[serde(rename = "recommendation", alias = "recommend", default)]
    pub recommendation: Option<String>,

    #[serde(rename = "confidence")]
    pub confidence: f64,
}

/// Troubleshooting pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TroubleshootingPattern {
    #[serde(rename = "pattern_id")]
    pub id: String,

    #[serde(rename = "name", default)]
    pub name: String,

    #[serde(rename = "symptoms", default)]
    pub symptoms: Vec<Symptom>,

    #[serde(rename = "likely_causes", default)]
    pub likely_causes: Vec<LikelyCause>,

    #[serde(rename = "checks", default)]
    pub checks: Vec<TroubleshootCheck>,

    #[serde(rename = "actions", default)]
    pub actions: Vec<TroubleshootAction>,
}

/// Symptom definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symptom {
    #[serde(rename = "metric", default)]
    pub metric: String,

    #[serde(rename = "observation", default)]
    pub observation: String,

    #[serde(rename = "condition", default)]
    pub condition: String,
}

/// Likely cause with signals
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LikelyCause {
    #[serde(rename = "cause")]
    pub cause: String,

    #[serde(rename = "probability", default)]
    pub probability: Option<f64>,

    #[serde(rename = "signals", default)]
    pub signals: Vec<String>,
}

/// Check step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TroubleshootCheck {
    #[serde(rename = "step")]
    pub step: String,

    #[serde(rename = "command", default)]
    pub command: String,

    #[serde(rename = "commands", default)]
    pub commands: Vec<String>,
}

/// Action to take
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TroubleshootAction {
    #[serde(rename = "action")]
    pub action: String,

    #[serde(rename = "methods", default)]
    pub methods: Vec<String>,
}

/// Reasoning template for complex workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasoningTemplate {
    #[serde(rename = "template_id")]
    pub id: String,

    #[serde(rename = "goal")]
    pub goal: String,

    #[serde(rename = "inputs", default)]
    pub inputs: Vec<TemplateInput>,

    #[serde(rename = "steps", default)]
    pub steps: Vec<TemplateStep>,

    #[serde(rename = "outputs", default)]
    pub outputs: Vec<TemplateOutput>,
}

/// Input for reasoning template
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateInput {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "example", default)]
    pub example: Option<serde_json::Value>,
}

/// Step in reasoning template
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateStep {
    #[serde(rename = "step")]
    pub step: i32,

    #[serde(rename = "check")]
    pub check: String,

    #[serde(rename = "logic")]
    pub logic: String,

    #[serde(rename = "next", default)]
    pub next: Vec<String>,
}

/// Output of reasoning template
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateOutput {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "type")]
    pub type_: String,

    #[serde(rename = "example", default)]
    pub example: Option<serde_json::Value>,
}

/// Generated command result
#[derive(Debug, Clone)]
pub struct GeneratedCommand {
    pub tool: String,
    pub command: String,
    pub generator_name: String,
    pub score: f32,
    pub timeout_seconds: Option<u64>,
}

/// Parsed output result
#[derive(Debug, Clone)]
pub struct ParsedOutput {
    pub data: Vec<HashMap<String, serde_json::Value>>,
    pub format: String,
}

/// Domain loading error
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Domain not found: {0}")]
    NotFound(String),

    #[error("Invalid reference: {0}")]
    InvalidReference(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid field value: {0}")]
    InvalidField(String),
}
