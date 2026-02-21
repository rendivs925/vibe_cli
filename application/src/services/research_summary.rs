use std::collections::HashSet;

use crate::services::research_agent_service::{ResearchDepth, ResearchSource};

pub(crate) struct SummaryBlock {
    pub title: String,
    pub content: String,
}

pub(crate) fn summarize_sources(
    sources: &[&ResearchSource],
    depth: &ResearchDepth,
) -> Vec<SummaryBlock> {
    if sources.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for source in sources {
        if let Some(block) = extract_summary_block(&source.content, depth) {
            candidates.push(SummaryBlock {
                title: source.title.clone(),
                content: block,
            });
        }
    }

    if candidates.is_empty() {
        candidates = sources
            .iter()
            .map(|s| SummaryBlock {
                title: s.title.clone(),
                content: s.title.clone(),
            })
            .collect();
    }

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for block in candidates {
        let key = block.content.to_lowercase();
        if seen.insert(key) {
            deduped.push(block);
        }
    }

    deduped.truncate(summary_limit(depth));
    deduped
}

fn extract_summary_block(text: &str, depth: &ResearchDepth) -> Option<String> {
    let cleaned = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.len() < 80 {
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

    let max_len = summary_block_len(depth);
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in cleaned.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            if current.trim().len() > 20 {
                sentences.push(current.trim().to_string());
            }
            current.clear();
        }
        if sentences.iter().map(|s| s.len()).sum::<usize>() >= max_len {
            break;
        }
    }

    let mut block = if sentences.is_empty() {
        truncate_chars(&cleaned, max_len)
    } else {
        let joined = sentences.join(" ");
        truncate_chars(&joined, max_len)
    };

    block = block.trim().to_string();
    if block.len() < 80 {
        None
    } else {
        Some(format_block(&block, depth))
    }
}

fn truncate_chars(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let mut out = String::new();
    let mut count = 0;
    for ch in text.chars() {
        if count >= max_len {
            break;
        }
        out.push(ch);
        count += 1;
    }
    out.push_str("...");
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

fn summary_block_len(depth: &ResearchDepth) -> usize {
    match depth {
        ResearchDepth::Quick => 1200,
        ResearchDepth::Standard => 1600,
        ResearchDepth::Deep => 2000,
        ResearchDepth::Comprehensive => 2400,
    }
}

fn format_block(text: &str, depth: &ResearchDepth) -> String {
    let max_line_len = match depth {
        ResearchDepth::Quick => 220,
        ResearchDepth::Standard => 240,
        ResearchDepth::Deep => 260,
        ResearchDepth::Comprehensive => 280,
    };
    let max_sentences_per_line = match depth {
        ResearchDepth::Quick => 2,
        ResearchDepth::Standard => 2,
        ResearchDepth::Deep => 3,
        ResearchDepth::Comprehensive => 3,
    };

    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let trimmed = current.trim();
            if trimmed.len() > 20 {
                sentences.push(trimmed.to_string());
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }

    let mut lines = Vec::new();
    let mut line = String::new();
    let mut sentence_count = 0;
    for sentence in sentences {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }

        let mut idx = 0;
        while idx < words.len() {
            let word = words[idx];
            let extra = if line.is_empty() { word.len() } else { word.len() + 1 };
            if line.len() + extra <= max_line_len {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
                idx += 1;
            } else if line.is_empty() {
                let chunk: String = word.chars().take(max_line_len).collect();
                lines.push(chunk);
                idx += 1;
            } else {
                lines.push(line.trim().to_string());
                line.clear();
            }
        }

        sentence_count += 1;
        if sentence_count >= max_sentences_per_line {
            if !line.trim().is_empty() {
                lines.push(line.trim().to_string());
                line.clear();
            }
            sentence_count = 0;
        }
    }
    if !line.trim().is_empty() {
        lines.push(line.trim().to_string());
    }

    lines.join("\n")
}
