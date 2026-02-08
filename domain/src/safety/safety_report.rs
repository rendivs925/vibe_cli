//! Safety violation report
//!
//! Detailed report of safety analysis for a command

use super::{RiskLevel, ViolationType};
use serde::{Deserialize, Serialize};

/// Complete safety analysis report for a command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyReport {
    /// Overall risk level
    pub overall_risk: RiskLevel,
    /// Individual violations found
    pub violations: Vec<SafetyViolation>,
    /// Whether command can proceed
    pub can_proceed: bool,
    /// Summary message for user
    pub summary: String,
    /// Original command analyzed
    pub command: String,
    /// Timestamp of analysis
    pub analyzed_at: String,
}

impl SafetyReport {
    /// Create a new safe report
    pub fn safe(command: &str) -> Self {
        Self {
            overall_risk: RiskLevel::Safe,
            violations: vec![],
            can_proceed: true,
            summary: format!("Command '{}' passed all safety checks", command),
            command: command.to_string(),
            analyzed_at: chrono::Local::now().to_rfc3339(),
        }
    }

    /// Create a report with violations
    pub fn with_violations(command: &str, violations: Vec<SafetyViolation>) -> Self {
        let has_blocks = violations.iter().any(|v| v.is_blocked());
        let overall_risk = if has_blocks {
            RiskLevel::Dangerous
        } else if !violations.is_empty() {
            RiskLevel::Warning
        } else {
            RiskLevel::Safe
        };

        let summary = if has_blocks {
            format!(
                "Command '{}' has {} CRITICAL safety violation(s) and CANNOT be executed",
                command,
                violations.iter().filter(|v| v.is_blocked()).count()
            )
        } else if !violations.is_empty() {
            format!(
                "Command '{}' has {} warning(s) that require confirmation",
                command,
                violations.len()
            )
        } else {
            format!("Command '{}' passed all safety checks", command)
        };

        Self {
            overall_risk,
            violations,
            can_proceed: !has_blocks,
            summary,
            command: command.to_string(),
            analyzed_at: chrono::Local::now().to_rfc3339(),
        }
    }

    /// Check if command is completely safe
    pub fn is_safe(&self) -> bool {
        self.overall_risk == RiskLevel::Safe
    }

    /// Check if command is blocked
    pub fn is_blocked(&self) -> bool {
        !self.can_proceed
    }

    /// Get blocked violations only
    pub fn blocked_violations(&self) -> Vec<&SafetyViolation> {
        self.violations.iter().filter(|v| v.is_blocked()).collect()
    }

    /// Get warning violations only
    pub fn warning_violations(&self) -> Vec<&SafetyViolation> {
        self.violations.iter().filter(|v| !v.is_blocked()).collect()
    }

    /// Format report for display
    pub fn format_display(&self) -> String {
        let mut output = String::new();

        // Header with risk level
        let risk_color = match self.overall_risk {
            RiskLevel::Safe => "🟢",
            RiskLevel::Warning => "🟡",
            RiskLevel::Dangerous => "🔴",
            RiskLevel::Unknown => "⚪",
        };

        output.push_str(&format!("{} {}\n", risk_color, self.summary));

        if !self.violations.is_empty() {
            output.push_str("\nDetailed Analysis:\n");

            // Show blocked violations first
            let blocked: Vec<_> = self.blocked_violations();
            if !blocked.is_empty() {
                output.push_str("\n❌ BLOCKED VIOLATIONS:\n");
                for v in blocked {
                    output.push_str(&v.format_display());
                    output.push('\n');
                }
            }

            // Show warnings
            let warnings: Vec<_> = self.warning_violations();
            if !warnings.is_empty() {
                output.push_str("\n⚠️  WARNINGS:\n");
                for v in warnings {
                    output.push_str(&v.format_display());
                    output.push('\n');
                }
            }
        }

        output
    }
}

/// A single safety violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyViolation {
    /// Rule ID that was violated
    pub rule_id: String,
    /// Human-readable name
    pub rule_name: String,
    /// Type of violation
    pub violation_type: ViolationType,
    /// Detailed description
    pub description: String,
    /// Whether this blocks execution
    pub blocked: bool,
    /// Part of command that matched
    pub matched_pattern: String,
    /// Suggested safer alternative
    pub suggestion: Option<String>,
}

impl SafetyViolation {
    /// Create a new violation
    pub fn new(
        rule_id: &str,
        rule_name: &str,
        violation_type: ViolationType,
        description: &str,
        blocked: bool,
        matched_pattern: &str,
        suggestion: Option<&str>,
    ) -> Self {
        Self {
            rule_id: rule_id.to_string(),
            rule_name: rule_name.to_string(),
            violation_type,
            description: description.to_string(),
            blocked,
            matched_pattern: matched_pattern.to_string(),
            suggestion: suggestion.map(|s| s.to_string()),
        }
    }

    /// Check if this violation blocks execution
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    /// Format for display
    pub fn format_display(&self) -> String {
        let icon = if self.blocked { "🚫" } else { "⚠️" };
        let mut output = format!("  {} [{}] {}\n", icon, self.rule_id, self.rule_name);
        output.push_str(&format!("     Type: {}\n", self.violation_type));
        output.push_str(&format!("     Description: {}\n", self.description));
        output.push_str(&format!("     Matched: {}\n", self.matched_pattern));
        if let Some(ref suggestion) = self.suggestion {
            output.push_str(&format!("     Suggestion: {}\n", suggestion));
        }
        output
    }
}
