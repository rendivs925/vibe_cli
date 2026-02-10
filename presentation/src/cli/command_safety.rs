pub struct BlockedCommand {
    pub reason: String,
}

pub fn blocked_reason(command: &str) -> Option<BlockedCommand> {
    let c = command.to_ascii_lowercase();
    let toks: Vec<&str> = c.split_whitespace().collect();
    let first = toks.first().copied().unwrap_or("");

    // Package managers / installers
    if c.contains("pacman") || c.contains("apt ") || c.contains("dnf ") || c.contains("yum ") {
        return Some(BlockedCommand {
            reason: "package manager command".to_string(),
        });
    }

    // Dangerous primaries
    if first == "rm" || first == "dd" || first.starts_with("mkfs") {
        return Some(BlockedCommand {
            reason: "destructive command".to_string(),
        });
    }

    // Power / disruption actions
    let bad_anywhere = [
        "shutdown",
        "reboot",
        "poweroff",
        "halt",
        "init 0",
        "telinit 0",
        "systemctl reboot",
        "systemctl poweroff",
        "systemctl halt",
        "systemctl shutdown",
        ":(){",
        "killall",
    ];
    if bad_anywhere.iter().any(|b| c.contains(b)) {
        return Some(BlockedCommand {
            reason: "disruptive command".to_string(),
        });
    }

    // dd patterns
    if c.contains("dd") && c.contains("if=") {
        return Some(BlockedCommand {
            reason: "dangerous dd usage".to_string(),
        });
    }

    // Extra destructive patterns
    let dangerous_patterns = [
        "rm -rf",
        "rm -r",
        "dd if=",
        "mkfs",
        "format",
        "shred",
        "wipe",
        "fdisk",
        "sfdisk",
        "parted",
        "sgdisk",
        "wipefs",
        "dd of=",
        "> /dev",
        "< /dev",
        "2> /dev",
        "chmod -r 777 /",
        "chmod -r 666 /",
        "chown -r /",
        "curl | sh",
        "curl | bash",
        "wget | sh",
        "wget | bash",
        "bash <(",
        "sh <(",
        "drop database",
        "drop schema",
        "delete from",
        "git push --force",
        "git push -f",
        "git reset --hard",
    ];
    if dangerous_patterns.iter().any(|pattern| c.contains(pattern)) {
        return Some(BlockedCommand {
            reason: "dangerous filesystem operation".to_string(),
        });
    }

    // Shell injection patterns
    let injection_patterns = [
        "; rm",
        "&& rm",
        "|| rm",
        "$(rm",
        "`rm`",
        "| rm",
        "> rm",
        "< rm",
    ];
    if injection_patterns.iter().any(|pattern| command.contains(pattern)) {
        return Some(BlockedCommand {
            reason: "shell injection pattern".to_string(),
        });
    }

    None
}

pub fn is_blocked_command(command: &str) -> bool {
    blocked_reason(command).is_some()
}
