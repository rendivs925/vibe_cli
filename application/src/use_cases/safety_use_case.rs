use async_trait::async_trait;
use domain::entities::command::{Command, SafetyCheck, SafetyCheckType};
use domain::value_objects::safety_policy::{SafetyPolicy, SafetyResult};
use shared::error::AppError;

/// Use case for safety validation and policy enforcement
pub struct SafetyUseCase {
    safety_policy: SafetyPolicy,
}

impl SafetyUseCase {
    pub fn new(safety_policy: SafetyPolicy) -> Self {
        Self { safety_policy }
    }

    pub fn with_default_policy() -> Self {
        Self::new(SafetyPolicy::default())
    }

    pub fn with_strict_policy() -> Self {
        Self::new(SafetyPolicy::strict())
    }

    /// Validate a command against safety policy
    pub fn validate_command(&self, command_line: &str) -> SafetyValidationResult {
        let safety_result = self.safety_policy.validate_command(command_line);

        SafetyValidationResult::new(
            command_line.to_string(),
            safety_result.clone(),
            self.get_recommendations(&safety_result),
        )
    }

    /// Validate a command entity
    pub fn validate_command_entity(&self, command: &Command) -> SafetyValidationResult {
        let safety_result = self.safety_policy.validate_command_entity(command);

        SafetyValidationResult::new(
            command.command_line().to_string(),
            safety_result.clone(),
            self.get_recommendations(&safety_result),
        )
    }

    /// Check if a file path is safe to access
    pub fn validate_file_access(&self, file_path: &str) -> FileAccessValidationResult {
        let path = std::path::Path::new(file_path);

        // Check for dangerous patterns
        let dangerous_patterns = vec![
            "/etc/",
            "/root/",
            "/var/log",
            "/sys/",
            "/proc/",
            "~/.ssh/",
            "~/.gnupg/",
        ];

        let path_str = file_path.to_lowercase();
        let is_dangerous = dangerous_patterns
            .iter()
            .any(|pattern| path_str.contains(pattern));

        // Check for absolute paths
        let is_absolute = path.is_absolute();

        // Check for hidden files
        let is_hidden = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false);

        let safety_result = SafetyResult::new(
            !is_dangerous && !is_hidden,
            vec![
                SafetyCheck::new(SafetyCheckType::SensitiveFileAccess, !is_dangerous),
                SafetyCheck::new(SafetyCheckType::FileSystemWrite, false), // Read access
            ],
        );

        FileAccessValidationResult::new(
            file_path.to_string(),
            safety_result,
            is_absolute,
            is_hidden,
            is_dangerous,
        )
    }

    /// Get safety policy information
    pub fn get_policy_info(&self) -> SafetyPolicyInfo {
        SafetyPolicyInfo::new(
            self.safety_policy.rules().to_vec(),
            self.safety_policy.is_strict_mode(),
        )
    }

    /// Update safety policy
    pub fn update_policy(&mut self, new_policy: SafetyPolicy) {
        self.safety_policy = new_policy;
    }

    /// Get safety recommendations based on validation result
    fn get_recommendations(&self, safety_result: &SafetyResult) -> Vec<String> {
        let mut recommendations = Vec::new();

        if !safety_result.is_safe() {
            recommendations.push("Consider using a safer alternative".to_string());
            recommendations.push("Review the command before execution".to_string());

            for check in safety_result.failed_checks() {
                match check.check_type() {
                    SafetyCheckType::FileSystemWrite => {
                        recommendations.push("Use read-only operations when possible".to_string());
                    }
                    SafetyCheckType::NetworkAccess => {
                        recommendations.push("Verify network endpoints are trusted".to_string());
                    }
                    SafetyCheckType::SystemCommand => {
                        recommendations
                            .push("Use specific commands instead of system calls".to_string());
                    }
                    SafetyCheckType::DestructiveOperation => {
                        recommendations
                            .push("Backup data before destructive operations".to_string());
                    }
                    SafetyCheckType::SensitiveFileAccess => {
                        recommendations.push("Use proper file permissions".to_string());
                    }
                    SafetyCheckType::PrivilegeEscalation => {
                        recommendations.push("Run with minimum required privileges".to_string());
                    }
                }
            }
        }

        recommendations
    }

    /// Analyze command for potential risks
    pub fn analyze_command_risks(&self, command_line: &str) -> CommandRiskAnalysis {
        let validation = self.validate_command(command_line);

        let risk_level = if validation.safety_result().is_safe() {
            RiskLevel::Low
        } else {
            let failed_checks = validation.safety_result().failed_checks().len();
            if failed_checks >= 3 {
                RiskLevel::High
            } else if failed_checks >= 2 {
                RiskLevel::Medium
            } else {
                RiskLevel::Low
            }
        };

        CommandRiskAnalysis::new(
            command_line.to_string(),
            risk_level,
            validation.safety_result().checks().to_vec(),
            validation.recommendations().to_vec(),
        )
    }
}

