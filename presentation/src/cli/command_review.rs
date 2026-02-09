use crate::cli::cache::CommandCandidate;
use crate::cli::command_extraction::{matches_query, query_keywords};
use crate::cli::command_safety::{blocked_reason, is_blocked_command};
use infrastructure::syntax_grammar_validator::{SyntaxGrammarValidator, ValidationResult};
use std::path::Path;
use std::process::Command;

pub struct ReviewedCandidates {
    pub usable: Vec<CommandCandidate>,
    pub rejected: Vec<RejectedCandidate>,
}

pub struct RejectedCandidate {
    pub command: String,
    pub reasons: Vec<String>,
}

pub struct CommandReview {
    pub warnings: Vec<String>,
    pub reasons: Vec<String>,
}

impl CommandReview {
    pub fn is_usable(&self) -> bool {
        self.reasons.is_empty()
    }

    pub fn label_with_existing(&self, existing: Option<&str>) -> Option<String> {
        if self.warnings.is_empty() {
            existing.map(|s| s.to_string())
        } else {
            let warnings = self.warnings.join("; ");
            if let Some(label) = existing {
                Some(format!("{}; {}", label, warnings))
            } else {
                Some(warnings)
            }
        }
    }
}

pub fn review_candidates(
    candidates: &[CommandCandidate],
    user_query: &str,
    validator: &mut SyntaxGrammarValidator,
) -> ReviewedCandidates {
    let keywords = query_keywords(user_query);
    let suppress_low_relevance = candidates.len() == 1;
    let mut usable = Vec::new();
    let mut rejected = Vec::new();

    for candidate in candidates {
        let review = review_command(
            &candidate.command,
            &keywords,
            validator,
            candidate.label.as_deref(),
            suppress_low_relevance,
        );
        if review.is_usable() {
            let mut updated = candidate.clone();
            if let Some(label) = review.label_with_existing(updated.label.as_deref()) {
                updated = updated.with_label(label);
            }
            usable.push(updated);
        } else {
            rejected.push(RejectedCandidate {
                command: candidate.command.clone(),
                reasons: review.reasons,
            });
        }
    }

    ReviewedCandidates { usable, rejected }
}

fn review_command(
    command: &str,
    keywords: &[String],
    validator: &mut SyntaxGrammarValidator,
    existing_label: Option<&str>,
    suppress_low_relevance: bool,
) -> CommandReview {
    let mut warnings = Vec::new();
    let mut reasons = Vec::new();

    if is_blocked_command(command) {
        let reason = blocked_reason(command)
            .map(|r| r.reason)
            .unwrap_or_else(|| "blocked by safety policy".to_string());
        reasons.push(reason);
        return CommandReview { warnings, reasons };
    }

    let segments = command_segments(command);
    if segments.is_empty() {
        reasons.push("unable to parse command".to_string());
        return CommandReview { warnings, reasons };
    }

    let mut missing_bins = Vec::new();
    let mut invalid_flags = Vec::new();
    let mut manpage_missing = false;

    for segment in segments {
        let seg = segment.trim();
        if seg.is_empty() {
            continue;
        }

        if let Some(cmd) = base_command(seg) {
            if !binary_exists(&cmd) {
                missing_bins.push(cmd);
            }
        }

        if seg.contains('-') {
            let result = validator.validate(seg);
            collect_validation(result, &mut invalid_flags, &mut manpage_missing);
        }
    }

    if !missing_bins.is_empty() {
        missing_bins.sort();
        missing_bins.dedup();
        reasons.push(format!("missing binary: {}", missing_bins.join(", ")));
    }

    if !invalid_flags.is_empty() {
        invalid_flags.sort();
        invalid_flags.dedup();
        reasons.push(format!("invalid flags: {}", invalid_flags.join(", ")));
    }

    if manpage_missing {
        warnings.push("manpage unavailable".to_string());
    }

    let is_symbolic = existing_label
        .map(|label| label.contains("symbolic"))
        .unwrap_or(false);

    CommandReview { warnings, reasons }
}

fn collect_validation(
    result: ValidationResult,
    invalid_flags: &mut Vec<String>,
    manpage_missing: &mut bool,
) {
    if !result.manpage_available {
        *manpage_missing = true;
    }
    if !result.invalid_flags.is_empty() {
        invalid_flags.extend(result.invalid_flags);
    }
}

fn command_segments(command: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '|' {
            segments.push(current.trim().to_string());
            current.clear();
            continue;
        }

        if ch == ';' {
            segments.push(current.trim().to_string());
            current.clear();
            continue;
        }

        if ch == '&' {
            if matches!(chars.peek(), Some('&')) {
                chars.next();
                segments.push(current.trim().to_string());
                current.clear();
                continue;
            }
        }

        current.push(ch);
    }

    if !current.trim().is_empty() {
        segments.push(current.trim().to_string());
    }

    segments
}

fn base_command(segment: &str) -> Option<String> {
    let parts: Vec<&str> = segment.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let mut idx = 0;
    if parts[0] == "sudo" {
        idx = 1;
    }

    let cmd = parts.get(idx)?;
    Some(cmd.to_string())
}

fn binary_exists(cmd: &str) -> bool {
    let builtins = [
        "echo", "cd", "pwd", "ls", "cat", "grep", "find", "which", "type",
    ];
    if builtins.contains(&cmd) {
        return true;
    }

    if cmd.contains('/') {
        return Path::new(cmd).exists();
    }

    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
