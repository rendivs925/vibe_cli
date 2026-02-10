//! Enhanced Safety Kernel for Vibe CLI
//!
//! Provides 20+ hard safety rules to prevent catastrophic system actions.
//! Each rule can BLOCK (prevent execution) or WARN (require confirmation).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Risk level classification for commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Safe to execute without confirmation
    Safe,
    /// Potentially dangerous, requires confirmation
    Warning,
    /// Catastrophic, execution blocked
    Dangerous,
    /// Unknown risk, treat as warning
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

/// Type of safety violation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationType {
    /// Destructive wildcard pattern
    DestructiveWildcard,
    /// System directory deletion
    SystemDirectoryDeletion,
    /// Disk formatting operation
    DiskFormatting,
    /// Dangerous permission change
    PermissionEscalation,
    /// Network security risk
    NetworkExposure,
    /// Password in plain text
    PasswordExposure,
    /// Service disruption risk
    ServiceDisruption,
    /// Data destruction
    DataDestruction,
    /// Sudo misuse
    SudoMisuse,
    /// Dangerous pipeline
    DangerousPipeline,
    /// Git destructive operation
    GitDestruction,
    /// Database destructive operation
    DatabaseDestruction,
    /// Other violation
    Other,
}

impl fmt::Display for ViolationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ViolationType::DestructiveWildcard => "Destructive Wildcard",
            ViolationType::SystemDirectoryDeletion => "System Directory Deletion",
            ViolationType::DiskFormatting => "Disk Formatting",
            ViolationType::PermissionEscalation => "Permission Escalation",
            ViolationType::NetworkExposure => "Network Exposure",
            ViolationType::PasswordExposure => "Password Exposure",
            ViolationType::ServiceDisruption => "Service Disruption",
            ViolationType::DataDestruction => "Data Destruction",
            ViolationType::SudoMisuse => "Sudo Misuse",
            ViolationType::DangerousPipeline => "Dangerous Pipeline",
            ViolationType::GitDestruction => "Git Destructive Operation",
            ViolationType::DatabaseDestruction => "Database Destruction",
            ViolationType::Other => "Other Violation",
        };
        write!(f, "{}", name)
    }
}

/// Action to take when rule matches
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    /// Block execution entirely
    Block,
    /// Warn user but allow with confirmation
    Warn,
    /// Allow execution
    Allow,
}

/// A single safety rule
#[derive(Debug, Clone)]
pub struct SafetyRule {
    /// Unique rule identifier
    pub id: &'static str,
    /// Human-readable name
    pub name: &'static str,
    /// Detailed description
    pub description: &'static str,
    /// Type of violation
    pub violation_type: ViolationType,
    /// Action to take
    pub action: RuleAction,
    /// Regex patterns to match (any match triggers rule)
    pub patterns: Vec<&'static str>,
    /// Case-insensitive matching
    pub case_insensitive: bool,
    /// Suggested safer alternative
    pub suggestion: Option<&'static str>,
}

/// Collection of all hard safety rules
pub struct HardRules;

impl HardRules {
    fn rule(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        violation_type: ViolationType,
        action: RuleAction,
        patterns: Vec<&'static str>,
        case_insensitive: bool,
        suggestion: Option<&'static str>,
    ) -> SafetyRule {
        SafetyRule {
            id,
            name,
            description,
            violation_type,
            action,
            patterns,
            case_insensitive,
            suggestion,
        }
    }
}

impl HardRules {
    /// Get all safety rules
    pub fn all_rules() -> Vec<SafetyRule> {
        vec![
            Self::destructive_wildcards(),
            Self::system_directory_deletion(),
            Self::disk_formatting(),
            Self::dd_to_disk(),
            Self::permission_escalation(),
            Self::chmod_system_dirs(),
            Self::network_exposure(),
            Self::iptables_flush(),
            Self::password_exposure(),
            Self::echo_password(),
            Self::service_disruption(),
            Self::kill_init(),
            Self::stop_ssh_while_connected(),
            Self::data_destruction(),
            Self::shred_system_files(),
            Self::write_to_disk_device(),
            Self::sudo_misuse(),
            Self::sudo_rm_rf(),
            Self::sudo_bash(),
            Self::dangerous_pipeline(),
            Self::curl_pipe_bash(),
            Self::wget_pipe_sh(),
            Self::git_destruction(),
            Self::git_force_push(),
            Self::git_reset_hard(),
            Self::database_destruction(),
            Self::drop_database(),
            Self::delete_without_where(),
        ]
    }