/// Result of safety validation
#[derive(Debug, Clone)]
pub struct SafetyValidationResult {
    command: String,
    safety_result: SafetyResult,
    recommendations: Vec<String>,
}

impl SafetyValidationResult {
    pub fn new(command: String, safety_result: SafetyResult, recommendations: Vec<String>) -> Self {
        Self {
            command,
            safety_result,
            recommendations,
        }
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn safety_result(&self) -> &SafetyResult {
        &self.safety_result
    }

    pub fn recommendations(&self) -> &[String] {
        &self.recommendations
    }

    pub fn is_safe(&self) -> bool {
        self.safety_result.is_safe()
    }

    pub fn risk_score(&self) -> f32 {
        let total_checks = self.safety_result.checks().len();
        let failed_checks = self.safety_result.failed_checks().len();

        if total_checks == 0 {
            0.0
        } else {
            1.0 - (failed_checks as f32 / total_checks as f32)
        }
    }
}

/// Result of file access validation
#[derive(Debug, Clone)]
pub struct FileAccessValidationResult {
    file_path: String,
    safety_result: SafetyResult,
    is_absolute: bool,
    is_hidden: bool,
    is_dangerous: bool,
}

impl FileAccessValidationResult {
    pub fn new(
        file_path: String,
        safety_result: SafetyResult,
        is_absolute: bool,
        is_hidden: bool,
        is_dangerous: bool,
    ) -> Self {
        Self {
            file_path,
            safety_result,
            is_absolute,
            is_hidden,
            is_dangerous,
        }
    }

    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    pub fn safety_result(&self) -> &SafetyResult {
        &self.safety_result
    }

    pub fn is_absolute(&self) -> bool {
        self.is_absolute
    }

    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    pub fn is_dangerous(&self) -> bool {
        self.is_dangerous
    }

    pub fn is_safe_to_access(&self) -> bool {
        self.safety_result.is_safe()
    }
}

/// Safety policy information
#[derive(Debug, Clone)]
pub struct SafetyPolicyInfo {
    rules: Vec<domain::value_objects::safety_policy::SafetyRule>,
    strict_mode: bool,
}

impl SafetyPolicyInfo {
    pub fn new(
        rules: Vec<domain::value_objects::safety_policy::SafetyRule>,
        strict_mode: bool,
    ) -> Self {
        Self { rules, strict_mode }
    }

    pub fn rules(&self) -> &[domain::value_objects::safety_policy::SafetyRule] {
        &self.rules
    }

    pub fn is_strict_mode(&self) -> bool {
        self.strict_mode
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

/// Command risk analysis
#[derive(Debug, Clone)]
pub struct CommandRiskAnalysis {
    command: String,
    risk_level: RiskLevel,
    safety_checks: Vec<SafetyCheck>,
    recommendations: Vec<String>,
}

impl CommandRiskAnalysis {
    pub fn new(
        command: String,
        risk_level: RiskLevel,
        safety_checks: Vec<SafetyCheck>,
        recommendations: Vec<String>,
    ) -> Self {
        Self {
            command,
            risk_level,
            safety_checks,
            recommendations,
        }
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn risk_level(&self) -> &RiskLevel {
        &self.risk_level
    }

    pub fn safety_checks(&self) -> &[SafetyCheck] {
        &self.safety_checks
    }

    pub fn recommendations(&self) -> &[String] {
        &self.recommendations
    }
}

/// Risk level classification
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "Low",
            RiskLevel::Medium => "Medium",
            RiskLevel::High => "High",
            RiskLevel::Critical => "Critical",
        }
    }

    pub fn color_code(&self) -> &'static str {
        match self {
            RiskLevel::Low => "green",
            RiskLevel::Medium => "yellow",
            RiskLevel::High => "orange",
            RiskLevel::Critical => "red",
        }
    }
}

#[async_trait]
pub trait AsyncSafetyService: Send + Sync {
    async fn validate_command(&self, command: &str) -> Result<SafetyValidationResult, AppError>;
    async fn validate_file_access(
        &self,
        file_path: &str,
    ) -> Result<FileAccessValidationResult, AppError>;
}
