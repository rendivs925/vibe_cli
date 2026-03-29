pub fn normalize_command_candidate(input: &str) -> String {
    let mut candidate = input.trim().to_string();
    if candidate.is_empty() {
        return candidate;
    }

    let mut lines = candidate.lines().map(str::trim).filter(|l| !l.is_empty());
    if let Some(first) = lines.next() {
        if first.starts_with("```") {
            // Extract first non-fence line from code block
            if let Some(line) = lines.find(|l| !l.starts_with("```")) {
                candidate = line.to_string();
            }
        } else {
            candidate = first.to_string();
        }
    }

    candidate = strip_command_prefix(&candidate)
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string();

    if candidate.eq_ignore_ascii_case("none") {
        String::new()
    } else {
        candidate
    }
}

pub fn cleanup_ai_response(response: &str) -> String {
    normalize_command_candidate(response)
}

pub fn extract_best_command(raw: &str, user_query: &str) -> Option<String> {
    extract_candidate_commands(raw, user_query)
        .into_iter()
        .next()
}

pub fn extract_candidate_commands(raw: &str, user_query: &str) -> Vec<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Vec::new();
    }

    let q_keywords = query_keywords(user_query);
    let mut found: Vec<(Source, String)> = Vec::new();

    // 1) Explicit prefixes
    for line in raw.lines() {
        let l = line.trim();
        for p in ["COMMAND:", "Command:", "CMD:"] {
            if let Some(rest) = l.strip_prefix(p) {
                push_candidate(&mut found, Source::ExplicitPrefix, rest, &q_keywords);
            }
        }
        if let Some(rest) = strip_command_label(l) {
            push_candidate(&mut found, Source::ExplicitPrefix, rest, &q_keywords);
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
                if SHELL_LANG_TAGS.contains(&t.trim()) {
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
                    let normalized = normalize_command(snippet);
                    let token_count = normalized.split_whitespace().count();
                    let has_shell_signal = has_shell_signal(&normalized);

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

    // 4) Prompt-style / UI-style lines
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

        // 5) Operator-heavy lines
        if t.contains('|') || t.contains("&&") || t.contains("||") {
            push_candidate(&mut found, Source::OperatorLine, t, &q_keywords);
        }

        // 6) Action sentence lines (e.g., "Execute the top command.")
        if looks_like_action_sentence(t) {
            push_candidate(&mut found, Source::ActionSentence, t, &q_keywords);
        }
    }

    found.sort_by(|(sa, a), (sb, b)| sa.cmp(sb).then_with(|| a.cmp(b)));

    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for (_src, cmd) in found {
        if seen.contains(&cmd) {
            continue;
        }
        seen.insert(cmd.clone());
        out.push(cmd);
    }

    out
}

pub fn query_keywords(query: &str) -> Vec<String> {
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

fn strip_command_prefix(input: &str) -> &str {
    let trimmed = input.trim_start();
    if let Some(prefix) = trimmed.get(..8) {
        if prefix.eq_ignore_ascii_case("command:") {
            return trimmed.get(8..).unwrap_or("").trim();
        }
    }
    trimmed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    ExplicitPrefix = 1,
    CodeFence = 2,
    InlineBackticks = 3,
    PromptLine = 4,
    OperatorLine = 5,
    ActionSentence = 6,
}

const BAD_PREFIXES: &[&str] = &[
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

const SHELL_LANG_TAGS: &[&str] = &["bash", "sh", "zsh", "shell", "console"];

fn push_candidate(
    out: &mut Vec<(Source, String)>,
    src: Source,
    raw_cmd: &str,
    q_keywords: &[String],
) {
    let cmd = normalize_command(raw_cmd);
    if cmd.is_empty() {
        return;
    }
    if !looks_like_command(&cmd) {
        return;
    }

    let high_conf = matches!(
        src,
        Source::ExplicitPrefix | Source::CodeFence | Source::InlineBackticks
    );
    if !high_conf && !matches_query(&cmd, q_keywords) {
        return;
    }

    out.push((src, cmd));
}

fn has_shell_signal(s: &str) -> bool {
    s.contains('|')
        || s.contains("&&")
        || s.contains("||")
        || s.contains(';')
        || s.contains(" -")
        || s.contains("--")
        || s.contains(">/")
        || s.contains("</")
        || s.contains("$(")
        || s.contains('`')
        || s.contains('/')
}

fn normalize_command(s: &str) -> String {
    let s = s.trim();
    let mut result = s.to_string();

    if let Some(start) = s.find('`') {
        if let Some(end_rel) = s[start + 1..].find('`') {
            let inner = &s[start + 1..start + 1 + end_rel];
            if !inner.trim().is_empty() {
                result = inner.trim().to_string();
            }
        }
    }

    if let Some((before, after)) = result.split_once(':') {
        let before_lower = before.to_ascii_lowercase();
        if before_lower.contains("execute")
            || before_lower.contains("run")
            || before_lower.contains("command")
            || before_lower.contains("next action")
            || before_lower.contains("final command")
        {
            let candidate = after.trim();
            if !candidate.is_empty() {
                result = candidate.to_string();
            }
        }
    }

    let lower = result.to_ascii_lowercase();
    if lower.starts_with("execute ")
        || lower.starts_with("run ")
        || lower.starts_with("use ")
        || lower.starts_with("try ")
    {
        let mut rest = result
            .splitn(2, ' ')
            .nth(1)
            .unwrap_or("")
            .trim()
            .to_string();
        if rest.to_ascii_lowercase().starts_with("the ") {
            rest = rest[4..].trim().to_string();
        }
        rest = rest
            .trim_matches('"')
            .trim_matches('\'')
            .trim_matches('`')
            .trim_end_matches('.')
            .to_string();
        let lower_rest = rest.to_ascii_lowercase();
        if lower_rest.ends_with(" command") {
            let len = rest.len().saturating_sub(" command".len());
            rest = rest[..len].trim_end().to_string();
        }
        if !rest.is_empty() {
            result = rest;
        }
    }

    result
        .trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .trim_end_matches(';')
        .trim_end_matches('.')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_action_sentence(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let starts = lower.starts_with("execute ")
        || lower.starts_with("run ")
        || lower.starts_with("use ")
        || lower.starts_with("try ");
    if !starts {
        return false;
    }
    lower.contains(" command") || line.contains('`') || line.contains('"')
}

fn strip_command_label(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let prefixes = [
        "final command",
        "command",
        "cmd",
        "next action",
        "execute",
        "run",
    ];
    let sep_pos = trimmed
        .find(':')
        .or_else(|| trimmed.find('-'))
        .or_else(|| trimmed.find('='));
    let Some(pos) = sep_pos else {
        return None;
    };
    let (left, right) = trimmed.split_at(pos);
    let cleaned_left = left.trim().trim_matches('*').trim_matches('_').trim();
    if prefixes
        .iter()
        .any(|p| cleaned_left.eq_ignore_ascii_case(p))
    {
        let candidate = right[1..].trim();
        if !candidate.is_empty() {
            return Some(candidate);
        }
    }
    if prefixes.iter().any(|p| lower.starts_with(p)) && lower.contains(':') {
        let candidate = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
        if !candidate.is_empty() {
            return Some(candidate);
        }
    }
    None
}

fn looks_like_command(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }

    if t.starts_with("```") {
        return false;
    }

    let lower = t.to_ascii_lowercase();
    let has_shell_signal = has_shell_signal(t);

    if BAD_PREFIXES.iter().any(|p| lower.starts_with(p)) && !has_shell_signal {
        return false;
    }

    if t.starts_with('[')
        || (t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.contains("] "))
    {
        return false;
    }

    if lower.contains("http://") || lower.contains("https://") {
        return false;
    }

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

    if t.ends_with(':') {
        return false;
    }

    if SHELL_LANG_TAGS.contains(&lower.as_str()) {
        return false;
    }

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

    let tokens: Vec<&str> = t.split_whitespace().collect();
    let first_tok = tokens.first().copied().unwrap_or("");
    let second_tok = tokens.get(1).copied().unwrap_or("");
    let allow_no_signal = matches!(first_tok, "systemctl" | "service")
        || (first_tok == "sudo" && matches!(second_tok, "systemctl" | "service"));

    if sentencey && !has_shell_signal {
        return false;
    }

    let token_count = t.split_whitespace().count();
    if token_count == 1 {
        return true;
    }

    if token_count >= 2 && !has_shell_signal && !allow_no_signal {
        return false;
    }

    true
}
