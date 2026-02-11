use crate::cli::cache::CommandCandidate;
use crate::cli::command_safety::is_blocked_command;
use infrastructure::ai_command_extractor::OllamaCommandExtractor;
use shared::command_extraction::{
    extract_candidate_commands, query_keywords as shared_query_keywords,
};
use std::env;

pub fn extract_command(raw: &str, user_query: &str) -> Option<String> {
    extract_commands(raw, user_query)
        .into_iter()
        .next()
        .map(|candidate| candidate.command)
}

fn should_use_ai_extractor() -> bool {
    if cfg!(test) {
        return false;
    }
    match env::var("VIBE_CLI_DISABLE_AI_EXTRACT") {
        Ok(value) if value == "1" || value.eq_ignore_ascii_case("true") => false,
        _ => true,
    }
}

fn try_ai_extract(raw: &str) -> Option<String> {
    if raw.trim().is_empty() || !should_use_ai_extractor() {
        return None;
    }
    let extractor = OllamaCommandExtractor::new().ok()?;
    extractor.extract(raw)
}

pub(crate) fn query_keywords(query: &str) -> Vec<String> {
    shared_query_keywords(query)
}

pub fn extract_commands(raw: &str, user_query: &str) -> Vec<CommandCandidate> {
    let raw = raw.trim();
    let mut ordered = Vec::new();

    if let Some(ai_cmd) = try_ai_extract(raw) {
        ordered.push(ai_cmd);
    }

    ordered.extend(extract_candidate_commands(raw, user_query));

    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<CommandCandidate> = Vec::new();
    for cmd in ordered {
        if cmd.is_empty() || is_blocked_command(&cmd) || seen.contains(&cmd) {
            continue;
        }
        seen.insert(cmd.clone());
        out.push(CommandCandidate::new(cmd));
    }

    out
}

pub fn looks_like_shell_command(s: &str) -> bool {
    extract_candidate_commands(s, "").first().is_some()
}

pub fn clean_command_output(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 3
            && lines[0].trim().starts_with("```")
            && lines.last().map(|l| l.trim()) == Some("```")
        {
            return lines[1..lines.len() - 1].join("\n").trim().to_string();
        }
    }
    trimmed.to_string()
}

pub fn extract_command_from_response(response: &str) -> String {
    let response = response.trim();
    let cleaned = if response.starts_with("```bash") && response.ends_with("```") {
        let start = response.find('\n').unwrap_or(0) + 1;
        let end = response.len() - 3;
        response[start..end].trim().to_string()
    } else if response.starts_with("```") && response.ends_with("```") {
        let start = response.find('\n').unwrap_or(0) + 1;
        let end = response.len() - 3;
        response[start..end].trim().to_string()
    } else {
        response.to_string()
    };
    if let Some(cmd) = extract_command(&cleaned, "") {
        return cmd;
    }
    cleaned
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

pub fn extract_last_json(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        return Some(trimmed);
    }
    let bytes = trimmed.as_bytes();
    let mut depth = 0_i32;
    let mut start = None;

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'{' || b == b'[' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if b == b'}' || b == b']' {
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start {
                    return Some(&trimmed[s..=i]);
                }
            }
        }
    }
    None
}

