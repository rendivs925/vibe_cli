//! Risk Scorer - Probabilistic risk assessment for commands
//!
//! Calculates risk scores based on multiple factors:
//! - Command destructiveness (read vs write vs delete)
//! - Target sensitivity (system paths, user data)
//! - Permission requirements
//! - Historical success rates
//! - System impact scope

use crate::storage::experience_buffer::{ExperienceBuffer, FailureType};
use std::collections::HashMap;

/// Risk level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Minimal = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Minimal => "minimal",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }

    pub fn from_score(score: f32) -> Self {
        if score < 0.2 {
            RiskLevel::Minimal
        } else if score < 0.4 {
            RiskLevel::Low
        } else if score < 0.6 {
            RiskLevel::Medium
        } else if score < 0.8 {
            RiskLevel::High
        } else {
            RiskLevel::Critical
        }
    }
}

/// Risk profile for a command
#[derive(Debug, Clone)]
pub struct RiskProfile {
    pub overall_score: f32,
    pub risk_level: RiskLevel,
    pub factors: Vec<RiskFactor>,
    pub confidence: f32,
    pub mitigation_steps: Vec<String>,
}

/// Individual risk factor
#[derive(Debug, Clone)]
pub struct RiskFactor {
    pub category: RiskCategory,
    pub description: String,
    pub score: f32,
    pub weight: f32,
}

/// Categories of risk
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RiskCategory {
    Destructiveness,
    TargetSensitivity,
    PermissionRequirements,
    HistoricalSuccess,
    SystemImpact,
    Scope,
    UnknownCommand,
}

/// Risk scorer engine
pub struct RiskScorer {
    weights: HashMap<RiskCategory, f32>,
    experience_buffer: Option<ExperienceBuffer>,
}

impl RiskScorer {
    /// Create new risk scorer with default weights
    pub fn new() -> Self {
        let mut weights = HashMap::new();
        weights.insert(RiskCategory::Destructiveness, 0.30);
        weights.insert(RiskCategory::TargetSensitivity, 0.25);
        weights.insert(RiskCategory::PermissionRequirements, 0.15);
        weights.insert(RiskCategory::HistoricalSuccess, 0.15);
        weights.insert(RiskCategory::SystemImpact, 0.10);
        weights.insert(RiskCategory::Scope, 0.05);
        weights.insert(RiskCategory::UnknownCommand, 0.10);

        Self {
            weights,
            experience_buffer: None,
        }
    }

    /// Set experience buffer for historical analysis
    pub fn with_experience_buffer(mut self, buffer: ExperienceBuffer) -> Self {
        self.experience_buffer = Some(buffer);
        self
    }

    /// Calculate risk profile for a command
    pub fn assess(&self, command: &str, query: &str) -> RiskProfile {
        let mut factors = vec![];
        let mut total_weight = 0.0;
        let mut weighted_score = 0.0;

        // Destructiveness factor
        let destruct_factor = self.assess_destructiveness(command);
        let destruct_weight = self.weights[&RiskCategory::Destructiveness];
        factors.push(destruct_factor.clone());
        weighted_score += destruct_factor.score * destruct_weight;
        total_weight += destruct_weight;

        // Target sensitivity factor
        let target_factor = self.assess_target_sensitivity(command);
        let target_weight = self.weights[&RiskCategory::TargetSensitivity];
        factors.push(target_factor.clone());
        weighted_score += target_factor.score * target_weight;
        total_weight += target_weight;

        // Permission requirements factor
        let perm_factor = self.assess_permission_requirements(command);
        let perm_weight = self.weights[&RiskCategory::PermissionRequirements];
        factors.push(perm_factor.clone());
        weighted_score += perm_factor.score * perm_weight;
        total_weight += perm_weight;

        // Historical success factor
        let history_factor = self.assess_historical_success(query);
        let history_weight = self.weights[&RiskCategory::HistoricalSuccess];
        factors.push(history_factor.clone());
        weighted_score += history_factor.score * history_weight;
        total_weight += history_weight;

        // System impact factor
        let impact_factor = self.assess_system_impact(command);
        let impact_weight = self.weights[&RiskCategory::SystemImpact];
        factors.push(impact_factor.clone());
        weighted_score += impact_factor.score * impact_weight;
        total_weight += impact_weight;

        // Scope factor
        let scope_factor = self.assess_scope(command);
        let scope_weight = self.weights[&RiskCategory::Scope];
        factors.push(scope_factor.clone());
        weighted_score += scope_factor.score * scope_weight;
        total_weight += scope_weight;

        // Calculate final score
        let overall_score = if total_weight > 0.0 {
            weighted_score / total_weight
        } else {
            0.5
        };

        let risk_level = RiskLevel::from_score(overall_score);
        let confidence = self.calculate_confidence(&factors);
        let mitigation_steps = self.generate_mitigations(&factors, overall_score);

        RiskProfile {
            overall_score,
            risk_level,
            factors,
            confidence,
            mitigation_steps,
        }
    }

