pub fn normalize_command_candidate(input: &str) -> String {
    let mut candidate = input.trim().to_string();
    if candidate.is_empty() {
        return candidate;
    }

    let mut lines = candidate.lines().map(str::trim).filter(|l| !l.is_empty());
    if let Some(first) = lines.next() {
        if first.starts_with("```") {
            for line in lines {
                if line.starts_with("```") {
                    break;
                }
                candidate = line.to_string();
                break;
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

fn strip_command_prefix(input: &str) -> &str {
    let trimmed = input.trim_start();
    if let Some(prefix) = trimmed.get(..8) {
        if prefix.eq_ignore_ascii_case("command:") {
            return trimmed.get(8..).unwrap_or("").trim();
        }
    }
    trimmed
}
