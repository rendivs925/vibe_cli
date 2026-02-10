use std::collections::HashSet;

pub(crate) fn normalize_command(command: &str) -> String {
    let trimmed = command.trim().trim_end_matches(';').trim();
    trimmed
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join(" ")
}

pub(crate) fn strip_sudo_prefix(command: &str) -> String {
    let trimmed = command.trim_start();
    trimmed.strip_prefix("sudo ").unwrap_or(trimmed).to_string()
}

pub(crate) fn normalize_set(values: &HashSet<String>) -> HashSet<String> {
    values.iter().map(|v| normalize_command(v)).collect()
}

pub(crate) fn is_disallowed_by_learning(command: &str, failed_commands: &[String]) -> bool {
    let normalized = normalize_command(command);
    let stripped = strip_sudo_prefix(&normalized);

    failed_commands.iter().any(|failed| {
        let failed_norm = normalize_command(failed);
        failed_norm == normalized || strip_sudo_prefix(&failed_norm) == stripped
    })
}