    /// Assess command destructiveness (read vs write vs delete)
    fn assess_destructiveness(&self, command: &str) -> RiskFactor {
        let cmd_lower = command.to_lowercase();

        // Destructive commands
        let destructive_patterns = vec![
            ("rm", 0.9),
            ("dd", 0.95),
            ("mkfs", 0.95),
            ("fdisk", 0.9),
            ("shred", 0.95),
            ("wipe", 0.95),
            ("format", 0.9),
        ];

        // Write/modify commands
        let write_patterns = vec![
            ("cp", 0.4),
            ("mv", 0.5),
            ("chmod", 0.5),
            ("chown", 0.6),
            ("echo", 0.3),
            (">", 0.6),
            (">>", 0.4),
        ];

        // Safe commands
        let safe_patterns = vec![
            ("ls", 0.05),
            ("cat", 0.05),
            ("ps", 0.05),
            ("top", 0.05),
            ("htop", 0.05),
            ("df", 0.05),
            ("free", 0.05),
            ("grep", 0.05),
            ("awk", 0.1),
            ("sed", 0.2),
        ];

        for (pattern, score) in destructive_patterns {
            if cmd_lower.contains(pattern) {
                return RiskFactor {
                    category: RiskCategory::Destructiveness,
                    description: format!("Destructive command: {}", pattern),
                    score,
                    weight: self.weights[&RiskCategory::Destructiveness],
                };
            }
        }

        for (pattern, score) in write_patterns {
            if cmd_lower.contains(pattern) {
                return RiskFactor {
                    category: RiskCategory::Destructiveness,
                    description: format!("Write operation: {}", pattern),
                    score,
                    weight: self.weights[&RiskCategory::Destructiveness],
                };
            }
        }

        for (pattern, score) in safe_patterns {
            if cmd_lower.contains(pattern) {
                return RiskFactor {
                    category: RiskCategory::Destructiveness,
                    description: format!("Read-only command: {}", pattern),
                    score,
                    weight: self.weights[&RiskCategory::Destructiveness],
                };
            }
        }

        RiskFactor {
            category: RiskCategory::Destructiveness,
            description: "Unknown command type".to_string(),
            score: 0.5,
            weight: self.weights[&RiskCategory::Destructiveness],
        }
    }

    /// Assess target sensitivity (system paths vs user data)
    fn assess_target_sensitivity(&self, command: &str) -> RiskFactor {
        let critical_paths = vec![
            ("/boot", 0.95),
            ("/etc", 0.9),
            ("/usr", 0.85),
            ("/bin", 0.95),
            ("/sbin", 0.95),
            ("/lib", 0.9),
            ("/dev", 0.95),
            ("/proc", 0.95),
            ("/sys", 0.95),
        ];

        let sensitive_paths = vec![
            ("/var/log", 0.7),
            ("/var/lib", 0.75),
            ("/opt", 0.6),
            ("/home", 0.4),
            ("/root", 0.8),
            ("/tmp", 0.3),
        ];

        for (path, score) in critical_paths {
            if command.contains(path) {
                return RiskFactor {
                    category: RiskCategory::TargetSensitivity,
                    description: format!("Critical system path: {}", path),
                    score,
                    weight: self.weights[&RiskCategory::TargetSensitivity],
                };
            }
        }

        for (path, score) in sensitive_paths {
            if command.contains(path) {
                return RiskFactor {
                    category: RiskCategory::TargetSensitivity,
                    description: format!("Sensitive path: {}", path),
                    score,
                    weight: self.weights[&RiskCategory::TargetSensitivity],
                };
            }
        }

        RiskFactor {
            category: RiskCategory::TargetSensitivity,
            description: "User data or temporary path".to_string(),
            score: 0.2,
            weight: self.weights[&RiskCategory::TargetSensitivity],
        }
    }

