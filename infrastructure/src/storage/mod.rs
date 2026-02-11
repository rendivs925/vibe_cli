//! Storage modules for infrastructure layer

pub mod experience_buffer;
pub mod induction_engine;
pub mod induction_engine_types;
pub mod knowledge_graph;
pub mod knowledge_graph_entities;
pub mod manpage_cache;
pub mod risk_scorer;
pub mod safety_violation_storage;

pub use experience_buffer::{ExperienceBuffer, ExperienceEntry, FailureType, QueryPattern};
pub use induction_engine::InductionEngine;
pub use induction_engine_types::{
    InducedPattern, InducedRule, InducedRuleResult, PatternType, RuleAction, RuleCondition,
};
pub use knowledge_graph::KnowledgeGraph;
pub use knowledge_graph_entities::{Entity, EntityType, Relationship};
pub use manpage_cache::ManpageCache;
pub use risk_scorer::{RiskCategory, RiskFactor, RiskLevel, RiskProfile, RiskScorer};
pub use safety_violation_storage::{RuleStats, SafetyViolationStorage, ViolationRecord};
