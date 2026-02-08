//! Syntax Grammar Validator - validates commands against man page flags
//!
//! Ensures that every generated flag actually exists in the installed
//! version of the tool by checking against parsed man pages.

use crate::manpage_crawler::ManpageCrawler;

/// Result of validating a command
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the command syntax is valid
    pub is_valid: bool,
    /// Command that was validated
    pub command: String,
    /// Invalid flags found
    pub invalid_flags: Vec<String>,
    /// Valid flags found
    pub valid_flags: Vec<String>,
    /// Warnings (deprecated flags, etc.)
    pub warnings: Vec<String>,
    /// Whether man page was available
    pub manpage_available: bool,
}

impl ValidationResult {
    /// Create a valid result
    pub fn valid(command: &str) -> Self {
        Self {
            is_valid: true,
            command: command.to_string(),
            invalid_flags: vec![],
            valid_flags: vec![],
            warnings: vec![],
            manpage_available: true,
        }
    }

    /// Create an invalid result
    pub fn invalid(command: &str, invalid_flags: Vec<String>) -> Self {
        Self {
            is_valid: false,
            command: command.to_string(),
            invalid_flags,
            valid_flags: vec![],
            warnings: vec![],
            manpage_available: true,
        }
    }

    /// Format result for display
    pub fn format_display(&self) -> String {
        if self.is_valid && self.invalid_flags.is_empty() {
            format!("OK Command '{}' syntax is valid", self.command)
        } else if !self.is_valid {
            let mut output = format!("INVALID Command '{}' has invalid flags:\n", self.command);
            for flag in &self.invalid_flags {
                output.push_str(&format!("  - {}\n", flag));
            }
            if !self.warnings.is_empty() {
                output.push_str("Warnings:\n");
                for warning in &self.warnings {
                    output.push_str(&format!("  ! {}\n", warning));
                }
            }
            output
        } else {
            format!(
                "? Command '{}' validation uncertain (man page unavailable)",
                self.command
            )
        }
    }
}

/// Validates command syntax against man pages
pub struct SyntaxGrammarValidator {
    crawler: ManpageCrawler,
}