    /// Assess permission requirements
    fn assess_permission_requirements(&self, command: &str) -> RiskFactor {
        if command.contains("sudo") {
            RiskFactor {
                category: RiskCategory::PermissionRequirements,
                description: "Requires elevated privileges (sudo)".to_string(),
                score: 0.6,
                weight: self.weights[&RiskCategory::PermissionRequirements],
            }
        } else {
            RiskFactor {
                category: RiskCategory::PermissionRequirements,
                description: "Standard user permissions".to_string(),
                score: 0.2,
                weight: self.weights[&RiskCategory::PermissionRequirements],
            }
        }
    }

    /// Assess historical success rate
    fn assess_historical_success(&self, query: &str) -> RiskFactor {
        if let Some(ref buffer) = self.experience_buffer {
            match buffer.get_success_rate(query) {
                Ok(rate) => {
                    let score = 1.0 - rate; // Higher failure rate = higher risk
                    let description = if rate < 0.3 {
                        "Very low historical success rate"
                    } else if rate < 0.5 {
                        "Low historical success rate"
                    } else if rate < 0.7 {
                        "Moderate historical success rate"
                    } else {
                        "Good historical success rate"
                    };

                    RiskFactor {
                        category: RiskCategory::HistoricalSuccess,
                        description: description.to_string(),
                        score,
                        weight: self.weights[&RiskCategory::HistoricalSuccess],
                    }
                }
                Err(_) => RiskFactor {
                    category: RiskCategory::HistoricalSuccess,
                    description: "No historical data available".to_string(),
                    score: 0.5,
                    weight: self.weights[&RiskCategory::HistoricalSuccess],
                },
            }
        } else {
            RiskFactor {
                category: RiskCategory::HistoricalSuccess,
                description: "Experience buffer not configured".to_string(),
                score: 0.5,
                weight: self.weights[&RiskCategory::HistoricalSuccess],
            }
        }
    }

    /// Assess system impact
    fn assess_system_impact(&self, command: &str) -> RiskFactor {
        let high_impact_commands = vec![
            "reboot",
            "shutdown",
            "halt",
            "poweroff",
            "systemctl stop",
            "service stop",
            "killall",
            "pkill",
            "iptables",
            "ufw",
            "firewall-cmd",
        ];

        let medium_impact_commands = vec![
            "systemctl restart",
            "service restart",
            "modprobe",
            "insmod",
            "rmmod",
        ];

        let cmd_lower = command.to_lowercase();

        for pattern in high_impact_commands {
            if cmd_lower.contains(pattern) {
                return RiskFactor {
                    category: RiskCategory::SystemImpact,
                    description: format!("High system impact: {}", pattern),
                    score: 0.9,
                    weight: self.weights[&RiskCategory::SystemImpact],
                };
            }
        }

        for pattern in medium_impact_commands {
            if cmd_lower.contains(pattern) {
                return RiskFactor {
                    category: RiskCategory::SystemImpact,
                    description: format!("Medium system impact: {}", pattern),
                    score: 0.6,
                    weight: self.weights[&RiskCategory::SystemImpact],
                };
            }
        }

        RiskFactor {
            category: RiskCategory::SystemImpact,
            description: "Minimal system impact".to_string(),
            score: 0.1,
            weight: self.weights[&RiskCategory::SystemImpact],
        }
    }

    /// Assess scope (single file vs recursive vs all)
    fn assess_scope(&self, command: &str) -> RiskFactor {
        if command.contains(" -r ") || command.contains(" -R ") || command.contains(" --recursive")
        {
            RiskFactor {
                category: RiskCategory::Scope,
                description: "Recursive operation (affects multiple files/directories)".to_string(),
                score: 0.7,
                weight: self.weights[&RiskCategory::Scope],
            }
        } else if command.contains("*") || command.contains("?") {
            RiskFactor {
                category: RiskCategory::Scope,
                description: "Wildcard operation (affects multiple files)".to_string(),
                score: 0.5,
                weight: self.weights[&RiskCategory::Scope],
            }
        } else {
            RiskFactor {
                category: RiskCategory::Scope,
                description: "Single target operation".to_string(),
                score: 0.2,
                weight: self.weights[&RiskCategory::Scope],
            }
        }
    }

