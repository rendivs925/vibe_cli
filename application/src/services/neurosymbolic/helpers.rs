//! Neurosymbolic Service Helpers
//!
//! Command parsing and validation helper functions

use super::types::CommandSegment;

/// Normalize a command for comparison
pub(crate) fn normalize_command(command: &str) -> String {
    let trimmed = command.trim().trim_end_matches(';').trim();
    trimmed
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Strip sudo prefix from command
pub(crate) fn strip_sudo(command: &str) -> String {
    command.strip_prefix("sudo ").unwrap_or(command).to_string()
}

/// Check if a command matches a template pattern
pub(crate) fn command_matches_template(command: &str, template: &str) -> bool {
    let cmd_segments = parse_segments(command);
    let tpl_segments = parse_segments(template);
    if cmd_segments.is_empty() || tpl_segments.is_empty() {
        return false;
    }
    if cmd_segments.len() != tpl_segments.len() {
        return false;
    }

    for (cmd_seg, tpl_seg) in cmd_segments.iter().zip(tpl_segments.iter()) {
        if cmd_seg.cmd != tpl_seg.cmd {
            return false;
        }
        if !flags_subset(&tpl_seg.flags, &cmd_seg.flags) {
            return false;
        }
    }

    true
}

/// Get mismatch reason between command and template
pub(crate) fn mismatch_reason(command: &str, template: &str) -> Option<String> {
    let cmd_segments = parse_segments(command);
    let tpl_segments = parse_segments(template);
    if cmd_segments.is_empty() || tpl_segments.is_empty() {
        return Some("unable to parse command segments".to_string());
    }
    if cmd_segments.len() != tpl_segments.len() {
        return Some(format!(
            "segment count mismatch (got {}, expected {})",
            cmd_segments.len(),
            tpl_segments.len()
        ));
    }

    for (cmd_seg, tpl_seg) in cmd_segments.iter().zip(tpl_segments.iter()) {
        if cmd_seg.cmd != tpl_seg.cmd {
            return Some(format!(
                "tool mismatch (got '{}', expected '{}')",
                cmd_seg.cmd, tpl_seg.cmd
            ));
        }
        let missing = missing_flags(&tpl_seg.flags, &cmd_seg.flags);
        if !missing.is_empty() {
            return Some(format!("missing flags: {}", missing.join(", ")));
        }
    }

    None
}

/// Parse command into segments
pub(crate) fn parse_segments(command: &str) -> Vec<CommandSegment> {
    let normalized = normalize_command(command);
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let parts: Vec<&str> = normalized.split('|').collect();

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(segment) = parse_segment(part) {
            segments.push(segment);
        }
    }

    segments
}

/// Parse a single command segment
fn parse_segment(segment: &str) -> Option<CommandSegment> {
    let tokens: Vec<&str> = segment.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let cmd = tokens[0].to_string();
    let mut flags = Vec::new();

    for token in &tokens[1..] {
        if token.starts_with('-') {
            // Handle combined flags like -la
            if token.len() > 2 && !token.starts_with("--") {
                for ch in token[1..].chars() {
                    flags.push(format!("-{}", ch));
                }
            } else {
                flags.push(token.to_string());
            }
        }
    }

    Some(CommandSegment { cmd, flags })
}

/// Check if actual flags contain all required flags
fn flags_subset(required: &[String], actual: &[String]) -> bool {
    for req in required {
        if !actual.contains(req) {
            return false;
        }
    }
    true
}

/// Find missing flags
fn missing_flags(required: &[String], actual: &[String]) -> Vec<String> {
    required
        .iter()
        .filter(|req| !actual.contains(req))
        .cloned()
        .collect()
}

/// Extract tool name from command
pub(crate) fn extract_tool(command: &str) -> Option<String> {
    let normalized = normalize_command(command);
    normalized.split_whitespace().next().map(|s| s.to_string())
}

/// Check if command is a compound command (has pipes)
pub(crate) fn is_compound_command(command: &str) -> bool {
    normalize_command(command).contains(" | ")
}

/// Split compound command into parts
pub(crate) fn split_compound(command: &str) -> Vec<String> {
    normalize_command(command)
        .split(" | ")
        .map(|s| s.to_string())
        .collect()
}