    // === DESTRUCTIVE WILDCARDS ===

    fn destructive_wildcards() -> SafetyRule {
        Self::rule(
            "SAFETY-001",
            "Destructive Wildcard Pattern",
            "Commands using wildcards that could delete system files or home directory",
            ViolationType::DestructiveWildcard,
            RuleAction::Block,
            vec![
                r"rm\s+-[rf]*\s+/",
                r"rm\s+-[rf]*\s+~/",
                r"rm\s+-[rf]*\s+\$HOME",
                r"find\s+/\s+-name\s+.*-delete",
                r"find\s+/\s+-exec\s+rm",
                r"rm\s+-[rf]*\s+\*/",
            ],
            true,
            Some("Use specific paths instead of wildcards. Consider using 'rm -i' for interactive mode."),
        )
    }

    // === SYSTEM DIRECTORY DELETION ===

    fn system_directory_deletion() -> SafetyRule {
        Self::rule(
            "SAFETY-002",
            "System Directory Deletion",
            "Attempting to delete critical system directories",
            ViolationType::SystemDirectoryDeletion,
            RuleAction::Block,
            vec![
                r"rm\s+-[rf]*\s+/etc(\s|/|\z)",
                r"rm\s+-[rf]*\s+/usr(\s|/|\z)",
                r"rm\s+-[rf]*\s+/bin(\s|/|\z)",
                r"rm\s+-[rf]*\s+/sbin(\s|/|\z)",
                r"rm\s+-[rf]*\s+/lib(\s|/|\z)",
                r"rm\s+-[rf]*\s+/lib64(\s|/|\z)",
                r"rm\s+-[rf]*\s+/boot(\s|/|\z)",
                r"rm\s+-[rf]*\s+/sys(\s|/|\z)",
                r"rm\s+-[rf]*\s+/proc(\s|/|\z)",
                r"rm\s+-[rf]*\s+/dev(\s|/|\z)",
            ],
            true,
            Some("System directories should never be deleted. Use package manager to remove software."),
        )
    }

    // === DISK FORMATTING ===

    fn disk_formatting() -> SafetyRule {
        Self::rule(
            "SAFETY-003",
            "Disk Formatting",
            "Formatting a filesystem which will destroy all data",
            ViolationType::DiskFormatting,
            RuleAction::Block,
            vec![
                r"mkfs\.\w+\s+/dev/[sh]d[a-z]\d*",
                r"mkfs\s+/dev/[sh]d[a-z]\d*",
                r"newfs\s+/dev/[sh]d[a-z]\d*",
            ],
            true,
            Some("Disk formatting destroys all data. Ensure you have backups and are targeting the correct device."),
        )
    }

    fn dd_to_disk() -> SafetyRule {
        Self::rule(
            "SAFETY-004",
            "Direct Disk Write",
            "Using dd to write directly to disk devices",
            ViolationType::DiskFormatting,
            RuleAction::Block,
            vec![
                r"dd\s+.*of=/dev/[sh]d[a-z]\d*",
                r"dd\s+.*of=/dev/nvme\d+n\d+",
                r"dd\s+.*of=/dev/mmcblk\d+",
                r"dd\s+.*of=/dev/disk\d+",
            ],
            true,
            Some("Writing directly to disk devices destroys data. Double-check the output device (of=...)."),
        )
    }

    // === PERMISSION ESCALATION ===

    fn permission_escalation() -> SafetyRule {
        Self::rule(
            "SAFETY-005",
            "Dangerous Permission Change",
            "Changing permissions to make files world-writable or executable",
            ViolationType::PermissionEscalation,
            RuleAction::Warn,
            vec![
                r"chmod\s+.*777\s+/etc",
                r"chmod\s+.*777\s+/usr",
                r"chmod\s+.*777\s+/bin",
                r"chmod\s+.*777\s+/var",
                r"chmod\s+-R\s+.*777\s+/",
                r"chmod\s+-R\s+.*666\s+/",
            ],
            true,
            Some("World-writable permissions are a security risk. Use more restrictive permissions like 755 or 644."),
        )
    }

