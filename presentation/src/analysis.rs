use crate::types::{CommandIntent, AgentCommandRisk, CommandRisk};

/// Analyze user query to determine intent
pub fn analyze_query_intent(query: &str) -> CommandIntent {
    let query_lower = query.to_lowercase();

    // Installation keywords
    let install_keywords = [
        "install",
        "setup",
        "add",
        "create",
        "build",
        "compile",
        "download",
        "get",
        "fetch",
        "deploy",
        "configure",
    ];

    // Configuration keywords
    let config_keywords = [
        "configure",
        "config",
        "enable",
        "disable",
        "set",
        "update",
        "modify",
        "change",
        "edit",
        "tune",
        "optimize",
    ];

    // Service control keywords
    let service_keywords = [
        "start",
        "stop",
        "restart",
        "reload",
        "enable",
        "disable",
        "status",
        "systemctl",
        "service",
        "daemon",
    ];

    // Information query patterns
    let info_patterns = [
        "what's",
        "what is",
        "how much",
        "how many",
        "show",
        "list",
        "display",
        "check",
        "verify",
        "info",
        "information",
    ];

    // Check for installation intent
    if install_keywords.iter().any(|&kw| query_lower.contains(kw)) {
        return CommandIntent::Installation;
    }

    // Check for configuration intent
    if config_keywords.iter().any(|&kw| query_lower.contains(kw)) {
        return CommandIntent::Configuration;
    }

    // Check for service control intent
    if service_keywords.iter().any(|&kw| query_lower.contains(kw)) {
        return CommandIntent::ServiceControl;
    }

    // Check for information queries
    if info_patterns.iter().any(|&pat| query_lower.contains(pat)) {
        return CommandIntent::InfoQuery;
    }

    // Default to system query for general queries
    if query_lower.contains("disk")
        || query_lower.contains("memory")
        || query_lower.contains("cpu")
        || query_lower.contains("gpu")
        || query_lower.contains("network")
        || query_lower.contains("process")
    {
        return CommandIntent::SystemQuery;
    }

    CommandIntent::Unknown
}

/// Assess risk level of a command for agent execution
pub fn assess_agent_command_risk(command: &str) -> AgentCommandRisk {
    let cmd_lower = command.to_lowercase();

    // Destructive commands - highest risk
    let destructive_patterns = [
        "rm -rf", "rm -r", "rmdir", "del", "delete", "format", "mkfs", "dd if=", "fdisk", "parted",
        "wipe", "shred", "unlink",
    ];

    // System-changing commands
    let system_change_patterns = [
        "chmod 777",
        "chmod 666",
        "chown root",
        "chown 0",
        "chown :root",
        "usermod",
        "userdel",
        "useradd",
        "groupmod",
        "groupdel",
        "groupadd",
        "systemctl enable",
        "systemctl disable",
        "systemctl stop",
        "ufw --force enable",
        "ufw --force disable",
        "iptables",
        "mount",
        "umount",
        "fsck",
        "tune2fs",
        "resize2fs",
    ];

    // Network access commands
    let network_patterns = [
        "curl",
        "wget",
        "git clone",
        "git pull",
        "git fetch",
        "npm install",
        "npm update",
        "yarn install",
        "yarn add",
        "pip install",
        "pip download",
        "apt install",
        "apt update",
        "yum install",
        "dnf install",
        "pacman -S",
        "brew install",
        "docker pull",
        "docker push",
        "scp",
        "rsync",
        "ssh",
    ];

    // Safe operations
    let safe_patterns = [
        "ls", "pwd", "echo", "printf", "cat", "head", "tail", "grep", "find", "which", "whereis",
        "type", "file", "stat", "du", "df", "free", "ps", "top", "htop", "uname", "whoami", "id",
        "groups", "mkdir", "touch", "cp", "mv", "ln", "basename", "dirname",
    ];

    // Info-only commands
    let info_patterns = [
        "date",
        "cal",
        "uptime",
        "w",
        "who",
        "last",
        "history",
        "env",
        "printenv",
        "locale",
        "tzselect",
        "locale-gen",
    ];

    // Check destructive first (highest priority)
    if destructive_patterns
        .iter()
        .any(|&pat| cmd_lower.contains(pat))
    {
        return AgentCommandRisk::Destructive;
    }

    // Check system changes
    if system_change_patterns
        .iter()
        .any(|&pat| cmd_lower.contains(pat))
    {
        return AgentCommandRisk::SystemChanges;
    }

    // Check network access
    if network_patterns.iter().any(|&pat| cmd_lower.contains(pat)) {
        return AgentCommandRisk::NetworkAccess;
    }

    // Check safe operations
    if safe_patterns
        .iter()
        .any(|&pat| cmd_lower.starts_with(pat) || cmd_lower.contains(&format!(" {}", pat)))
    {
        return AgentCommandRisk::SafeOperations;
    }

    // Check info-only
    if info_patterns.iter().any(|&pat| cmd_lower.starts_with(pat)) {
        return AgentCommandRisk::InfoOnly;
    }

    // Default to unknown
    AgentCommandRisk::Unknown
}

