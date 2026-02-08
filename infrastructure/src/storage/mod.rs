//! Storage modules for infrastructure layer

pub mod safety_violation_storage;

pub use safety_violation_storage::{RuleStats, SafetyViolationStorage, ViolationRecord};