    fn chmod_system_dirs() -> SafetyRule {
        Self::rule(
            "SAFETY-006",
            "Permission Change on System Directories",
            "Changing ownership or permissions of system directories",
            ViolationType::PermissionEscalation,
            RuleAction::Warn,
            vec![
                r"chown\s+-R\s+.*/etc(\s|/|\z)",
                r"chown\s+-R\s+.*/usr(\s|/|\z)",
                r"chown\s+-R\s+.*/bin(\s|/|\z)",
                r"chmod\s+-R\s+.*/etc(\s|/|\z)",
                r"chmod\s+-R\s+.*/usr(\s|/|\z)",
            ],
            true,
            Some("Changing system directory permissions can break your system. Use sudo for specific operations instead."),
        )
    }

    // === NETWORK EXPOSURE ===

    fn network_exposure() -> SafetyRule {
        Self::rule(
            "SAFETY-007",
            "Privileged Port Exposure",
            "Opening network ports below 1024 (privileged ports)",
            ViolationType::NetworkExposure,
            RuleAction::Warn,
            vec![
                r"iptables\s+.*-p\s+tcp\s+.*--dport\s+\d{1,3}\s",
                r"firewall-cmd\s+.*--add-port=\d{1,3}/",
                r"ufw\s+allow\s+\d{1,3}/",
            ],
            true,
            Some("Privileged ports (<1024) require root and may expose services. Ensure this is intentional."),
        )
    }

    fn iptables_flush() -> SafetyRule {
        Self::rule(
            "SAFETY-008",
            "Firewall Flush",
            "Flushing all iptables rules removes firewall protection",
            ViolationType::NetworkExposure,
            RuleAction::Block,
            vec![
                r"iptables\s+-F",
                r"iptables\s+--flush",
                r"nft\s+flush\s+ruleset",
            ],
            true,
            Some("Flushing firewall rules exposes your system. Save rules first with 'iptables-save'."),
        )
    }

    // === PASSWORD EXPOSURE ===

    fn password_exposure() -> SafetyRule {
        Self::rule(
            "SAFETY-009",
            "Password in Command",
            "Command contains potential password in plain text",
            ViolationType::PasswordExposure,
            RuleAction::Block,
            vec![
                r"mysql\s+-u\s+\w+\s+-p\s*\w+",
                r"psql\s+.*-W\s*\w+",
                r"redis-cli\s+.*-a\s*\w+",
                r"curl\s+.*-u\s+\w+:\w+",
                r"wget\s+.*--password[=\s]+\w+",
            ],
            true,
            Some("Never put passwords in commands - they appear in shell history and process lists. Use environment variables or config files."),
        )
    }

    fn echo_password() -> SafetyRule {
        Self::rule(
            "SAFETY-010",
            "Password Echoed to Command",
            "Echoing password into a command via pipe",
            ViolationType::PasswordExposure,
            RuleAction::Warn,
            vec![
                r"echo\s+.*\|\s*mysql",
                r"echo\s+.*\|\s*sudo",
                r"echo\s+.*\|\s*su",
            ],
            true,
            Some("Piped passwords appear in shell history. Use secure authentication methods instead."),
        )
    }

    // === SERVICE DISRUPTION ===

    fn service_disruption() -> SafetyRule {
        Self::rule(
            "SAFETY-011",
            "Service Disruption",
            "Stopping or killing critical system services",
            ViolationType::ServiceDisruption,
            RuleAction::Warn,
            vec![
                r"systemctl\s+stop\s+(systemd|network|ssh|sshd)",
                r"service\s+(ssh|sshd|network)\s+stop",
                r"killall\s+-9\s+(systemd|init)",
            ],
            true,
            Some("Stopping critical services may disconnect you or crash the system. Ensure you have alternative access."),
        )
    }

    fn kill_init() -> SafetyRule {
        Self::rule(
            "SAFETY-012",
            "Kill Init Process",
            "Attempting to kill init (PID 1) will crash the system",
            ViolationType::ServiceDisruption,
            RuleAction::Block,
            vec![
                r"kill\s+-9\s+1\s*$",
                r"kill\s+-SIGKILL\s+1\s*$",
                r"killall\s+-9\s+systemd",
                r"killall\s+-9\s+init",
            ],
            true,
            Some("Killing the init process will crash your system immediately. This should never be done."),
        )
    }

