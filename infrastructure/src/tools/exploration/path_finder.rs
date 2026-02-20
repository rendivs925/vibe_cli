use crate::tools::common::run_shell;
use domain::tools::ToolError;

pub fn fuzzy_find_paths(query: &str, directory: &str, limit: usize) -> Result<Vec<String>, ToolError> {
    let directory = if directory.trim().is_empty() { "." } else { directory };
    let listing = list_files(directory)?;
    let mut scored: Vec<(i64, String)> = listing
        .into_iter()
        .filter_map(|path| score_match(query, &path).map(|score| (score, path)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.len().cmp(&b.1.len())));
    let results = scored
        .into_iter()
        .take(limit.max(1))
        .map(|(_, path)| path)
        .collect();
    Ok(results)
}

fn list_files(directory: &str) -> Result<Vec<String>, ToolError> {
    let cmd = format!(
        "if command -v fd >/dev/null 2>&1; then fd --type f --hidden --exclude .git --exclude node_modules --exclude target --exclude dist --exclude build --exclude .next . {dir}; \
         elif command -v rg >/dev/null 2>&1; then rg --files --hidden -g '!.git' -g '!node_modules' -g '!target' -g '!dist' -g '!build' -g '!.next' {dir}; \
         else find {dir} -type f 2>/dev/null; fi",
        dir = escape_shell_arg(directory)
    );
    let output = run_shell(&cmd)?;
    let mut files = Vec::new();
    for line in output.stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            files.push(trimmed.to_string());
        }
    }
    Ok(files)
}

fn score_match(query: &str, candidate: &str) -> Option<i64> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let q = query.to_lowercase();
    let c = candidate.to_lowercase();
    if let Some(idx) = c.find(&q) {
        let mut score = 1200 - idx as i64;
        if c.ends_with(&q) {
            score += 120;
        }
        if is_segment_match(&c, &q) {
            score += 80;
        }
        score -= candidate.len() as i64 / 4;
        return Some(score);
    }

    let mut score: i64 = 0;
    let mut pos = 0usize;
    let chars: Vec<char> = c.chars().collect();
    for ch in q.chars() {
        let mut found = None;
        for idx in pos..chars.len() {
            if chars[idx] == ch {
                found = Some(idx);
                break;
            }
        }
        let idx = found?;
        let boundary = idx == 0 || is_boundary(chars[idx.saturating_sub(1)]);
        if boundary {
            score += 12;
        } else {
            score += 6;
        }
        if idx == pos {
            score += 4;
        }
        score -= (idx as i64 - pos as i64).max(0);
        pos = idx + 1;
    }
    score -= candidate.len() as i64 / 5;
    Some(score)
}

fn is_segment_match(candidate: &str, query: &str) -> bool {
    candidate
        .split(|c| c == '/' || c == '-' || c == '_' || c == '.')
        .any(|part| part == query)
}

fn is_boundary(ch: char) -> bool {
    ch == '/' || ch == '-' || ch == '_' || ch == '.'
}

fn escape_shell_arg(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{}'", escaped)
}
