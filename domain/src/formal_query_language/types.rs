//! Formal Query Language (FQL) for neurosymbolic reasoning
//!
//! FQL decouples "intent understanding" (Neural) from "command syntax" (Symbolic)
//! by providing a structured intermediate representation.
//!
//! Example: "clean old logs in /var/log safely"
//! → FQL: ACTION(delete) & TARGET(/var/log) & PATTERN(*.log) & CONSTRAINT(age>7d) & CONSTRAINT(safe_delete)

use serde::{Deserialize, Serialize};
use std::fmt;

/// A complete FQL query representing user intent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FqlQuery {
    /// Primary action to perform
    pub action: FqlAction,
    /// Target of the action (file, service, etc.)
    pub target: FqlTarget,
    /// Optional pattern for filtering
    pub pattern: Option<FqlPattern>,
    /// Constraints that must be satisfied
    pub constraints: Vec<FqlConstraint>,
    /// Scope of the operation
    pub scope: FqlScope,
    /// Conditions for when to apply
    pub conditions: Vec<FqlCondition>,
    /// Modifiers (dry-run, force, etc.)
    pub modifiers: Vec<FqlModifier>,
}

impl FqlQuery {
    /// Create a new FQL query with defaults
    pub fn new(action: FqlAction, target: FqlTarget) -> Self {
        Self {
            action,
            target,
            pattern: None,
            constraints: vec![],
            scope: FqlScope::Single,
            conditions: vec![],
            modifiers: vec![],
        }
    }

    /// Add a constraint
    pub fn with_constraint(mut self, constraint: FqlConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Add a pattern
    pub fn with_pattern(mut self, pattern: FqlPattern) -> Self {
        self.pattern = Some(pattern);
        self
    }

    /// Set scope
    pub fn with_scope(mut self, scope: FqlScope) -> Self {
        self.scope = scope;
        self
    }

    /// Add a condition
    pub fn with_condition(mut self, condition: FqlCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Add a modifier
    pub fn with_modifier(mut self, modifier: FqlModifier) -> Self {
        self.modifiers.push(modifier);
        self
    }

    /// Convert FQL to human-readable string representation
    pub fn to_fql_string(&self) -> String {
        let mut parts = vec![format!("ACTION({})", self.action)];
        parts.push(format!("TARGET({})", self.target));

        if let Some(ref pattern) = self.pattern {
            parts.push(format!("PATTERN({})", pattern));
        }

        for constraint in &self.constraints {
            parts.push(format!("CONSTRAINT({})", constraint));
        }

        if self.scope != FqlScope::Single {
            parts.push(format!("SCOPE({})", self.scope));
        }

        for condition in &self.conditions {
            parts.push(format!("WHEN({})", condition));
        }

        for modifier in &self.modifiers {
            parts.push(format!("MODIFIER({})", modifier));
        }

        parts.join(" & ")
    }

    /// Check if query has a specific constraint type
    pub fn has_constraint(&self, constraint_type: &str) -> bool {
        self.constraints.iter().any(|c| {
            format!("{:?}", c)
                .to_lowercase()
                .contains(&constraint_type.to_lowercase())
        })
    }

    /// Check if this is a safe operation
    pub fn is_safe(&self) -> bool {
        self.constraints
            .iter()
            .any(|c| matches!(c, FqlConstraint::SafeDelete))
            || self
                .modifiers
                .iter()
                .any(|m| matches!(m, FqlModifier::DryRun))
    }

    /// Get the risk level based on action and constraints
    pub fn risk_level(&self) -> crate::safety::RiskLevel {
        use crate::safety::RiskLevel;

        match self.action {
            FqlAction::Delete | FqlAction::Destroy | FqlAction::Drop => {
                if self.is_safe() {
                    RiskLevel::Warning
                } else {
                    RiskLevel::Dangerous
                }
            }
            FqlAction::Modify | FqlAction::Change | FqlAction::Update => RiskLevel::Warning,
            FqlAction::Create | FqlAction::Read | FqlAction::List | FqlAction::Show => {
                RiskLevel::Safe
            }
            _ => RiskLevel::Unknown,
        }
    }
}

impl fmt::Display for FqlQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_fql_string())
    }
}

/// Actions that can be performed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FqlAction {
    // Data operations
    Create,
    Read,
    Update,
    Delete,

    // System operations
    Start,
    Stop,
    Restart,
    Enable,
    Disable,

    // Information operations
    List,
    Show,
    Display,
    Check,
    Monitor,

    // File operations
    Copy,
    Move,
    Rename,
    Archive,
    Extract,
    Compress,

    // Destructive operations
    Destroy,
    Purge,
    Clean,
    Truncate,
    Drop,

    // Permission operations
    Change,
    Modify,
    Set,

    // Search operations
    Find,
    Search,
    Locate,
    Grep,

    // Network operations
    Connect,
    Disconnect,
    Transfer,
    Download,
    Upload,

    // Installation operations
    Install,
    Uninstall,
    Upgrade,
    Downgrade,

    // Other
    Execute,
    Run,
    Validate,
    Verify,
}

