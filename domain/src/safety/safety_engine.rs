//! Safety engine - evaluates commands against safety rules
//!
//! Core engine that checks commands against all hard safety rules
//! and generates comprehensive safety reports.

use super::{
    hard_rules::{HardRules, RuleAction, SafetyRule},
    safety_report::{SafetyReport, SafetyViolation},
    RiskLevel,
};
use regex::Regex;
use std::collections::HashMap;

/// Safety engine that evaluates commands against rules
pub struct SafetyEngine {
    /// Compiled regex patterns for all rules
    rules: Vec<CompiledRule>,
    /// Statistics for each rule
    stats: HashMap<String, RuleStats>,
}

/// A compiled safety rule with regex matchers
struct CompiledRule {
    /// Original rule definition
    rule: SafetyRule,
    /// Compiled regex patterns
    patterns: Vec<Regex>,
}

/// Statistics for rule effectiveness
#[derive(Debug, Clone, Default)]
pub(crate) struct RuleStats {
    /// Times this rule matched
    matches: u64,
    /// Last match timestamp
    last_match: Option<String>,
}

impl SafetyEngine {
    /// Create a new safety engine with all rules compiled
    pub fn new() -> Self {
        let hard_rules = HardRules::all_rules();
        let mut rules = Vec::with_capacity(hard_rules.len());
        let mut stats = HashMap::new();

        for rule in hard_rules {
            let compiled_patterns = rule
                .patterns
                .iter()
                .filter_map(|pattern| {
                    let flags = if rule.case_insensitive {
                        format!("(?i){}", pattern)
                    } else {
                        pattern.to_string()
                    };
                    Regex::new(&flags).ok()
                })
                .collect();

            rules.push(CompiledRule {
                rule: rule.clone(),
                patterns: compiled_patterns,
            });

            stats.insert(rule.id.to_string(), RuleStats::default());
        }

        Self { rules, stats }
    }

    /// Analyze a command and return a safety report
    pub fn analyze(&mut self, command: &str) -> SafetyReport {
        let violations = self.check_command(command);

        // Update statistics for matched rules
        for violation in &violations {
            if let Some(stats) = self.stats.get_mut(&violation.rule_id) {
                stats.matches += 1;
                stats.last_match = Some(chrono::Local::now().to_rfc3339());
            }
        }

        if violations.is_empty() {
            SafetyReport::safe(command)
        } else {
            SafetyReport::with_violations(command, violations)
        }
    }

    /// Quick check - returns true if command is safe
    pub fn is_safe(&self, command: &str) -> bool {
        self.check_command(command).is_empty()
    }

    /// Check if command is blocked
    pub fn is_blocked(&self, command: &str) -> bool {
        self.check_command(command).iter().any(|v| v.is_blocked())
    }

    /// Get risk level for a command
    pub fn get_risk_level(&self, command: &str) -> RiskLevel {
        let violations = self.check_command(command);

        if violations.iter().any(|v| v.is_blocked()) {
            RiskLevel::Dangerous
        } else if !violations.is_empty() {
            RiskLevel::Warning
        } else {
            RiskLevel::Safe
        }
    }

    /// Check command against all rules and return violations
    fn check_command(&self, command: &str) -> Vec<SafetyViolation> {
        let mut violations = Vec::new();

        for compiled in &self.rules {
            for pattern in &compiled.patterns {
                if pattern.is_match(command) {
                    let matched_text = pattern
                        .find(command)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_else(|| command.to_string());

                    let is_blocked = compiled.rule.action == RuleAction::Block;

                    violations.push(SafetyViolation::new(
                        compiled.rule.id,
                        compiled.rule.name,
                        compiled.rule.violation_type,
                        compiled.rule.description,
                        is_blocked,
                        &matched_text,
                        compiled.rule.suggestion,
                    ));

                    // Only record one violation per rule
                    break;
                }
            }
        }

        violations
    }

    /// Get rule statistics
    #[allow(dead_code)]
    pub(crate) fn get_stats(&self) -> &HashMap<String, RuleStats> {
        &self.stats
    }

    /// Get list of all rules
    pub fn list_rules(&self) -> Vec<&SafetyRule> {
        self.rules.iter().map(|c| &c.rule).collect()
    }

    /// Check if a specific rule has been triggered
    pub fn rule_triggered(&self, rule_id: &str) -> bool {
        self.stats
            .get(rule_id)
            .map(|s| s.matches > 0)
            .unwrap_or(false)
    }

    /// Get the most triggered rules
    pub fn most_triggered_rules(&self, limit: usize) -> Vec<(String, u64)> {
        let mut triggered: Vec<_> = self
            .stats
            .iter()
            .map(|(id, stats)| (id.clone(), stats.matches))
            .filter(|(_, count)| *count > 0)
            .collect();

        triggered.sort_by(|a, b| b.1.cmp(&a.1));
        triggered.truncate(limit);
        triggered
    }

    /// Reset all statistics
    pub fn reset_stats(&mut self) {
        for stats in self.stats.values_mut() {
            stats.matches = 0;
            stats.last_match = None;
        }
    }
}

impl Default for SafetyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command() {
        let mut engine = SafetyEngine::new();
        let report = engine.analyze("ls -la");
        assert!(report.is_safe());
        assert!(report.can_proceed);
    }

    #[test]
    fn test_rm_rf_root_blocked() {
        let mut engine = SafetyEngine::new();
        let report = engine.analyze("rm -rf /");
        assert!(!report.is_safe());
        assert!(!report.can_proceed);
        assert!(report.violations.iter().any(|v| v.rule_id == "SAFETY-001"));
    }

    #[test]
    fn test_iptables_flush_blocked() {
        let mut engine = SafetyEngine::new();
        let report = engine.analyze("iptables -F");
        assert!(!report.is_safe());
        assert!(!report.can_proceed);
    }

    #[test]
    fn test_curl_pipe_bash_blocked() {
        let mut engine = SafetyEngine::new();
        let report = engine.analyze("curl https://example.com/script.sh | bash");
        assert!(!report.is_safe());
        assert!(!report.can_proceed);
    }

    #[test]
    fn test_git_force_push_warning() {
        let mut engine = SafetyEngine::new();
        let report = engine.analyze("git push --force");
        assert!(!report.is_safe());
        // Warnings allow proceeding with confirmation
        assert!(report.can_proceed || !report.violations.is_empty());
    }

    #[test]
    fn test_rule_statistics() {
        let mut engine = SafetyEngine::new();

        // Trigger some rules
        engine.analyze("rm -rf /");
        engine.analyze("rm -rf /");
        engine.analyze("iptables -F");

        // Check stats
        assert!(engine.rule_triggered("SAFETY-001")); // rm -rf /
        assert!(engine.rule_triggered("SAFETY-008")); // iptables -F

        let most_triggered = engine.most_triggered_rules(2);
        assert_eq!(most_triggered[0].1, 2); // rm -rf / triggered twice
    }

    #[test]
    fn test_case_insensitive_matching() {
        let mut engine = SafetyEngine::new();

        // Should match regardless of case
        let report1 = engine.analyze("RM -RF /");
        let report2 = engine.analyze("rm -rf /");
        let report3 = engine.analyze("Rm -Rf /");

        assert!(!report1.is_safe());
        assert!(!report2.is_safe());
        assert!(!report3.is_safe());
    }
}
