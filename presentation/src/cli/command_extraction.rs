use crate::cli::cache::CommandCandidate;

pub fn extract_command(raw: &str, user_query: &str) -> Option<String> {
    extract_commands(raw, user_query)
        .into_iter()
        .next()
        .map(|candidate| candidate.command)
}

pub fn extract_commands(raw: &str, _user_query: &str) -> Vec<CommandCandidate> {
    let raw = raw.trim();

    fn normalize(cmd: &str) -> String {
        cmd.trim()
            .trim_matches('`')
            .trim()
            .trim_end_matches(';')
            .trim()
            .to_string()
    }

    fn is_forbidden(cmd: &str) -> bool {
        let c = cmd.to_ascii_lowercase();

        if c.contains("pacman") || c.contains("apt ") || c.contains("dnf ") || c.contains("yum ") {
            return true;
        }
        if c.starts_with("sudo ") {
            return true;
        }
        let bad = [
            " rm ", "rm -", " dd ", "mkfs", ":(){", "shutdown", "reboot", "poweroff",
        ];
        bad.iter().any(|b| c.contains(b))
    }

    fn looks_like_command(s: &str) -> bool {
        let t = s.trim();
        if t.is_empty() {
            return false;
        }

        // Reject code fence markers
        if t.starts_with("```") {
            return false;
        }

        // Reject common markdown / prose starters
        let lower = t.to_ascii_lowercase();
        let bad_prefixes = [
            "to ",
            "run ",
            "then ",
            "next ",
            "this command",
            "these commands",
            "you can",
            "if you",
            "similarly",
            "check ",
            "open a terminal",
        ];
        if bad_prefixes.iter().any(|p| lower.starts_with(p)) {
            return false;
        }

        // Reject numbered/bulleted markdown lines
        if t.starts_with('-') || t.starts_with('*') || t.starts_with('•') {
            return false;
        }
        if t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            // "1.", "2)" etc
            let rest = t
                .chars()
                .skip_while(|c| c.is_ascii_digit())
                .collect::<String>();
            if rest.trim_start().starts_with('.') || rest.trim_start().starts_with(')') {
                return false;
            }
        }

        // Reject obvious non-commands
        if t.ends_with(':') {
            return false;
        }

        // Reject bare fence language tags
        if matches!(lower.as_str(), "bash" | "sh" | "zsh" | "shell" | "console") {
            return false;
        }

        // First token must look like an executable/command
        let first = t.split_whitespace().next().unwrap_or("");
        if first.is_empty() {
            return false;
        }
        if !first
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c))
        {
            return false;
        }

        // Must have some "command-ish" signal:
        // either arguments, operators, or a path/flag.
        let has_signal = t.split_whitespace().count() >= 2
            || t.contains('|')
            || t.contains("&&")
            || t.contains(';')
            || t.contains('/')
            || t.contains(" -")
            || t.contains("--");

        has_signal
    }

    let mut candidates: Vec<CommandCandidate> = Vec::new();

    for line in raw.lines() {
        let l = line.trim();
        for p in ["COMMAND:", "Command:", "CMD:"] {
            if let Some(rest) = l.strip_prefix(p) {
                let cmd = normalize(rest);
                if looks_like_command(&cmd) && !is_forbidden(&cmd) {
                    candidates.push(CommandCandidate::new(cmd));
                }
            }
        }
    }

    {
        let mut in_fence = false;
        for line in raw.lines() {
            let t = line.trim_end();
            if t.trim_start().starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                let cmd = normalize(t);
                if looks_like_command(&cmd) && !is_forbidden(&cmd) {
                    candidates.push(CommandCandidate::new(cmd));
                }
            }
        }
    }

    {
        let mut start = None;
        for (i, ch) in raw.char_indices() {
            if ch == '`' {
                if let Some(st) = start {
                    let snippet = &raw[st..i];
                    let cmd = normalize(snippet);
                    if looks_like_command(&cmd) && !is_forbidden(&cmd) {
                        candidates.push(CommandCandidate::new(cmd));
                    }
                    start = None;
                } else {
                    start = Some(i + 1);
                }
            }
        }
    }

    for line in raw.lines() {
        let cmd = normalize(line);
        if looks_like_command(&cmd) && !is_forbidden(&cmd) {
            candidates.push(CommandCandidate::new(cmd));
        }
    }

    candidates.sort_by(|a, b| a.command.cmp(&b.command));
    candidates.dedup_by(|a, b| a.command == b.command);

    candidates
}

pub fn looks_like_shell_command(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("to ") || lower.starts_with("run ") || lower.starts_with("then ") {
        return false;
    }
    if s.starts_with('-')
        || s.starts_with('*')
        || s.chars().next().is_some_and(|c| c.is_ascii_digit())
    {
        return false;
    }

    let has_cmd_chars = s.contains('|')
        || s.contains("&&")
        || s.contains(';')
        || s.contains('/')
        || s.contains('-');

    let first = s.split_whitespace().next().unwrap_or("");
    let starts_ok = first
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c));

    starts_ok && (has_cmd_chars || s.split_whitespace().count() >= 1)
}

pub fn clean_command_output(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 3
            && lines[0].trim().starts_with("```")
            && lines.last().unwrap().trim() == "```"
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
    cleaned
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

pub fn extract_last_json(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}')
        || trimmed.starts_with('[') && trimmed.ends_with(']')
    {
        return Some(trimmed);
    }
    let bytes = trimmed.as_bytes();
    let mut depth = 0;
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
    let mut depth = 0;
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
            line.trim_matches(',').trim().trim_matches('"').to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}
