//! Utility functions for CLI operations

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
