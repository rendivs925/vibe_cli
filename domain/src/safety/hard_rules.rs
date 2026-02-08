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
        SafetyRule {
            id: "SAFETY-001",
            name: "Destructive Wildcard Pattern",
            description: "Commands using wildcards that could delete system files or home directory",
            violation_type: ViolationType::DestructiveWildcard,
            action: RuleAction::Block,
            patterns: vec![
                r"rm\s+-[rf]*\s+/",
                r"rm\s+-[rf]*\s+~/",
                r"rm\s+-[rf]*\s+\$HOME",
                r"find\s+/\s+-name\s+.*-delete",
                r"find\s+/\s+-exec\s+rm",
                r"rm\s+-[rf]*\s+\*/",
            ],
            case_insensitive: true,
            suggestion: Some("Use specific paths instead of wildcards. Consider using 'rm -i' for interactive mode."),
        }
    }

    // === SYSTEM DIRECTORY DELETION ===

    fn system_directory_deletion() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-002",
            name: "System Directory Deletion",
            description: "Attempting to delete critical system directories",
            violation_type: ViolationType::SystemDirectoryDeletion,
            action: RuleAction::Block,
            patterns: vec![
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
            case_insensitive: true,
            suggestion: Some("System directories should never be deleted. Use package manager to remove software."),
        }
    }

    // === DISK FORMATTING ===

    fn disk_formatting() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-003",
            name: "Disk Formatting",
            description: "Formatting a filesystem which will destroy all data",
            violation_type: ViolationType::DiskFormatting,
            action: RuleAction::Block,
            patterns: vec![
                r"mkfs\.\w+\s+/dev/[sh]d[a-z]\d*",
                r"mkfs\s+/dev/[sh]d[a-z]\d*",
                r"newfs\s+/dev/[sh]d[a-z]\d*",
            ],
            case_insensitive: true,
            suggestion: Some("Disk formatting destroys all data. Ensure you have backups and are targeting the correct device."),
        }
    }

    fn dd_to_disk() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-004",
            name: "Direct Disk Write",
            description: "Using dd to write directly to disk devices",
            violation_type: ViolationType::DiskFormatting,
            action: RuleAction::Block,
            patterns: vec![
                r"dd\s+.*of=/dev/[sh]d[a-z]\d*",
                r"dd\s+.*of=/dev/nvme\d+n\d+",
                r"dd\s+.*of=/dev/mmcblk\d+",
                r"dd\s+.*of=/dev/disk\d+",
            ],
            case_insensitive: true,
            suggestion: Some("Writing directly to disk devices destroys data. Double-check the output device (of=...)."),
        }
    }

    // === PERMISSION ESCALATION ===

    fn permission_escalation() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-005",
            name: "Dangerous Permission Change",
            description: "Changing permissions to make files world-writable or executable",
            violation_type: ViolationType::PermissionEscalation,
            action: RuleAction::Warn,
            patterns: vec![
                r"chmod\s+.*777\s+/etc",
                r"chmod\s+.*777\s+/usr",
                r"chmod\s+.*777\s+/bin",
                r"chmod\s+.*777\s+/var",
                r"chmod\s+-R\s+.*777\s+/",
                r"chmod\s+-R\s+.*666\s+/",
            ],
            case_insensitive: true,
            suggestion: Some("World-writable permissions are a security risk. Use more restrictive permissions like 755 or 644."),
        }
    }

    fn chmod_system_dirs() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-006",
            name: "Permission Change on System Directories",
            description: "Changing ownership or permissions of system directories",
            violation_type: ViolationType::PermissionEscalation,
            action: RuleAction::Warn,
            patterns: vec![
                r"chown\s+-R\s+.*/etc(\s|/|\z)",
                r"chown\s+-R\s+.*/usr(\s|/|\z)",
                r"chown\s+-R\s+.*/bin(\s|/|\z)",
                r"chmod\s+-R\s+.*/etc(\s|/|\z)",
                r"chmod\s+-R\s+.*/usr(\s|/|\z)",
            ],
            case_insensitive: true,
            suggestion: Some("Changing system directory permissions can break your system. Use sudo for specific operations instead."),
        }
    }

    // === NETWORK EXPOSURE ===

    fn network_exposure() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-007",
            name: "Privileged Port Exposure",
            description: "Opening network ports below 1024 (privileged ports)",
            violation_type: ViolationType::NetworkExposure,
            action: RuleAction::Warn,
            patterns: vec![
                r"iptables\s+.*-p\s+tcp\s+.*--dport\s+\d{1,3}\s",
                r"firewall-cmd\s+.*--add-port=\d{1,3}/",
                r"ufw\s+allow\s+\d{1,3}/",
            ],
            case_insensitive: true,
            suggestion: Some("Privileged ports (<1024) require root and may expose services. Ensure this is intentional."),
        }
    }

    fn iptables_flush() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-008",
            name: "Firewall Flush",
            description: "Flushing all iptables rules removes firewall protection",
            violation_type: ViolationType::NetworkExposure,
            action: RuleAction::Block,
            patterns: vec![
                r"iptables\s+-F",
                r"iptables\s+--flush",
                r"nft\s+flush\s+ruleset",
            ],
            case_insensitive: true,
            suggestion: Some("Flushing firewall rules exposes your system. Save rules first with 'iptables-save'."),
        }
    }

    // === PASSWORD EXPOSURE ===

    fn password_exposure() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-009",
            name: "Password in Command",
            description: "Command contains potential password in plain text",
            violation_type: ViolationType::PasswordExposure,
            action: RuleAction::Block,
            patterns: vec![
                r"mysql\s+-u\s+\w+\s+-p\s*\w+",
                r"psql\s+.*-W\s*\w+",
                r"redis-cli\s+.*-a\s*\w+",
                r"curl\s+.*-u\s+\w+:\w+",
                r"wget\s+.*--password[=\s]+\w+",
            ],
            case_insensitive: true,
            suggestion: Some("Never put passwords in commands - they appear in shell history and process lists. Use environment variables or config files."),
        }
    }

    fn echo_password() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-010",
            name: "Password Echoed to Command",
            description: "Echoing password into a command via pipe",
            violation_type: ViolationType::PasswordExposure,
            action: RuleAction::Warn,
            patterns: vec![
                r"echo\s+.*\|\s*mysql",
                r"echo\s+.*\|\s*sudo",
                r"echo\s+.*\|\s*su",
            ],
            case_insensitive: true,
            suggestion: Some("Piped passwords appear in shell history. Use secure authentication methods instead."),
        }
    }

    // === SERVICE DISRUPTION ===

    fn service_disruption() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-011",
            name: "Service Disruption",
            description: "Stopping or killing critical system services",
            violation_type: ViolationType::ServiceDisruption,
            action: RuleAction::Warn,
            patterns: vec![
                r"systemctl\s+stop\s+(systemd|network|ssh|sshd)",
                r"service\s+(ssh|sshd|network)\s+stop",
                r"killall\s+-9\s+(systemd|init)",
            ],
            case_insensitive: true,
            suggestion: Some("Stopping critical services may disconnect you or crash the system. Ensure you have alternative access."),
        }
    }

    fn kill_init() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-012",
            name: "Kill Init Process",
            description: "Attempting to kill init (PID 1) will crash the system",
            violation_type: ViolationType::ServiceDisruption,
            action: RuleAction::Block,
            patterns: vec![
                r"kill\s+-9\s+1\s*$",
                r"kill\s+-SIGKILL\s+1\s*$",
                r"killall\s+-9\s+systemd",
                r"killall\s+-9\s+init",
            ],
            case_insensitive: true,
            suggestion: Some("Killing the init process will crash your system immediately. This should never be done."),
        }
    }

    fn stop_ssh_while_connected() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-013",
            name: "Stop SSH While Connected",
            description: "Stopping SSH service while connected via SSH",
            violation_type: ViolationType::ServiceDisruption,
            action: RuleAction::Warn,
            patterns: vec![
                r"systemctl\s+(stop|restart)\s+(ssh|sshd)",
                r"service\s+(ssh|sshd)\s+(stop|restart)",
            ],
            case_insensitive: true,
            suggestion: Some("You appear to be connected via SSH. Stopping SSH will disconnect you. Ensure you have console access."),
        }
    }

    // === DATA DESTRUCTION ===

    fn data_destruction() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-014",
            name: "Data Destruction",
            description: "Commands that securely delete or overwrite data",
            violation_type: ViolationType::DataDestruction,
            action: RuleAction::Block,
            patterns: vec![
                r"shred\s+-[u]*\s+/etc",
                r"shred\s+-[u]*\s+/usr",
                r"shred\s+-[u]*\s+/home",
                r"shred\s+-[u]*\s+/var",
            ],
            case_insensitive: true,
            suggestion: Some("Secure deletion cannot be undone. Ensure you have backups and are targeting the correct files."),
        }
    }

    fn shred_system_files() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-015",
            name: "Shred System Files",
            description: "Using shred on system or important directories",
            violation_type: ViolationType::DataDestruction,
            action: RuleAction::Block,
            patterns: vec![r"shred\s+.*-u\s+/", r"shred\s+.*--remove\s+/"],
            case_insensitive: true,
            suggestion: Some(
                "Shred with removal permanently destroys data. Double-check your target path.",
            ),
        }
    }

    fn write_to_disk_device() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-016",
            name: "Write to Disk Device",
            description: "Writing data directly to disk devices",
            violation_type: ViolationType::DataDestruction,
            action: RuleAction::Block,
            patterns: vec![
                r">\s*/dev/[sh]d[a-z]$",
                r">\s*/dev/nvme\d+n\d+$",
                r"cat\s+.*>\s*/dev/[sh]d[a-z]\d*",
                r"echo\s+.*>\s*/dev/[sh]d[a-z]\d*",
            ],
            case_insensitive: true,
            suggestion: Some("Writing directly to disk devices destroys filesystems. Use proper tools for disk operations."),
        }
    }

    // === SUDO MISUSE ===

    fn sudo_misuse() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-017",
            name: "Sudo Misuse",
            description: "Potentially dangerous sudo commands",
            violation_type: ViolationType::SudoMisuse,
            action: RuleAction::Warn,
            patterns: vec![
                r"sudo\s+rm\s+-[rf]*\s+/",
                r"sudo\s+mkfs",
                r"sudo\s+dd\s+.*of=/dev/",
            ],
            case_insensitive: true,
            suggestion: Some(
                "Sudo amplifies the danger of destructive commands. Double-check before executing.",
            ),
        }
    }

    fn sudo_rm_rf() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-018",
            name: "Sudo Recursive Delete",
            description: "Using sudo with recursive delete",
            violation_type: ViolationType::SudoMisuse,
            action: RuleAction::Block,
            patterns: vec![
                r"sudo\s+rm\s+-[rf]*\s+/\s*$",
                r"sudo\s+rm\s+-[rf]*\s+/\*",
                r"sudo\s+rm\s+-[rf]*\s+/\.\.",
            ],
            case_insensitive: true,
            suggestion: Some("This will delete your entire system. If you really need this, use a more targeted approach."),
        }
    }

    fn sudo_bash() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-019",
            name: "Sudo Shell",
            description: "Opening a root shell with sudo",
            violation_type: ViolationType::SudoMisuse,
            action: RuleAction::Warn,
            patterns: vec![r"sudo\s+(bash|sh|zsh|fish)$", r"sudo\s+-i$", r"sudo\s+su$"],
            case_insensitive: true,
            suggestion: Some(
                "Root shells bypass all safety checks. Use sudo for specific commands instead.",
            ),
        }
    }

    // === DANGEROUS PIPELINES ===

    fn dangerous_pipeline() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-020",
            name: "Dangerous Pipeline",
            description: "Piping curl or wget directly to shell",
            violation_type: ViolationType::DangerousPipeline,
            action: RuleAction::Warn,
            patterns: vec![
                r"curl\s+.*\|\s*(bash|sh|zsh)",
                r"curl\s+.*\|\s*sudo",
                r"wget\s+.*-O-\s*\|\s*(bash|sh|zsh)",
                r"wget\s+.*-O-\s*\|\s*sudo",
            ],
            case_insensitive: true,
            suggestion: Some("Piping internet content directly to shell is dangerous. Download first, verify, then execute."),
        }
    }

    fn curl_pipe_bash() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-021",
            name: "Curl Pipe to Bash",
            description: "Downloading and executing remote scripts without verification",
            violation_type: ViolationType::DangerousPipeline,
            action: RuleAction::Block,
            patterns: vec![
                r"curl\s+.*https?://.*\|\s*(bash|sh)",
            ],
            case_insensitive: true,
            suggestion: Some("This executes remote code without verification. Use: curl -O file.sh && cat file.sh && bash file.sh"),
        }
    }

    fn wget_pipe_sh() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-022",
            name: "Wget Pipe to Shell",
            description: "Downloading and executing remote scripts without verification",
            violation_type: ViolationType::DangerousPipeline,
            action: RuleAction::Block,
            patterns: vec![
                r"wget\s+-[qO]*\s+[^\s]*\s*-\s*\|\s*(sh|bash)",
                r"wget\s+[^|]*-O\s*-\s*[^|]*\|\s*(sh|bash)",
            ],
            case_insensitive: true,
            suggestion: Some("This executes remote code without verification. Use: wget file.sh && cat file.sh && bash file.sh"),
        }
    }

    // === GIT DESTRUCTION ===

    fn git_destruction() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-023",
            name: "Git Destructive Operation",
            description: "Potentially destructive git operations",
            violation_type: ViolationType::GitDestruction,
            action: RuleAction::Warn,
            patterns: vec![
                r"git\s+push\s+--force",
                r"git\s+push\s+-f",
                r"git\s+reset\s+--hard",
            ],
            case_insensitive: true,
            suggestion: Some(
                "Force push and hard reset can permanently lose work. Ensure you have backups.",
            ),
        }
    }

    fn git_force_push() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-024",
            name: "Git Force Push to Main",
            description: "Force pushing to main or master branch",
            violation_type: ViolationType::GitDestruction,
            action: RuleAction::Block,
            patterns: vec![
                r"git\s+push\s+(--force|-f)\s+(origin\s+)?(main|master)",
                r"git\s+push\s+(origin\s+)?(main|master)\s+(--force|-f)",
            ],
            case_insensitive: true,
            suggestion: Some("Force pushing to main/master rewrites shared history. Use revert or fix-forward instead."),
        }
    }

    fn git_reset_hard() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-025",
            name: "Git Hard Reset",
            description: "Hard reset with uncommitted changes",
            violation_type: ViolationType::GitDestruction,
            action: RuleAction::Warn,
            patterns: vec![
                r"git\s+reset\s+--hard\s+(HEAD|origin)",
                r"git\s+reset\s+--hard\s+~\d+",
            ],
            case_insensitive: true,
            suggestion: Some(
                "Hard reset permanently deletes uncommitted changes. Stash or commit first.",
            ),
        }
    }

    // === DATABASE DESTRUCTION ===

    fn database_destruction() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-026",
            name: "Database Destructive Operation",
            description: "Potentially destructive database operations",
            violation_type: ViolationType::DatabaseDestruction,
            action: RuleAction::Warn,
            patterns: vec![r"mysql\s+.*DROP", r"psql\s+.*DROP", r"mongo\s+.*drop"],
            case_insensitive: true,
            suggestion: Some("Database operations cannot be undone. Ensure you have backups."),
        }
    }

    fn drop_database() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-027",
            name: "Drop Database",
            description: "Dropping an entire database",
            violation_type: ViolationType::DatabaseDestruction,
            action: RuleAction::Block,
            patterns: vec![r"DROP\s+DATABASE\s+\w+", r"DROP\s+SCHEMA\s+\w+"],
            case_insensitive: true,
            suggestion: Some(
                "Dropping a database destroys all data. Ensure you have verified backups.",
            ),
        }
    }

    fn delete_without_where() -> SafetyRule {
        SafetyRule {
            id: "SAFETY-028",
            name: "Delete Without WHERE",
            description: "DELETE statement without WHERE clause",
            violation_type: ViolationType::DatabaseDestruction,
            action: RuleAction::Block,
            patterns: vec![
                r"DELETE\s+FROM\s+\w+\s*$",
                r"DELETE\s+FROM\s+\w+\s+;",
                r"DELETE\s+\w+\s+FROM\s+\w+\s*$",
            ],
            case_insensitive: true,
            suggestion: Some(
                "DELETE without WHERE removes all rows. Add a WHERE clause to limit scope.",
            ),
        }
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
