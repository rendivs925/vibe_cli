use std::collections::HashSet;

use crate::services::research_agent_service::{ResearchDepth, ResearchSource};

pub(crate) fn summarize_sources(
    sources: &[&ResearchSource],
    depth: &ResearchDepth,
) -> Vec<String> {
    if sources.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for source in sources {
        if let Some(line) = extract_summary_line(&source.content) {
            candidates.push(line);
        }
    }

    if candidates.is_empty() {
        candidates = sources.iter().map(|s| s.title.clone()).collect();
    }

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for line in candidates {
        let key = line.to_lowercase();
        if seen.insert(key) {
            deduped.push(line);
        }
    }

    deduped.truncate(summary_limit(depth));
    deduped
}

fn extract_summary_line(text: &str) -> Option<String> {
    let first_line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let cleaned = first_line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.len() < 30 {
        return None;
    }

    let lower = cleaned.to_lowercase();
    if lower.starts_with("could not fetch")
        || lower.starts_with("access denied")
        || lower.starts_with("forbidden")
        || lower.starts_with("<!doctype")
        || lower.starts_with("<html")
        || lower.starts_with("%pdf")
    {
        return None;
    }

    Some(truncate_chars(&cleaned, 220))
}

fn truncate_chars(text: &str, max_len: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    for ch in text.chars() {
        if count >= max_len {
            break;
        }
        out.push(ch);
        count += 1;
    }
    if text.chars().count() > max_len {
        out.push_str("...");
    }
    out
}

fn summary_limit(depth: &ResearchDepth) -> usize {
    match depth {
        ResearchDepth::Quick => 3,
        ResearchDepth::Standard => 5,
        ResearchDepth::Deep => 8,
        ResearchDepth::Comprehensive => 12,
    }
}