impl fmt::Display for FqlAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FqlAction::Create => write!(f, "create"),
            FqlAction::Read => write!(f, "read"),
            FqlAction::Update => write!(f, "update"),
            FqlAction::Delete => write!(f, "delete"),
            FqlAction::Start => write!(f, "start"),
            FqlAction::Stop => write!(f, "stop"),
            FqlAction::Restart => write!(f, "restart"),
            FqlAction::Enable => write!(f, "enable"),
            FqlAction::Disable => write!(f, "disable"),
            FqlAction::List => write!(f, "list"),
            FqlAction::Show => write!(f, "show"),
            FqlAction::Display => write!(f, "display"),
            FqlAction::Check => write!(f, "check"),
            FqlAction::Monitor => write!(f, "monitor"),
            FqlAction::Copy => write!(f, "copy"),
            FqlAction::Move => write!(f, "move"),
            FqlAction::Rename => write!(f, "rename"),
            FqlAction::Archive => write!(f, "archive"),
            FqlAction::Extract => write!(f, "extract"),
            FqlAction::Compress => write!(f, "compress"),
            FqlAction::Destroy => write!(f, "destroy"),
            FqlAction::Purge => write!(f, "purge"),
            FqlAction::Clean => write!(f, "clean"),
            FqlAction::Truncate => write!(f, "truncate"),
            FqlAction::Drop => write!(f, "drop"),
            FqlAction::Change => write!(f, "change"),
            FqlAction::Modify => write!(f, "modify"),
            FqlAction::Set => write!(f, "set"),
            FqlAction::Find => write!(f, "find"),
            FqlAction::Search => write!(f, "search"),
            FqlAction::Locate => write!(f, "locate"),
            FqlAction::Grep => write!(f, "grep"),
            FqlAction::Connect => write!(f, "connect"),
            FqlAction::Disconnect => write!(f, "disconnect"),
            FqlAction::Transfer => write!(f, "transfer"),
            FqlAction::Download => write!(f, "download"),
            FqlAction::Upload => write!(f, "upload"),
            FqlAction::Install => write!(f, "install"),
            FqlAction::Uninstall => write!(f, "uninstall"),
            FqlAction::Upgrade => write!(f, "upgrade"),
            FqlAction::Downgrade => write!(f, "downgrade"),
            FqlAction::Execute => write!(f, "execute"),
            FqlAction::Run => write!(f, "run"),
            FqlAction::Validate => write!(f, "validate"),
            FqlAction::Verify => write!(f, "verify"),
        }
    }
}

/// Target of the action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FqlTarget {
    // File system
    File(String),
    Directory(String),
    Path(String),

    // System entities
    Process(String),
    Service(String),
    Package(String),
    User(String),
    Group(String),

    // Network
    NetworkInterface(String),
    Port(u16),
    Host(String),
    Url(String),

    // Data
    Database(String),
    Table(String),
    Record(String),

    // System
    Memory,
    Cpu,
    Disk(String),
    Filesystem(String),

    // Information
    Log(String),
    Configuration(String),
    Variable(String),

    // Generic
    Resource(String),
    Component(String),
    Entity(String),
}

impl fmt::Display for FqlTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FqlTarget::File(s) => write!(f, "file:{}", s),
            FqlTarget::Directory(s) => write!(f, "dir:{}", s),
            FqlTarget::Path(s) => write!(f, "path:{}", s),
            FqlTarget::Process(s) => write!(f, "process:{}", s),
            FqlTarget::Service(s) => write!(f, "service:{}", s),
            FqlTarget::Package(s) => write!(f, "package:{}", s),
            FqlTarget::User(s) => write!(f, "user:{}", s),
            FqlTarget::Group(s) => write!(f, "group:{}", s),
            FqlTarget::NetworkInterface(s) => write!(f, "interface:{}", s),
            FqlTarget::Port(p) => write!(f, "port:{}", p),
            FqlTarget::Host(s) => write!(f, "host:{}", s),
            FqlTarget::Url(s) => write!(f, "url:{}", s),
            FqlTarget::Database(s) => write!(f, "database:{}", s),
            FqlTarget::Table(s) => write!(f, "table:{}", s),
            FqlTarget::Record(s) => write!(f, "record:{}", s),
            FqlTarget::Memory => write!(f, "memory"),
            FqlTarget::Cpu => write!(f, "cpu"),
            FqlTarget::Disk(s) => write!(f, "disk:{}", s),
            FqlTarget::Filesystem(s) => write!(f, "filesystem:{}", s),
            FqlTarget::Log(s) => write!(f, "log:{}", s),
            FqlTarget::Configuration(s) => write!(f, "config:{}", s),
            FqlTarget::Variable(s) => write!(f, "var:{}", s),
            FqlTarget::Resource(s) => write!(f, "resource:{}", s),
            FqlTarget::Component(s) => write!(f, "component:{}", s),
            FqlTarget::Entity(s) => write!(f, "entity:{}", s),
        }
    }
}

