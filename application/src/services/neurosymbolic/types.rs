//! Neurosymbolic Service Types
//!
//! Configuration and data structures for neurosymbolic processing

use domain::{domain_config::types::GeneratedCommand, safety::SafetyReport, services::SafetyProof};
use infrastructure::storage::risk_scorer::{RiskLevel, RiskProfile};
use std::collections::HashMap;

/// Configuration for neurosymbolic processing
#[derive(Debug, Clone)]
pub struct NeurosymbolicConfig {
    /// Enable safety validation
    pub enable_safety: bool,
    /// Enable manpage validation
    pub enable_manpage_validation: bool,
    /// Enable learning/RAG
    pub enable_learning: bool,
    /// Require confirmation for safety warnings (dangerous commands always blocked)
    pub block_on_safety: bool,
    /// Block on invalid syntax
    pub block_on_invalid_syntax: bool,
}

impl Default for NeurosymbolicConfig {
    fn default() -> Self {
        Self {
            enable_safety: true,
            enable_manpage_validation: true,
            enable_learning: true,
            block_on_safety: true,
            block_on_invalid_syntax: true,
        }
    }
}

/// Optional structured intent signal from upstream analysis (LLM or rules)
#[derive(Debug, Clone, Default)]
pub struct IntentSignal {
    pub category: Option<String>,
    pub action: Option<String>,
    pub target: Option<String>,
    pub objects: Vec<String>,
    pub constraints: Vec<String>,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct IntentSuggestion {
    pub intent: String,
    pub action: Option<String>,
    pub target: Option<String>,
    pub objects: Vec<String>,
    pub constraints: Vec<String>,
    pub params: HashMap<String, String>,
    pub reasoning: String,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct SymbolicCommandSuggestion {
    pub op_id: String,
    pub op_name: String,
    pub commands: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct DomainCommandValidation {
    pub is_valid: bool,
    pub reason: Option<String>,
    pub suggestion: Option<SymbolicCommandSuggestion>,
}

/// Result of neurosymbolic processing
#[derive(Debug, Clone)]
pub struct NeurosymbolicResult {
    /// Original query
    pub query: String,
    /// Whether the query was handled by neurosymbolic system
    pub handled: bool,
    /// Generated commands (if any)
    pub commands: Vec<GeneratedCommand>,
    /// Safety report for validation
    pub safety_report: Option<SafetyReport>,
    /// Safety proof for assurance
    pub safety_proof: Option<SafetyProof>,
    /// Risk profile if assessed
    pub risk_profile: Option<RiskProfile>,
    /// Manpage validation result
    pub manpage_valid: Option<bool>,
    /// Manpage validation details
    pub manpage_details: Option<String>,
    /// Whether confirmation was required
    pub confirmation_required: bool,
    /// User confirmed execution
    pub user_confirmed: bool,
    /// Was execution blocked
    pub blocked: bool,
    /// Block reason
    pub block_reason: Option<String>,
    /// Execution output
    pub execution_output: Option<String>,
    /// Learning applied
    pub learning_applied: bool,
    /// Execution status
    pub execution_success: Option<bool>,
}

impl NeurosymbolicResult {
    /// Create a result indicating neurosymbolic couldn't handle this query
    pub fn not_handled(query: &str) -> Self {
        Self {
            query: query.to_string(),
            handled: false,
            commands: Vec::new(),
            safety_report: None,
            safety_proof: None,
            risk_profile: None,
            manpage_valid: None,
            manpage_details: None,
            confirmation_required: false,
            user_confirmed: false,
            blocked: false,
            block_reason: None,
            execution_output: None,
            learning_applied: false,
            execution_success: None,
        }
    }

    /// Check if the result is safe to execute
    pub fn is_safe(&self) -> bool {
        if let Some(ref report) = self.safety_report {
            !report.has_fatal_violations()
        } else {
            // Without a safety report, assume unsafe if commands exist
            !self.has_commands()
        }
    }

    /// Check if this result has generated commands
    pub fn has_commands(&self) -> bool {
        !self.commands.is_empty()
    }

    /// Get the primary command (first one)
    pub fn primary_command(&self) -> Option<&GeneratedCommand> {
        self.commands.first()
    }

    /// Get risk level if assessed
    pub fn risk_level(&self) -> RiskLevel {
        self.risk_profile
            .as_ref()
            .map(|p| p.level.clone())
            .unwrap_or(RiskLevel::Unknown)
    }

    /// Check if this requires user confirmation
    pub fn requires_confirmation(&self) -> bool {
        self.confirmation_required && !self.user_confirmed
    }

    /// Format a user-friendly summary
    pub fn format_summary(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Query: {}", self.query));
        lines.push(format!("Handled: {}", self.handled));

        if !self.commands.is_empty() {
            lines.push(format!("Commands: {}", self.commands.len()));
            for (i, cmd) in self.commands.iter().enumerate() {
                lines.push(format!("  {}: {}", i + 1, cmd.command));
            }
        }

        if let Some(ref report) = self.safety_report {
            lines.push(format!("Safety: {} violations", report.violations.len()));
        }

        if let Some(ref profile) = self.risk_profile {
            lines.push(format!("Risk: {:?}", profile.level));
        }

        if let Some(valid) = self.manpage_valid {
            lines.push(format!("Manpage Valid: {}", valid));
        }

        lines.push(format!("Blocked: {}", self.blocked));
        if let Some(ref reason) = self.block_reason {
            lines.push(format!("Block Reason: {}", reason));
        }

        lines.join("\n")
    }
}

impl Default for NeurosymbolicResult {
    fn default() -> Self {
        Self {
            query: String::new(),
            handled: false,
            commands: Vec::new(),
            safety_report: None,
            safety_proof: None,
            risk_profile: None,
            manpage_valid: None,
            manpage_details: None,
            confirmation_required: false,
            user_confirmed: false,
            blocked: false,
            block_reason: None,
            execution_output: None,
            learning_applied: false,
            execution_success: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CommandSegment {
    pub(crate) cmd: String,
    pub(crate) flags: Vec<String>,
}

/// Processing stage for detailed tracking
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingStage {
    IntentAnalysis,
    CommandGeneration,
    SafetyValidation,
    ManpageValidation,
    RiskAssessment,
    LearningRetrieval,
    Execution,
}

impl std::fmt::Display for ProcessingStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessingStage::IntentAnalysis => write!(f, "intent analysis"),
            ProcessingStage::CommandGeneration => write!(f, "command generation"),
            ProcessingStage::SafetyValidation => write!(f, "safety validation"),
            ProcessingStage::ManpageValidation => write!(f, "manpage validation"),
            ProcessingStage::RiskAssessment => write!(f, "risk assessment"),
            ProcessingStage::LearningRetrieval => write!(f, "learning retrieval"),
            ProcessingStage::Execution => write!(f, "execution"),
        }
    }
}

/// Processing event for telemetry
#[derive(Debug, Clone)]
pub struct ProcessingEvent {
    pub stage: ProcessingStage,
    pub operation: String,
    pub duration_ms: u64,
    pub success: bool,
    pub details: Option<String>,
}