pub fn extract_json_array(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
    let mut start = None;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match b {
            b'"' => in_string = !in_string,
            b'\\' => {
                if in_string {
                    escape_next = true;
                }
            }
            b'[' => {
                if !in_string && depth == 0 {
                    start = Some(i);
                }
                if !in_string {
                    depth += 1;
                }
            }
            b']' => {
                if !in_string {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            return Some(&text[s..=i]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

pub fn parse_agent_plan(raw: &str) -> Vec<String> {
    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(raw) {
        return cmds;
    }
    let cleaned = clean_command_output(raw);
    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(&cleaned) {
        return cmds;
    }
    if let Some(arr) = extract_json_array(raw) {
        if let Ok(cmds) = serde_json::from_str::<Vec<String>>(arr) {
            return cmds;
        }
    }
    if let Some(json) = extract_last_json(raw) {
        if let Ok(cmds) = serde_json::from_str::<Vec<String>>(json) {
            return cmds;
        }
    }

    raw.lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty() && !l.starts_with("```") && !l.ends_with("```") && *l != "[" && *l != "]"
        })
        .map(|l| {
            let mut line = l
                .trim_start_matches(|c| c == '-' || c == '*' || c == '•')
                .trim();

            if let Some(pos) = line.find(|c: char| c == ')' || c == '.' || c == ':') {
                if pos < 4 {
                    line = line[pos + 1..].trim();
                }
            }

            line.trim_matches(',')
                .trim()
                .trim_matches('"')
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_command_simple() {
        let input = "lspci | grep -i nvidia";
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }

    #[test]
    fn test_extract_command_from_code_fence() {
        let input = r#"```bash
lspci | grep -i nvidia
```"#;
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }

    #[test]
    fn test_forbidden_commands() {
        let forbidden_commands = [
            "sudo rm -rf /",
            "dd if=/dev/zero of=/dev/sda",
            "mkfs.ext4 /dev/sda1",
            ":(){ :|:& };:",
            "shutdown -h now",
        ];

        for cmd in forbidden_commands {
            let result = extract_command(cmd, "");
            assert_eq!(result, None, "Should block forbidden command: {}", cmd);
        }
    }

    #[test]
    fn test_clean_command_output() {
        let input = r#"```bash
lspci | grep -i nvidia
```"#;
        let result = clean_command_output(input);
        assert_eq!(result, "lspci | grep -i nvidia");
    }

    #[test]
    fn test_parse_agent_plan_json() {
        let input = r#"["lspci | grep -i nvidia", "nvidia-smi"]"#;
        let result = parse_agent_plan(input);
        assert_eq!(result, vec!["lspci | grep -i nvidia", "nvidia-smi"]);
    }

    #[test]
    fn test_command_normalization_inline_backticks() {
        let input = "  `lspci  | grep  -i  nvidia`  ";
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }

    #[test]
    fn test_strip_bash_prefix() {
        let input = "bash free -h";
        let result = extract_command(input, "free memory");
        println!("Input: {:?}", input);
        println!("Result: {:?}", result);
        assert_eq!(result, Some("free -h".to_string()));
    }

    #[test]
    fn test_prompt_prefix() {
        let input = "$ cat /proc/meminfo | grep MemTotal";
        let result = extract_command(input, "memory info");
        assert_eq!(
            result,
            Some("cat /proc/meminfo | grep MemTotal".to_string())
        );
    }

    #[test]
    fn test_reject_prose_lines() {
        let input = r#"
The `free` command provides a summary of memory usage in the system.
This will display memory usage in human-readable format.
Choose the method that best fits your needs and environment.
"#;
        let cmds = extract_commands(input, "ram memory");
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_dedup_bash_prefix() {
        // Test that duplicates with "bash " prefix are properly deduplicated
        let input = r#"
bash lshw -short | grep memory
lshw -short | grep memory
```bash
lshw -short | grep memory
```
"#;
        let cmds = extract_commands(input, "memory hardware");
        // Should only have one command, not duplicates
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command, "lshw -short | grep memory");
    }

    #[test]
    fn test_operator_line_low_confidence_requires_query_match() {
        // Has operator, but unrelated to query keywords; should be filtered out
        let input = "echo hello | wc -c";
        let cmds = extract_commands(input, "ram memory");
        assert!(cmds.is_empty());

        // Now with matching keyword, it can pass
        let input2 = "echo memory | wc -c";
        let cmds2 = extract_commands(input2, "ram memory");
        assert_eq!(cmds2[0].command, "echo memory | wc -c");
    }
}