/// Pattern for filtering targets
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FqlPattern {
    // Glob patterns
    Glob(String),
    Wildcard(String),

    // Regular expressions
    Regex(String),

    // Specific patterns
    Extension(String),
    Name(String),

    // Temporal patterns
    OlderThan(String),
    NewerThan(String),

    // Size patterns
    LargerThan(String),
    SmallerThan(String),

    // Content patterns
    Contains(String),
    StartsWith(String),
    EndsWith(String),
}

impl fmt::Display for FqlPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FqlPattern::Glob(s) => write!(f, "glob:{}", s),
            FqlPattern::Wildcard(s) => write!(f, "wildcard:{}", s),
            FqlPattern::Regex(s) => write!(f, "regex:{}", s),
            FqlPattern::Extension(s) => write!(f, "ext:{}", s),
            FqlPattern::Name(s) => write!(f, "name:{}", s),
            FqlPattern::OlderThan(s) => write!(f, "older_than:{}", s),
            FqlPattern::NewerThan(s) => write!(f, "newer_than:{}", s),
            FqlPattern::LargerThan(s) => write!(f, "larger_than:{}", s),
            FqlPattern::SmallerThan(s) => write!(f, "smaller_than:{}", s),
            FqlPattern::Contains(s) => write!(f, "contains:{}", s),
            FqlPattern::StartsWith(s) => write!(f, "starts_with:{}", s),
            FqlPattern::EndsWith(s) => write!(f, "ends_with:{}", s),
        }
    }
}

/// Constraints on the operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FqlConstraint {
    // Safety constraints
    SafeDelete,
    DryRun,
    Confirm,
    Backup,

    // Permission constraints
    RequiresRoot,
    RequiresUser(String),
    RequiresSudo,

    // Resource constraints
    MaxCpu(f32),
    MaxMemory(String),
    MaxDisk(String),
    Timeout(u64),

    // Data constraints
    PreservePermissions,
    PreserveOwnership,
    Recursive(bool),
    Force(bool),

    // Other
    Interactive,
    Verbose,
    Quiet,
}

impl fmt::Display for FqlConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FqlConstraint::SafeDelete => write!(f, "safe_delete"),
            FqlConstraint::DryRun => write!(f, "dry_run"),
            FqlConstraint::Confirm => write!(f, "confirm"),
            FqlConstraint::Backup => write!(f, "backup"),
            FqlConstraint::RequiresRoot => write!(f, "requires_root"),
            FqlConstraint::RequiresUser(u) => write!(f, "requires_user:{}", u),
            FqlConstraint::RequiresSudo => write!(f, "requires_sudo"),
            FqlConstraint::MaxCpu(c) => write!(f, "max_cpu:{}", c),
            FqlConstraint::MaxMemory(m) => write!(f, "max_memory:{}", m),
            FqlConstraint::MaxDisk(d) => write!(f, "max_disk:{}", d),
            FqlConstraint::Timeout(t) => write!(f, "timeout:{}", t),
            FqlConstraint::PreservePermissions => write!(f, "preserve_permissions"),
            FqlConstraint::PreserveOwnership => write!(f, "preserve_ownership"),
            FqlConstraint::Recursive(r) => write!(f, "recursive:{}", r),
            FqlConstraint::Force(force) => write!(f, "force:{}", force),
            FqlConstraint::Interactive => write!(f, "interactive"),
            FqlConstraint::Verbose => write!(f, "verbose"),
            FqlConstraint::Quiet => write!(f, "quiet"),
        }
    }
}

/// Scope of the operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FqlScope {
    Single,
    Recursive,
    All,
    Children,
    Descendants,
    Siblings,
}

impl fmt::Display for FqlScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FqlScope::Single => write!(f, "single"),
            FqlScope::Recursive => write!(f, "recursive"),
            FqlScope::All => write!(f, "all"),
            FqlScope::Children => write!(f, "children"),
            FqlScope::Descendants => write!(f, "descendants"),
            FqlScope::Siblings => write!(f, "siblings"),
        }
    }
}

