use crate::cli::cache::CommandCandidate;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    ExplicitPrefix = 0,
    CodeFence = 1,
    InlineBackticks = 2,
    PromptLine = 3,
    OperatorLine = 4,
}

pub fn extract_command(raw: &str, user_query: &str) -> Option<String> {
    extract_commands(raw, user_query)
        .into_iter()
        .next()
        .map(|candidate| candidate.command)
}

pub fn extract_commands(raw: &str, user_query: &str) -> Vec<CommandCandidate> {
    let raw = raw.trim();

    fn normalize(mut s: &str) -> String {
        s = s.trim();

        // Strip common prompt markers
        for p in ["$ ", "# ", "> "] {
            if let Some(rest) = s.strip_prefix(p) {
                s = rest.trim_start();
                break;
            }
        }

        // Strip shell wrapper prefixes (with or without newlines/spaces)
        // Handles: "bash cmd", "bash\ncmd", "sh cmd", "sh\ncmd", "zsh cmd", "zsh\ncmd"
        for shell in ["bash", "sh", "zsh"] {
            if let Some(rest) = s.strip_prefix(shell) {
                let rest_trim = rest.trim_start();
                // Only strip if it's NOT a real shell invocation like: "bash -lc ..."
                if !rest_trim.starts_with('-') {
                    s = rest_trim;
                    break; // strip at most one wrapper
                }
            }
        }

        s.trim()
            .trim_matches('`')
            .trim_matches('"')
            .trim_matches('\'')
            .trim()
            .trim_end_matches(';')
            .trim_end_matches('.')
            .trim()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn is_forbidden(cmd: &str) -> bool {
        let c = cmd.to_ascii_lowercase();
        let toks: Vec<&str> = c.split_whitespace().collect();
        let first = toks.first().copied().unwrap_or("");

        // Package managers / installers
        if c.contains("pacman") || c.contains("apt ") || c.contains("dnf ") || c.contains("yum ") {
            return true;
        }

        // Dangerous primaries
        if first == "rm" || first == "dd" || first.starts_with("mkfs") {
            return true;
        }

        // Power / disruption actions
        let bad_anywhere = ["shutdown", "reboot", "poweroff", ":(){", "killall"];
        if bad_anywhere.iter().any(|b| c.contains(b)) {
            return true;
        }

        // dd patterns
        if c.contains("dd") && c.contains("if=") {
            return true;
        }

        false
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

        let lower = t.to_ascii_lowercase();

        // Reject obvious prose / UI noise - expanded list
        let bad_prefixes = [
            "to ",
            "run ",
            "then ",
            "next ",
            "this will",
            "choose ",
            "selected:",
            "generated",
            "method",
            "in `",
            "if you",
            "you can",
            "here are",
            "these commands",
            "this command",
            "open a terminal",
            "get the",
            "show the",
            "check the",
            "display the",
            "list the",
            "find the",
            "retrieve the",
            "execute ",
            "run the",
            "please ",
            "you can ",
            "here is",
            "use the",
            "you'll need",
            "install ",
            "download ",
            "create ",
            "make sure",
            "cpu ",
            "disk ",
            "memory ",
            "hostname",
            "operating",
            "platform",
            "kernel",
            "shell:",
            "total ",
            "free ",
            "cpu type",
            "processor",
            "information:",
            "generated command",
            "replacing xx",
        ];
        if bad_prefixes.iter().any(|p| lower.starts_with(p)) {
            return false;
        }

        // Reject numbered list items like "[1] Get the CPU" or "1] Get the CPU"
        if t.starts_with('[')
            || (t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.contains("] "))
        {
            return false;
        }

        // Reject URLs / markdown links
        if lower.contains("http://") || lower.contains("https://") {
            return false;
        }

        // Reject numbered/bulleted markdown lines
        if t.starts_with('-') || t.starts_with('*') || t.starts_with('•') {
            return false;
        }
        if t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            let rest: String = t.chars().skip_while(|c| c.is_ascii_digit()).collect();
            let r = rest.trim_start();
            if r.starts_with('.') || r.starts_with(')') {
                return false;
            }
        }

        // Reject headings
        if t.ends_with(':') {
            return false;
        }

        // Reject bare language tags
        if matches!(lower.as_str(), "bash" | "sh" | "zsh" | "shell" | "console") {
            return false;
        }

        // First token must be a plausible executable/path
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

        // If it looks like a sentence or description, require shell syntax
        // These phrases indicate prose, not commands
        let sentencey = lower.contains(" you ")
            || lower.contains(" this ")
            || lower.contains(" should ")
            || lower.contains(" able ")
            || lower.contains(" provides ")
            || lower.contains(" display ")
            || lower.starts_with("get ")
            || lower.starts_with("show ")
            || lower.starts_with("check ")
            || lower.starts_with("list ")
            || lower.starts_with("find ")
            || lower.starts_with("display ")
            || lower.starts_with("retrieve ")
            || lower.starts_with("execute ")
            || lower.starts_with("run ")
            || lower.starts_with("install ")
            || lower.starts_with("download ")
            || lower.starts_with("create ")
            || lower.starts_with("make ")
            || lower.starts_with("please ")
            || lower.starts_with("use ")
            || lower.starts_with("here ")
            || lower.starts_with("you'll");

        let has_shell_signal = t.contains('|')
            || t.contains("&&")
            || t.contains("||")
            || t.contains(';')
            || t.contains(" -")
            || t.contains("--")
            || t.contains(">/")
            || t.contains("</")
            || t.contains("$(")
            || t.contains('`')
            || t.contains('/'); // Paths indicate commands

        let tokens: Vec<&str> = t.split_whitespace().collect();
        let first_tok = tokens.first().copied().unwrap_or("");
        let second_tok = tokens.get(1).copied().unwrap_or("");
        let allow_no_signal = matches!(
            first_tok,
            "systemctl" | "service"
        ) || (first_tok == "sudo" && matches!(second_tok, "systemctl" | "service"));

        if sentencey && !has_shell_signal {
            return false;
        }

        // Require *some* signal beyond a single bare word, unless allowlisted
        let token_count = t.split_whitespace().count();
        if token_count == 1 {
            // Allowlist for common single-word info commands
            let ok = matches!(
                lower.as_str(),
                "htop"
                    | "top"
                    | "free"
                    | "uname"
                    | "nvidia-smi"
                    | "ls"
                    | "ps"
                    | "df"
                    | "du"
                    | "who"
                    | "w"
                    | "uptime"
                    | "hostname"
                    | "arch"
                    | "date"
                    | "cal"
                    | "last"
                    | "id"
                    | "groups"
                    | "users"
            );
            return ok;
        }

        // For multi-word, require shell signal or be very confident
        if token_count >= 2 && !has_shell_signal && !allow_no_signal {
            return false;
        }

        true
    }

    fn query_keywords(query: &str) -> Vec<String> {
        // Tiny heuristic: split and keep “word-ish” tokens.
        // Used only as a soft filter later.
        query
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
            .map(str::trim)
            .filter(|w| w.len() >= 3)
            .map(|w| w.to_string())
            .collect()
    }

    fn matches_query(cmd: &str, keywords: &[String]) -> bool {
        if keywords.is_empty() {
            return true;
        }
        let c = cmd.to_ascii_lowercase();
        keywords.iter().any(|k| c.contains(k))
    }

    fn push_candidate(
        out: &mut Vec<(Source, CommandCandidate)>,
        src: Source,
        raw_cmd: &str,
        q_keywords: &[String],
    ) {
        let cmd = normalize(raw_cmd);
        if cmd.is_empty() {
            return;
        }
        if !looks_like_command(&cmd) || is_forbidden(&cmd) {
            return;
        }

        // Relevance filter (soft):
        // - Always accept high-confidence sources.
        // - For low-confidence sources, require at least one query keyword match.
        let high_conf = matches!(
            src,
            Source::ExplicitPrefix | Source::CodeFence | Source::InlineBackticks
        );
        if !high_conf && !matches_query(&cmd, q_keywords) {
            return;
        }

        out.push((src, CommandCandidate::new(cmd)));
    }

    let q_keywords = query_keywords(user_query);

    let mut found: Vec<(Source, CommandCandidate)> = Vec::new();

    // 1) Explicit prefixes (highest confidence)
    for line in raw.lines() {
        let l = line.trim();
        for p in ["COMMAND:", "Command:", "CMD:"] {
            if let Some(rest) = l.strip_prefix(p) {
                push_candidate(&mut found, Source::ExplicitPrefix, rest, &q_keywords);
            }
        }
    }

    // 2) Code fences
    {
        let mut in_fence = false;
        for line in raw.lines() {
            let t = line.trim_end();
            if t.trim_start().starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                // ignore fence language tags accidentally inside
                if matches!(t.trim(), "bash" | "sh" | "zsh" | "shell" | "console") {
                    continue;
                }
                push_candidate(&mut found, Source::CodeFence, t, &q_keywords);
            }
        }
    }

    // 3) Inline backticks
    {
        let mut start = None;
        for (i, ch) in raw.char_indices() {
            if ch == '`' {
                if let Some(st) = start {
                    let snippet = &raw[st..i];
                    // Skip single-word inline backticks that look like command mentions in prose
                    // Only allow multi-word commands or commands with shell signals
                    let normalized = normalize(snippet);
                    let token_count = normalized.split_whitespace().count();
                    let has_shell_signal = normalized.contains('|')
                        || normalized.contains("&&")
                        || normalized.contains("||")
                        || normalized.contains(';')
                        || normalized.contains(" -")
                        || normalized.contains("--")
                        || normalized.contains(">/")
                        || normalized.contains("</");

                    if token_count > 1 || has_shell_signal {
                        push_candidate(&mut found, Source::InlineBackticks, snippet, &q_keywords);
                    }
                    start = None;
                } else {
                    start = Some(i + 1);
                }
            }
        }
    }

    // 4) Prompt-style / UI-style lines (lower confidence)
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("$ ")
            || t.starts_with("# ")
            || t.starts_with("> ")
            || t.starts_with("bash ")
        {
            push_candidate(&mut found, Source::PromptLine, t, &q_keywords);
            continue;
        }

        // 5) Operator-heavy lines (lowest confidence)
        if t.contains('|') || t.contains("&&") || t.contains("||") {
            push_candidate(&mut found, Source::OperatorLine, t, &q_keywords);
        }
    }

    // Rank by source first, then command lexicographically for stable output
    found.sort_by(|(sa, a), (sb, b)| sa.cmp(sb).then_with(|| a.command.cmp(&b.command)));

    // Dedup by command string, keeping best-ranked (lowest Source)
    let mut seen_commands = std::collections::HashSet::new();
    let mut out: Vec<CommandCandidate> = Vec::new();
    for (_src, cand) in found {
        if seen_commands.contains(&cand.command) {
            continue;
        }
        seen_commands.insert(cand.command.clone());
        out.push(cand);
    }

    out
}

pub fn looks_like_shell_command(s: &str) -> bool {
    // Keep this for backward compatibility if other code calls it.
    // Delegate to the stricter matcher used by extract_commands.
    fn inner(s: &str) -> bool {
        let t = s.trim();
        if t.is_empty() {
            return false;
        }
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("to ") || lower.starts_with("run ") || lower.starts_with("then ") {
            return false;
        }
        if t.starts_with('-')
            || t.starts_with('*')
            || t.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            return false;
        }

        let first = t.split_whitespace().next().unwrap_or("");
        let starts_ok = first
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c));

        let has_signal = t.contains('|')
            || t.contains("&&")
            || t.contains("||")
            || t.contains(';')
            || t.contains('/')
            || t.contains(" -")
            || t.contains("--");

        // Require more than just a bare word unless it’s allowlisted.
        let token_count = t.split_whitespace().count();
        if token_count == 1 {
            return matches!(lower.as_str(), "htop" | "top" | "free" | "uname" | "ls");
        }

        starts_ok && has_signal
    }

    inner(s)
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