    /// Calculate confidence in risk assessment
    fn calculate_confidence(&self, factors: &[RiskFactor]) -> f32 {
        // More factors = higher confidence
        let coverage = factors.len() as f32 / self.weights.len() as f32;

        // Check if we have experience data
        let has_history = factors
            .iter()
            .any(|f| f.category == RiskCategory::HistoricalSuccess && f.score != 0.5);

        if has_history {
            coverage * 1.0
        } else {
            coverage * 0.8
        }
    }

    /// Generate mitigation steps based on risk factors
    fn generate_mitigations(&self, factors: &[RiskFactor], overall_score: f32) -> Vec<String> {
        let mut mitigations = vec![];

        if overall_score > 0.7 {
            mitigations.push("⚠️  HIGH RISK: Consider using --dry-run first".to_string());
        }

        for factor in factors {
            if factor.score > 0.7 {
                match factor.category {
                    RiskCategory::Destructiveness => {
                        mitigations.push("💾 Backup data before executing".to_string());
                    }
                    RiskCategory::TargetSensitivity => {
                        mitigations.push("🔒 Verify target path is correct".to_string());
                        mitigations.push("👤 Ensure you have proper permissions".to_string());
                    }
                    RiskCategory::PermissionRequirements => {
                        mitigations.push("⚡ Double-check sudo is necessary".to_string());
                    }
                    RiskCategory::SystemImpact => {
                        mitigations.push("🔄 Schedule during maintenance window".to_string());
                        mitigations.push("📢 Notify other users if applicable".to_string());
                    }
                    _ => {}
                }
            }
        }

        mitigations
    }

    /// Format risk profile for display
    pub fn format_profile(&self, profile: &RiskProfile) -> String {
        let icon = match profile.risk_level {
            RiskLevel::Minimal => "🟢",
            RiskLevel::Low => "🟢",
            RiskLevel::Medium => "🟡",
            RiskLevel::High => "🟠",
            RiskLevel::Critical => "🔴",
        };

        let mut output = format!(
            "{} Risk Assessment: {} ({:.0}% confidence)\n",
            icon,
            profile.risk_level.as_str().to_uppercase(),
            profile.confidence * 100.0
        );
        output.push_str(&format!(
            "Overall Score: {:.2}/1.0\n\n",
            profile.overall_score
        ));

        output.push_str("Risk Factors:\n");
        for factor in &profile.factors {
            let factor_icon = if factor.score < 0.3 {
                "🟢"
            } else if factor.score < 0.6 {
                "🟡"
            } else {
                "🔴"
            };
            output.push_str(&format!(
                "  {} {}: {:.2} ({}%)\n",
                factor_icon,
                factor.description,
                factor.score,
                (factor.weight * 100.0) as i32
            ));
        }

        if !profile.mitigation_steps.is_empty() {
            output.push_str("\nMitigation Steps:\n");
            for step in &profile.mitigation_steps {
                output.push_str(&format!("  {}\n", step));
            }
        }

        output
    }
}

impl Default for RiskScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_from_score() {
        assert_eq!(RiskLevel::from_score(0.1), RiskLevel::Minimal);
        assert_eq!(RiskLevel::from_score(0.3), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(0.5), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(0.7), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(0.9), RiskLevel::Critical);
    }

    #[test]
    fn test_assess_destructiveness() {
        let scorer = RiskScorer::new();

        let rm_factor = scorer.assess_destructiveness("rm -rf /tmp");
        assert!(rm_factor.score > 0.8);

        let ls_factor = scorer.assess_destructiveness("ls -la");
        assert!(ls_factor.score < 0.2);
    }

    #[test]
    fn test_assess_target_sensitivity() {
        let scorer = RiskScorer::new();

        let etc_factor = scorer.assess_target_sensitivity("cat /etc/passwd");
        assert!(etc_factor.score > 0.8);

        let home_factor = scorer.assess_target_sensitivity("ls ~/documents");
        assert!(home_factor.score < 0.5);
    }

    #[test]
    fn test_full_assessment() {
        let scorer = RiskScorer::new();
        let profile = scorer.assess("rm -rf /etc", "delete config");

        assert!(profile.overall_score > 0.6);
        assert_eq!(profile.risk_level, RiskLevel::High);
        assert!(!profile.factors.is_empty());
    }

    #[test]
    fn test_mitigation_generation() {
        let scorer = RiskScorer::new();
        let profile = scorer.assess("rm -rf /", "delete everything");

        assert!(!profile.mitigation_steps.is_empty());
        assert!(profile
            .mitigation_steps
            .iter()
            .any(|m| m.contains("Backup")));
    }
}
