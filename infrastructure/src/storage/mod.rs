//! Storage modules for infrastructure layer

pub mod manpage_cache;
pub mod safety_violation_storage;

pub use manpage_cache::ManpageCache;
pub use safety_violation_storage::{RuleStats, SafetyViolationStorage, ViolationRecord};