/// Assess risk level of a command
pub fn assess_command_risk(command: &str) -> CommandRisk {
    let cmd_lower = command.to_lowercase();

    // High-risk commands
    let high_risk_patterns = [
        "rm -rf",
        "format",
        "mkfs",
        "fdisk",
        "dd if=",
        "shutdown",
        "reboot",
        "halt",
        "poweroff",
        "systemctl stop",
        "killall",
    ];

    // System-changing commands
    let system_change_patterns = [
        "usermod",
        "userdel",
        "groupmod",
        "chmod 777",
        "chown root",
        "systemctl enable",
        "systemctl disable",
        "ufw",
        "firewall",
        "iptables",
        "mount",
        "umount",
    ];

    // Safe setup commands
    let safe_setup_commands = [
        "apt install",
        "apt-get install",
        "yum install",
        "dnf install",
        "pacman -S",
        "brew install",
        "pip install",
        "npm install",
        "gem install",
        "cargo install",
    ];

    // Info-only commands (read-only)
    let info_only_commands = [
        "ls", "df", "free", "ps", "top", "htop", "uname", "whoami", "pwd", "cat", "grep", "find",
        "which", "whereis", "type",
    ];

    // Check high risk first
    if high_risk_patterns
        .iter()
        .any(|&pat| cmd_lower.contains(pat))
    {
        return CommandRisk::HighRisk;
    }

    // Check system changes
    if system_change_patterns
        .iter()
        .any(|&pat| cmd_lower.contains(pat))
    {
        return CommandRisk::SystemChanges;
    }

    // Check safe setup
    if safe_setup_commands
        .iter()
        .any(|&cmd| cmd_lower.contains(cmd))
    {
        return CommandRisk::SafeSetup;
    }

    // Check info-only
    if info_only_commands
        .iter()
        .any(|&cmd| cmd_lower.starts_with(cmd))
    {
        return CommandRisk::InfoOnly;
    }

    // Default to unknown
    CommandRisk::Unknown
}

/// Validate that a command has basic syntactical correctness
pub fn validate_command_syntax(command: &str) -> std::result::Result<(), String> {
    let trimmed = command.trim();

    // Check for unclosed quotes
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for ch in trimmed.chars() {
        match ch {
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            _ => {}
        }
    }

    if in_single_quote || in_double_quote {
        return Err("Command contains unclosed quotes".to_string());
    }

    // Check for unbalanced parentheses (basic check)
    let paren_count = trimmed.chars().fold(0, |count, ch| match ch {
        '(' => count + 1,
        ')' => count - 1,
        _ => count,
    });

    if paren_count != 0 {
        return Err("Command contains unbalanced parentheses".to_string());
    }

    // Check for obviously malformed patterns
    if trimmed.contains("&&&") || trimmed.contains("|||") {
        return Err("Command contains consecutive operators".to_string());
    }

    if trimmed.starts_with('|') || trimmed.starts_with('&') || trimmed.starts_with(';') {
        return Err("Command starts with a pipe or operator".to_string());
    }

    if trimmed.ends_with('|') || trimmed.ends_with('&') {
        return Err("Command ends with a pipe or operator".to_string());
    }

    Ok(())
}

/// Check if a command typically requires sudo/admin privileges
pub fn command_needs_sudo(command: &str) -> bool {
    let sudo_commands = [
        "systemctl",
        "service",
        "systemd",
        "apt",
        "apt-get",
        "yum",
        "dnf",
        "pacman",
        "zypper",
        "mount",
        "umount",
        "fdisk",
        "mkfs",
        "fsck",
        "iptables",
        "ufw",
        "firewall-cmd",
        "usermod",
        "useradd",
        "userdel",
        "groupadd",
        "groupdel",
        "chmod",
        "chown",
        "passwd",
        "visudo",
        "crontab",
        "overwrite",
        "modify",
        "edit",
        "update",
    ];

    // Check if the command starts with any of the sudo-requiring commands
    for sudo_cmd in &sudo_commands {
        if command.starts_with(&format!("{} ", sudo_cmd)) || command == *sudo_cmd {
            return true;
        }
    }

    false
}

/// Check if a command's exit code should be considered successful despite being non-zero
pub fn is_expected_exit_code(command: &str, exit_code: Option<i32>, stderr: &str) -> bool {
    // Handle systemctl status commands - exit code 3 means service is inactive (normal)
    if (command.contains("systemctl status") || command.contains("sudo systemctl status"))
        && exit_code == Some(3)
        && !stderr.contains("Failed to")
    {
        return true;
    }

    // Add more command-specific exit code handling here as needed
    // For example:
    // if command.contains("some_command") && exit_code == Some(expected_code) {
    //     return true;
    // }

    false
}