    fn stop_ssh_while_connected() -> SafetyRule {
        Self::rule(
            "SAFETY-013",
            "Stop SSH While Connected",
            "Stopping SSH service while connected via SSH",
            ViolationType::ServiceDisruption,
            RuleAction::Warn,
            vec![
                r"systemctl\s+(stop|restart)\s+(ssh|sshd)",
                r"service\s+(ssh|sshd)\s+(stop|restart)",
            ],
            true,
            Some("You appear to be connected via SSH. Stopping SSH will disconnect you. Ensure you have console access."),
        )
    }

    // === DATA DESTRUCTION ===

    fn data_destruction() -> SafetyRule {
        Self::rule(
            "SAFETY-014",
            "Data Destruction",
            "Commands that securely delete or overwrite data",
            ViolationType::DataDestruction,
            RuleAction::Block,
            vec![
                r"shred\s+-[u]*\s+/etc",
                r"shred\s+-[u]*\s+/usr",
                r"shred\s+-[u]*\s+/home",
                r"shred\s+-[u]*\s+/var",
            ],
            true,
            Some("Secure deletion cannot be undone. Ensure you have backups and are targeting the correct files."),
        )
    }

    fn shred_system_files() -> SafetyRule {
        Self::rule(
            "SAFETY-015",
            "Shred System Files",
            "Using shred on system or important directories",
            ViolationType::DataDestruction,
            RuleAction::Block,
            vec![
                r"shred\s+.*-u\s+/", r"shred\s+.*--remove\s+/"
            ],
            true,
            Some(,
        )
    }

    fn write_to_disk_device() -> SafetyRule {
        Self::rule(
            "SAFETY-016",
            "Write to Disk Device",
            "Writing data directly to disk devices",
            ViolationType::DataDestruction,
            RuleAction::Block,
            vec![
                r">\s*/dev/[sh]d[a-z]$",
                r">\s*/dev/nvme\d+n\d+$",
                r"cat\s+.*>\s*/dev/[sh]d[a-z]\d*",
                r"echo\s+.*>\s*/dev/[sh]d[a-z]\d*",
            ],
            true,
            Some("Writing directly to disk devices destroys filesystems. Use proper tools for disk operations."),
        )
    }

    // === SUDO MISUSE ===

    fn sudo_misuse() -> SafetyRule {
        Self::rule(
            "SAFETY-017",
            "Sudo Misuse",
            "Potentially dangerous sudo commands",
            ViolationType::SudoMisuse,
            RuleAction::Warn,
            vec![
                r"sudo\s+rm\s+-[rf]*\s+/",
                r"sudo\s+mkfs",
                r"sudo\s+dd\s+.*of=/dev/",
            ],
            true,
            Some(,
        )
    }

    fn sudo_rm_rf() -> SafetyRule {
        Self::rule(
            "SAFETY-018",
            "Sudo Recursive Delete",
            "Using sudo with recursive delete",
            ViolationType::SudoMisuse,
            RuleAction::Block,
            vec![
                r"sudo\s+rm\s+-[rf]*\s+/\s*$",
                r"sudo\s+rm\s+-[rf]*\s+/\*",
                r"sudo\s+rm\s+-[rf]*\s+/\.\.",
            ],
            true,
            Some("This will delete your entire system. If you really need this, use a more targeted approach."),
        )
    }

    fn sudo_bash() -> SafetyRule {
        Self::rule(
            "SAFETY-019",
            "Sudo Shell",
            "Opening a root shell with sudo",
            ViolationType::SudoMisuse,
            RuleAction::Warn,
            vec![
                r"sudo\s+(bash|sh|zsh|fish)$", r"sudo\s+-i$", r"sudo\s+su$"
            ],
            true,
            Some(,
        )
    }

    // === DANGEROUS PIPELINES ===

    fn dangerous_pipeline() -> SafetyRule {
        Self::rule(
            "SAFETY-020",
            "Dangerous Pipeline",
            "Piping curl or wget directly to shell",
            ViolationType::DangerousPipeline,
            RuleAction::Warn,
            vec![
                r"curl\s+.*\|\s*(bash|sh|zsh)",
                r"curl\s+.*\|\s*sudo",
                r"wget\s+.*-O-\s*\|\s*(bash|sh|zsh)",
                r"wget\s+.*-O-\s*\|\s*sudo",
            ],
            true,
            Some("Piping internet content directly to shell is dangerous. Download first, verify, then execute."),
        )
    }

    fn curl_pipe_bash() -> SafetyRule {
        Self::rule(
            "SAFETY-021",
            "Curl Pipe to Bash",
            "Downloading and executing remote scripts without verification",
            ViolationType::DangerousPipeline,
            RuleAction::Block,
            vec![
                r"curl\s+.*https?://.*\|\s*(bash|sh)",
            ],
            true,
            Some("This executes remote code without verification. Use: curl -O file.sh && cat file.sh && bash file.sh"),
        )
    }

    fn wget_pipe_sh() -> SafetyRule {
        Self::rule(
            "SAFETY-022",
            "Wget Pipe to Shell",
            "Downloading and executing remote scripts without verification",
            ViolationType::DangerousPipeline,
            RuleAction::Block,
            vec![
                r"wget\s+-[qO]*\s+[^\s]*\s*-\s*\|\s*(sh|bash)",
                r"wget\s+[^|]*-O\s*-\s*[^|]*\|\s*(sh|bash)",
            ],
            true,
            Some("This executes remote code without verification. Use: wget file.sh && cat file.sh && bash file.sh"),
        )
    }

    // === GIT DESTRUCTION ===

    fn git_destruction() -> SafetyRule {
        Self::rule(
            "SAFETY-023",
            "Git Destructive Operation",
            "Potentially destructive git operations",
            ViolationType::GitDestruction,
            RuleAction::Warn,
            vec![
                r"git\s+push\s+--force",
                r"git\s+push\s+-f",
                r"git\s+reset\s+--hard",
            ],
            true,
            Some(,
        )
    }

    fn git_force_push() -> SafetyRule {
        Self::rule(
            "SAFETY-024",
            "Git Force Push to Main",
            "Force pushing to main or master branch",
            ViolationType::GitDestruction,
            RuleAction::Block,
            vec![
                r"git\s+push\s+(--force|-f)\s+(origin\s+)?(main|master)",
                r"git\s+push\s+(origin\s+)?(main|master)\s+(--force|-f)",
            ],
            true,
            Some("Force pushing to main/master rewrites shared history. Use revert or fix-forward instead."),
        )
    }

    fn git_reset_hard() -> SafetyRule {
        Self::rule(
            "SAFETY-025",
            "Git Hard Reset",
            "Hard reset with uncommitted changes",
            ViolationType::GitDestruction,
            RuleAction::Warn,
            vec![
                r"git\s+reset\s+--hard\s+(HEAD|origin)",
                r"git\s+reset\s+--hard\s+~\d+",
            ],
            true,
            Some(,
        )
    }

    // === DATABASE DESTRUCTION ===

    fn database_destruction() -> SafetyRule {
        Self::rule(
            "SAFETY-026",
            "Database Destructive Operation",
            "Potentially destructive database operations",
            ViolationType::DatabaseDestruction,
            RuleAction::Warn,
            vec![
                r"mysql\s+.*DROP", r"psql\s+.*DROP", r"mongo\s+.*drop"
            ],
            true,
            Some("Database operations cannot be undone. Ensure you have backups."),
        )
    }

    fn drop_database() -> SafetyRule {
        Self::rule(
            "SAFETY-027",
            "Drop Database",
            "Dropping an entire database",
            ViolationType::DatabaseDestruction,
            RuleAction::Block,
            vec![
                r"DROP\s+DATABASE\s+\w+", r"DROP\s+SCHEMA\s+\w+"
            ],
            true,
            Some(,
        )
    }

    fn delete_without_where() -> SafetyRule {
        Self::rule(
            "SAFETY-028",
            "Delete Without WHERE",
            "DELETE statement without WHERE clause",
            ViolationType::DatabaseDestruction,
            RuleAction::Block,
            vec![
                r"DELETE\s+FROM\s+\w+\s*$",
                r"DELETE\s+FROM\s+\w+\s+;",
                r"DELETE\s+\w+\s+FROM\s+\w+\s*$",
            ],
            true,
            Some(,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_rules_have_unique_ids() {
        let rules = HardRules::all_rules();
        let ids: Vec<_> = rules.iter().map(|r| r.id).collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().cloned().collect();
        assert_eq!(ids.len(), unique_ids.len(), "Rule IDs must be unique");
    }

    #[test]
    fn test_destructive_wildcard_patterns() {
        let rules = HardRules::all_rules();
        let wildcard_rule = rules.iter().find(|r| r.id == "SAFETY-001").unwrap();
        assert_eq!(
            wildcard_rule.violation_type,
            ViolationType::DestructiveWildcard
        );
        assert_eq!(wildcard_rule.action, RuleAction::Block);
    }
}