impl SyntaxGrammarValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {
            crawler: ManpageCrawler::new(),
        }
    }

    /// Validate a command string
    pub fn validate(&mut self, command_line: &str) -> ValidationResult {
        // Extract the base command and flags
        let parts: Vec<&str> = command_line.split_whitespace().collect();
        if parts.is_empty() {
            return ValidationResult::invalid(command_line, vec!["Empty command".to_string()]);
        }

        let base_command = parts[0];
        let flags = self.extract_flags(base_command, &parts);

        // Check if we can parse the man page
        let entry = self.crawler.parse_manpage(base_command);
        let manpage_available = entry.is_some();

        if !manpage_available {
            return ValidationResult {
                is_valid: true, // Assume valid if we can't check
                command: command_line.to_string(),
                invalid_flags: vec![],
                valid_flags: flags.clone(),
                warnings: vec!["Man page unavailable, assuming flags are valid".to_string()],
                manpage_available: false,
            };
        }

        // Validate each flag
        let mut invalid_flags = vec![];
        let mut valid_flags = vec![];

        for flag in &flags {
            if self.crawler.is_valid_flag(base_command, flag) {
                valid_flags.push(flag.clone());
            } else {
                invalid_flags.push(flag.clone());
            }
        }

        if invalid_flags.is_empty() {
            ValidationResult {
                is_valid: true,
                command: command_line.to_string(),
                invalid_flags,
                valid_flags,
                warnings: vec![],
                manpage_available: true,
            }
        } else {
            ValidationResult {
                is_valid: false,
                command: command_line.to_string(),
                invalid_flags,
                valid_flags,
                warnings: vec![],
                manpage_available: true,
            }
        }
    }

    /// Validate multiple commands
    pub fn validate_batch(&mut self, commands: &[String]) -> Vec<ValidationResult> {
        commands.iter().map(|cmd| self.validate(cmd)).collect()
    }

    /// Extract flags from command parts
    fn extract_flags(&mut self, base_command: &str, parts: &[&str]) -> Vec<String> {
        let mut flags = vec![];
        let mut skip_next = false;

        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                continue; // Skip command name
            }

            if skip_next {
                skip_next = false;
                continue;
            }

            if part.starts_with('-') {
                // Prefer treating as a long flag if the man page recognizes it
                if part.len() > 2
                    && !part.starts_with("--")
                    && self.crawler.is_valid_flag(base_command, part)
                {
                    flags.push(part.to_string());
                    continue;
                }

                // Handle combined short flags (-la)
                if !part.starts_with("--") && part.len() > 2 {
                    let chars: Vec<char> = part.chars().skip(1).collect();
                    for c in chars {
                        flags.push(format!("-{}", c));
                    }
                } else {
                    // Handle regular flags
                    flags.push(part.to_string());
                }

                // Check if next part is a value for this flag
                if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                    skip_next = true;
                }
            }
        }

        flags
    }

    /// Get suggestions for invalid flags
    pub fn suggest_fixes(&mut self, command_line: &str) -> Option<String> {
        let parts: Vec<&str> = command_line.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let base_command = parts[0];
        let flags = self.extract_flags(base_command, &parts);
        let mut suggestions = vec![];

        for flag in flags {
            if !self.crawler.is_valid_flag(base_command, &flag) {
                // Try to find similar flags
                if let Some(valid_flags) = self.crawler.get_valid_flags(base_command) {
                    let similar = self.find_similar_flags(&flag, &valid_flags);
                    if !similar.is_empty() {
                        suggestions.push(format!("{} → {}", flag, similar.join(", ")));
                    }
                }
            }
        }

        if suggestions.is_empty() {
            None
        } else {
            Some(suggestions.join("\n"))
        }
    }

    /// Find similar flags using simple edit distance
    fn find_similar_flags(&self, invalid: &str, valid: &[String]) -> Vec<String> {
        let mut similar = vec![];

        for v in valid {
            if self.levenshtein_distance(invalid, v) <= 2 {
                similar.push(v.clone());
            }
        }

        similar.truncate(3); // Max 3 suggestions
        similar
    }

    /// Calculate Levenshtein distance between two strings
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();
        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        for (i, c1) in s1.chars().enumerate() {
            for (j, c2) in s2.chars().enumerate() {
                let cost = if c1 == c2 { 0 } else { 1 };
                matrix[i + 1][j + 1] = std::cmp::min(
                    std::cmp::min(
                        matrix[i][j + 1] + 1, // deletion
                        matrix[i + 1][j] + 1, // insertion
                    ),
                    matrix[i][j] + cost, // substitution
                );
            }
        }

        matrix[len1][len2]
    }

    /// Clear the underlying cache
    pub fn clear_cache(&mut self) {
        self.crawler.clear_cache();
    }
}

impl Default for SyntaxGrammarValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_flags() {
        let validator = SyntaxGrammarValidator::new();
        let parts = vec!["ls", "-la", "-h", "--directory", "/tmp"];
        let flags = validator.extract_flags("ls", &parts);

        // Should extract -l, -a (from -la), -h, --directory
        assert!(flags.iter().any(|f| f == "-l"));
        assert!(flags.iter().any(|f| f == "-a"));
        assert!(flags.iter().any(|f| f == "-h"));
        assert!(flags.iter().any(|f| f == "--directory"));
    }

    #[test]
    fn test_levenshtein_distance() {
        let validator = SyntaxGrammarValidator::new();

        assert_eq!(validator.levenshtein_distance("-l", "-l"), 0);
        assert_eq!(validator.levenshtein_distance("-l", "-la"), 1);
        assert_eq!(validator.levenshtein_distance("--file", "--files"), 1);
    }

    #[test]
    fn test_validation_result_display() {
        let valid = ValidationResult::valid("ls -la");
        assert!(valid.format_display().contains("OK"));

        let invalid = ValidationResult::invalid("ls -z", vec!["-z".to_string()]);
        assert!(invalid.format_display().contains("INVALID"));
    }
}
