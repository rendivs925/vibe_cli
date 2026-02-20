use std::collections::HashSet;

pub struct Validator;

impl Validator {
    pub fn validate_syntax(command: &str) -> bool {
        let dangerous = [
            "rm -rf", "rm -r", "dd if=", "mkfs", "format", "shred", "wipe", "fdisk", "sfdisk",
            "parted", "dd of=", "> /dev", "< /dev", "2> /dev",
        ];

        if dangerous.iter().any(|p| command.to_lowercase().contains(p)) {
            return false;
        }

        let injection = [
            "; rm", "&& rm", "|| rm", "$(rm", "`rm`", "| rm", "> rm", "< rm",
        ];

        if injection.iter().any(|p| command.contains(p)) {
            return false;
        }

        true
    }

    pub fn validate_exists(command: &str) -> bool {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        let cmd_name = parts[0];

        let builtins = [
            "echo", "cd", "pwd", "ls", "cat", "grep", "find", "which", "type",
        ];
        if builtins.contains(&cmd_name) {
            return true;
        }

        match std::process::Command::new("which")
            .arg(cmd_name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    pub fn validate(command: &str) -> bool {
        if command.trim().is_empty() {
            return false;
        }
        Self::validate_syntax(command) && Self::validate_exists(command)
    }

    pub fn semantic_similarity(prompt1: &str, prompt2: &str) -> f64 {
        let norm1 = normalize(prompt1);
        let norm2 = normalize(prompt2);

        if norm1 == norm2 {
            return 1.0;
        }

        let words1: HashSet<&str> = norm1.split_whitespace().collect();
        let words2: HashSet<&str> = norm2.split_whitespace().collect();

        let intersection: HashSet<&str> = words1.intersection(&words2).cloned().collect();
        let union: HashSet<&str> = words1.union(&words2).cloned().collect();

        if union.is_empty() {
            return 0.0;
        }

        intersection.len() as f64 / union.len() as f64
    }
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax() {
        assert!(Validator::validate_syntax("ls -la"));
        assert!(Validator::validate_syntax("echo hello"));
        assert!(!Validator::validate_syntax("rm -rf /"));
        assert!(!Validator::validate_syntax("dd if=/dev/zero"));
    }

    #[test]
    fn test_exists() {
        assert!(Validator::validate_exists("ls"));
        assert!(Validator::validate_exists("echo"));
        assert!(!Validator::validate_exists("nonexistent_xyz123"));
    }

    #[test]
    fn test_validate() {
        assert!(Validator::validate("ls -la"));
        assert!(Validator::validate("echo hello world"));
        assert!(!Validator::validate("rm -rf /"));
        assert!(!Validator::validate(""));
        assert!(!Validator::validate("   "));
    }
}
