//! Induction Engine Types - Pattern detection from failures
//!
//! Types for mining the experience buffer to discover patterns in failures
//! and automatically generating rules to prevent them.

use crate::storage::experience_buffer::FailureType;

/// A discovered pattern from failure analysis
#[derive(Debug, Clone)]
pub struct InducedPattern {
    pub id: i64,
    pub pattern_type: PatternType,
    pub description: String,
    pub confidence: f32,
    pub occurrences: i32,
    pub example_queries: Vec<String>,
    pub example_commands: Vec<String>,
    pub induced_rule: Option<InducedRule>,
    pub discovered_at: String,
}

/// Types of patterns that can be discovered
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternType {
    PermissionRequired,
    PathPattern,
    MissingDependency,
    WrongUser,
    ServiceNotRunning,
    InvalidSyntax,
    ResourceUnavailable,
    TimingIssue,
    Other,
}

impl PatternType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PatternType::PermissionRequired => "permission_required",
            PatternType::PathPattern => "path_pattern",
            PatternType::MissingDependency => "missing_dependency",
            PatternType::WrongUser => "wrong_user",
            PatternType::ServiceNotRunning => "service_not_running",
            PatternType::InvalidSyntax => "invalid_syntax",
            PatternType::ResourceUnavailable => "resource_unavailable",
            PatternType::TimingIssue => "timing_issue",
            PatternType::Other => "other",
        }
    }
}

/// An automatically induced rule
#[derive(Debug, Clone)]
pub struct InducedRule {
    pub id: i64,
    pub pattern_id: i64,
    pub rule_name: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub confidence: f32,
    pub enabled: bool,
}

/// Condition for an induced rule
#[derive(Debug, Clone)]
pub enum RuleCondition {
    PathMatches(String),
    CommandContains(String),
    FailureType(FailureType),
    ErrorMessageContains(String),
    And(Box<RuleCondition>, Box<RuleCondition>),
    Or(Box<RuleCondition>, Box<RuleCondition>),
}

/// Action to take when rule matches
#[derive(Debug, Clone)]
pub enum RuleAction {
    AddPrefix(String),
    AddFlag(String),
    CheckService(String),
    Warn(String),
    Block(String),
    SuggestAlternative(String),
}

/// Result of applying induced rules to a command
#[derive(Debug, Clone)]
pub struct InducedRuleResult {
    pub command: String,
    pub warnings: Vec<String>,
    pub blocked_reason: Option<String>,
    pub notes: Vec<String>,
}