/// Conditions for when to apply
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FqlCondition {
    // State conditions
    IfExists,
    IfNotExists,
    IfRunning,
    IfStopped,
    IfEmpty,

    // Comparison conditions
    Equals(String, String),
    GreaterThan(String, String),
    LessThan(String, String),
    Contains(String, String),
    Matches(String, String), // regex

    // Time conditions
    After(String),
    Before(String),
    During(String),

    // Logical combinations
    And(Box<FqlCondition>, Box<FqlCondition>),
    Or(Box<FqlCondition>, Box<FqlCondition>),
    Not(Box<FqlCondition>),
}

impl fmt::Display for FqlCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FqlCondition::IfExists => write!(f, "if_exists"),
            FqlCondition::IfNotExists => write!(f, "if_not_exists"),
            FqlCondition::IfRunning => write!(f, "if_running"),
            FqlCondition::IfStopped => write!(f, "if_stopped"),
            FqlCondition::IfEmpty => write!(f, "if_empty"),
            FqlCondition::Equals(a, b) => write!(f, "{}=={}", a, b),
            FqlCondition::GreaterThan(a, b) => write!(f, "{}>{}", a, b),
            FqlCondition::LessThan(a, b) => write!(f, "{}<{}", a, b),
            FqlCondition::Contains(a, b) => write!(f, "{} contains {}", a, b),
            FqlCondition::Matches(a, b) => write!(f, "{} matches {}", a, b),
            FqlCondition::After(t) => write!(f, "after:{}", t),
            FqlCondition::Before(t) => write!(f, "before:{}", t),
            FqlCondition::During(t) => write!(f, "during:{}", t),
            FqlCondition::And(a, b) => write!(f, "({} AND {})", a, b),
            FqlCondition::Or(a, b) => write!(f, "({} OR {})", a, b),
            FqlCondition::Not(c) => write!(f, "NOT({})", c),
        }
    }
}

/// Modifiers for the operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FqlModifier {
    // Execution modifiers
    DryRun,
    Parallel,
    Sequential,
    Async,

    // Output modifiers
    Quiet,
    Verbose,
    Json,
    Table,

    // Error handling
    IgnoreErrors,
    StopOnError,
    Retry(u32),
}

impl fmt::Display for FqlModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FqlModifier::DryRun => write!(f, "dry_run"),
            FqlModifier::Parallel => write!(f, "parallel"),
            FqlModifier::Sequential => write!(f, "sequential"),
            FqlModifier::Async => write!(f, "async"),
            FqlModifier::Quiet => write!(f, "quiet"),
            FqlModifier::Verbose => write!(f, "verbose"),
            FqlModifier::Json => write!(f, "json"),
            FqlModifier::Table => write!(f, "table"),
            FqlModifier::IgnoreErrors => write!(f, "ignore_errors"),
            FqlModifier::StopOnError => write!(f, "stop_on_error"),
            FqlModifier::Retry(n) => write!(f, "retry:{}", n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fql_query_construction() {
        let query = FqlQuery::new(
            FqlAction::Delete,
            FqlTarget::Directory("/var/log".to_string()),
        )
        .with_pattern(FqlPattern::Glob("*.log".to_string()))
        .with_constraint(FqlConstraint::SafeDelete)
        .with_constraint(FqlConstraint::Recursive(true))
        .with_scope(FqlScope::Recursive)
        .with_modifier(FqlModifier::DryRun);

        assert_eq!(query.action, FqlAction::Delete);
        assert!(query.pattern.is_some());
        assert_eq!(query.constraints.len(), 2);
        assert!(query.is_safe());
    }

    #[test]
    fn test_fql_to_string() {
        let query = FqlQuery::new(FqlAction::List, FqlTarget::Process("nginx".to_string()));

        let fql_str = query.to_fql_string();
        assert!(fql_str.contains("ACTION(list)"));
        assert!(fql_str.contains("TARGET(process:nginx)"));
    }

    #[test]
    fn test_risk_level_calculation() {
        let safe_query = FqlQuery::new(FqlAction::List, FqlTarget::Directory("/tmp".to_string()));
        assert!(matches!(
            safe_query.risk_level(),
            crate::safety::RiskLevel::Safe
        ));

        let dangerous_query =
            FqlQuery::new(FqlAction::Delete, FqlTarget::Directory("/".to_string()));
        assert!(matches!(
            dangerous_query.risk_level(),
            crate::safety::RiskLevel::Dangerous
        ));

        let safe_delete_query =
            FqlQuery::new(FqlAction::Delete, FqlTarget::Directory("/tmp".to_string()))
                .with_constraint(FqlConstraint::SafeDelete);
        assert!(matches!(
            safe_delete_query.risk_level(),
            crate::safety::RiskLevel::Warning
        ));
    }
}
