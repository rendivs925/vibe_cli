//! Enhanced Safety Kernel
//! 
//! Provides comprehensive safety checking for commands with 28 hard rules
//! that prevent catastrophic system actions.

pub mod hard_rules;
pub mod safety_engine;
pub mod safety_report;

// Re-export main types
pub use hard_rules::{HardRules, RuleAction, SafetyRule, ViolationType};
pub use safety_engine::SafetyEngine;
pub use safety_report::{SafetyReport, SafetyViolation};

use serde::{Deserialize, Serialize};
use std::fmt;

/// Risk level for commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum RiskLevel {
    /// Safe to execute without confirmation
    Safe,
    /// Potentially dangerous, requires confirmation
    Warning,
    /// Catastrophic, execution blocked
    Dangerous,
    /// Unknown risk, treat as warning
    #[default]
    Unknown,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Safe => write!(f, "SAFE"),
            RiskLevel::Warning => write!(f, "WARNING"),
            RiskLevel::Dangerous => write!(f, "DANGEROUS"),
            RiskLevel::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

