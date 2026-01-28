use super::super::entities::command::{Command, SafetyCheck, SafetyCheckType};

/// Safety policy value object defining security rules
#[derive(Debug, Clone)]
pub struct SafetyPolicy {
    rules: Vec<SafetyRule>,
    strict_mode: bool,
}

impl SafetyPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn strict() -> Self {
        Self {
            rules: vec![
                SafetyRule::NoFileSystemWrites,
                SafetyRule::NoNetworkAccess,
                SafetyRule::NoSystemCommands,
                SafetyRule::NoDestructiveOperations,
                SafetyRule::NoSensitiveFileAccess,
                SafetyRule::NoPrivilegeEscalation,
            ],
            strict_mode: true,
        }
    }

    pub fn permissive() -> Self {
        Self {
            rules: vec![
                SafetyRule::NoDestructiveOperations,
                SafetyRule::NoPrivilegeEscalation,
            ],
            strict_mode: false,
        }
    }

    pub fn with_rules(mut self, rules: Vec<SafetyRule>) -> Self {
        self.rules = rules;
        self
    }

    pub fn strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    pub fn rules(&self) -> &[SafetyRule] {
        &self.rules
    }

    pub fn is_strict_mode(&self) -> bool {
        self.strict_mode
    }

    pub fn validate_command(&self, command: &str) -> SafetyResult {
        let mut checks = Vec::new();
        let mut overall_safe = true;

        for rule in &self.rules {
            let check = rule.check_command(command);
            if !check.passed() {
                overall_safe = false;
            }
            checks.push(check);
        }

        SafetyResult::new(overall_safe, checks)
    }

    pub fn validate_command_entity(&self, command: &Command) -> SafetyResult {
        let mut checks = command.safety_checks().to_vec();
        let mut overall_safe = command.is_safe();

        // Apply additional policy rules
        for rule in &self.rules {
            let check = rule.check_command(command.command_line());
            if !check.passed() {
                overall_safe = false;
            }
            checks.push(check);
        }

        SafetyResult::new(overall_safe, checks)
    }
}

impl Default for SafetyPolicy {
    fn default() -> Self {
        Self {
            rules: vec![
                SafetyRule::NoFileSystemWrites,
                SafetyRule::NoNetworkAccess,
                SafetyRule::NoSystemCommands,
            ],
            strict_mode: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SafetyRule {
    NoFileSystemWrites,
    NoNetworkAccess,
    NoSystemCommands,
    NoDestructiveOperations,
    NoSensitiveFileAccess,
    NoPrivilegeEscalation,
}

impl SafetyRule {
    pub fn check_command(&self, command: &str) -> SafetyCheck {
        let passed = !self.violates_rule(command);
        let reason = if passed {
            None
        } else {
            Some(format!("Command violates rule: {}", self.description()))
        };

        SafetyCheck::with_reason(self.to_check_type(), passed, reason.unwrap_or_default())
    }

    pub fn description(&self) -> &'static str {
        match self {
            SafetyRule::NoFileSystemWrites => "Prevents file system write operations",
            SafetyRule::NoNetworkAccess => "Prevents network access operations",
            SafetyRule::NoSystemCommands => "Prevents system command execution",
            SafetyRule::NoDestructiveOperations => "Prevents destructive file operations",
            SafetyRule::NoSensitiveFileAccess => "Prevents access to sensitive files",
            SafetyRule::NoPrivilegeEscalation => "Prevents privilege escalation commands",
        }
    }

    fn violates_rule(&self, command: &str) -> bool {
        let cmd_lower = command.to_lowercase();

        match self {
            SafetyRule::NoFileSystemWrites => {
                cmd_lower.contains("rm ")
                    || cmd_lower.contains("mv ")
                    || cmd_lower.contains("cp ")
                    || cmd_lower.contains("write")
                    || cmd_lower.contains("create")
                    || cmd_lower.contains(">")
                    || cmd_lower.contains(">>")
            }
            SafetyRule::NoNetworkAccess => {
                cmd_lower.contains("curl")
                    || cmd_lower.contains("wget")
                    || cmd_lower.contains("http")
                    || cmd_lower.contains("ftp")
                    || cmd_lower.contains("ssh")
                    || cmd_lower.contains("telnet")
            }
            SafetyRule::NoSystemCommands => {
                cmd_lower.contains("sudo")
                    || cmd_lower.contains("su ")
                    || cmd_lower.contains("chmod")
                    || cmd_lower.contains("chown")
                    || cmd_lower.contains("kill")
                    || cmd_lower.contains("systemctl")
            }
            SafetyRule::NoDestructiveOperations => {
                cmd_lower.contains("rm -rf")
                    || cmd_lower.contains("format")
                    || cmd_lower.contains("delete")
                    || cmd_lower.contains("truncate")
            }
            SafetyRule::NoSensitiveFileAccess => {
                cmd_lower.contains("/etc/")
                    || cmd_lower.contains("/root/")
                    || cmd_lower.contains("/var/log")
                    || cmd_lower.contains("passwd")
                    || cmd_lower.contains("shadow")
            }
            SafetyRule::NoPrivilegeEscalation => {
                cmd_lower.contains("sudo")
                    || cmd_lower.contains("su ")
                    || cmd_lower.contains("doas")
                    || cmd_lower.contains("pkexec")
            }
        }
    }

    fn to_check_type(&self) -> SafetyCheckType {
        match self {
            SafetyRule::NoFileSystemWrites => SafetyCheckType::FileSystemWrite,
            SafetyRule::NoNetworkAccess => SafetyCheckType::NetworkAccess,
            SafetyRule::NoSystemCommands => SafetyCheckType::SystemCommand,
            SafetyRule::NoDestructiveOperations => SafetyCheckType::DestructiveOperation,
            SafetyRule::NoSensitiveFileAccess => SafetyCheckType::SensitiveFileAccess,
            SafetyRule::NoPrivilegeEscalation => SafetyCheckType::PrivilegeEscalation,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SafetyResult {
    is_safe: bool,
    checks: Vec<SafetyCheck>,
}

impl SafetyResult {
    pub fn new(is_safe: bool, checks: Vec<SafetyCheck>) -> Self {
        Self { is_safe, checks }
    }

    pub fn is_safe(&self) -> bool {
        self.is_safe
    }

    pub fn checks(&self) -> &[SafetyCheck] {
        &self.checks
    }

    pub fn failed_checks(&self) -> Vec<&SafetyCheck> {
        self.checks.iter().filter(|c| !c.passed()).collect()
    }

    pub fn passed_checks(&self) -> Vec<&SafetyCheck> {
        self.checks.iter().filter(|c| c.passed()).collect()
    }

    pub fn summary(&self) -> String {
        let passed = self.passed_checks().len();
        let total = self.checks.len();

        if self.is_safe {
            format!("All {} safety checks passed", total)
        } else {
            format!("{} of {} safety checks failed", total - passed, total)
        }
    }
}
