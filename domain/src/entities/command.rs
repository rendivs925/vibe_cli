use serde::{Deserialize, Serialize};

/// Command entity representing a system command with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    id: String,
    description: String,
    command_line: String,
    safety_checks: Vec<SafetyCheck>,
    confidence: f32,
}

impl Command {
    pub fn new(
        id: String,
        description: String,
        command_line: String,
        safety_checks: Vec<SafetyCheck>,
        confidence: f32,
    ) -> Self {
        Self {
            id,
            description,
            command_line,
            safety_checks,
            confidence,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn command_line(&self) -> &str {
        &self.command_line
    }

    pub fn safety_checks(&self) -> &[SafetyCheck] {
        &self.safety_checks
    }

    pub fn confidence(&self) -> f32 {
        self.confidence
    }

    pub fn is_safe(&self) -> bool {
        self.safety_checks.iter().all(|check| check.passed())
    }

    pub fn add_safety_check(&mut self, check: SafetyCheck) {
        self.safety_checks.push(check);
    }

    pub fn update_confidence(&mut self, confidence: f32) {
        self.confidence = confidence.clamp(0.0, 1.0);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyCheck {
    check_type: SafetyCheckType,
    passed: bool,
    reason: Option<String>,
}

impl SafetyCheck {
    pub fn new(check_type: SafetyCheckType, passed: bool) -> Self {
        Self {
            check_type,
            passed,
            reason: None,
        }
    }

    pub fn with_reason(check_type: SafetyCheckType, passed: bool, reason: String) -> Self {
        Self {
            check_type,
            passed,
            reason: Some(reason),
        }
    }

    pub fn check_type(&self) -> &SafetyCheckType {
        &self.check_type
    }

    pub fn passed(&self) -> bool {
        self.passed
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyCheckType {
    FileSystemWrite,
    NetworkAccess,
    SystemCommand,
    DestructiveOperation,
    SensitiveFileAccess,
    PrivilegeEscalation,
}

impl SafetyCheckType {
    pub fn description(&self) -> &'static str {
        match self {
            SafetyCheckType::FileSystemWrite => "File system write access",
            SafetyCheckType::NetworkAccess => "Network access",
            SafetyCheckType::SystemCommand => "System command execution",
            SafetyCheckType::DestructiveOperation => "Destructive operation",
            SafetyCheckType::SensitiveFileAccess => "Sensitive file access",
            SafetyCheckType::PrivilegeEscalation => "Privilege escalation",
        }
    }
}
