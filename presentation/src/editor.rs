use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use shared::types::Result;

/// Editor integration for in-terminal editing of plans, diffs, and commands
pub struct Editor;

/// Content types that can be edited
pub enum EditContent {
    Plan(String),
    Diff(String),
    Command(String),
    File(String),
}

impl Editor {
    /// Detect the user's preferred editor from environment variables
    pub fn detect_editor() -> String {
        env::var("EDITOR")
            .or_else(|_| env::var("VISUAL"))
            .unwrap_or_else(|_| {
                // Fallback chain: nvim -> vim -> nano -> vi
                let candidates = ["nvim", "vim", "nano", "vi"];
                for candidate in &candidates {
                    if Command::new("which")
                        .arg(candidate)
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false)
                    {
                        return candidate.to_string();
                    }
                }
                "vi".to_string() // Ultimate fallback
            })
    }

    /// Edit content using the user's editor with better error handling and validation
    pub fn edit_content(content: &str, content_type: EditContent) -> Result<String> {
        let editor = Self::detect_editor();
        let temp_file = Self::create_temp_file(content, &content_type)?;

        println!("[EDIT] Opening {} in {}", Self::content_type_name(&content_type), editor);

        // Launch editor
        let status = Command::new(&editor)
            .arg(&temp_file)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to launch editor '{}': {}", editor, e))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Editor '{}' exited with error code {:?}", editor, status.code()));
        }

        // Read back the edited content
        let edited_content = fs::read_to_string(&temp_file)
            .map_err(|e| anyhow::anyhow!("Failed to read edited file: {}", e))?;

        // Clean up temp file
        let _ = fs::remove_file(&temp_file);

        let trimmed = edited_content.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("Edited content cannot be empty"));
        }

        Ok(trimmed.to_string())
    }

    /// Get human-readable name for content type
    fn content_type_name(content_type: &EditContent) -> &'static str {
        match content_type {
            EditContent::Plan(_) => "plan",
            EditContent::Diff(_) => "diff",
            EditContent::Command(_) => "command",
            EditContent::File(_) => "file",
        }
    }

    /// Create a temporary file with appropriate extension and content
    fn create_temp_file(content: &str, content_type: &EditContent) -> Result<PathBuf> {
        let (prefix, extension) = match content_type {
            EditContent::Plan(_) => ("vibe_plan", "md"),
            EditContent::Diff(_) => ("vibe_diff", "diff"),
            EditContent::Command(_) => ("vibe_cmd", "sh"),
            EditContent::File(_) => ("vibe_file", "txt"),
        };

        let mut temp_path = env::temp_dir();
        temp_path.push(format!("{}_{}.{}", prefix, std::process::id(), extension));

        fs::write(&temp_path, content)
            .map_err(|e| anyhow::anyhow!("Failed to create temp file: {}", e))?;

        Ok(temp_path)
    }

    /// Parse edited plan back into structured format
    pub fn parse_edited_plan(edited_text: &str) -> Result<Vec<String>> {
        let mut steps = Vec::new();

        for line in edited_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse numbered steps like "1. Create ./health.sh (Low)"
            if let Some(step_text) = line.strip_prefix(|c: char| c.is_ascii_digit()) {
                if let Some(step_text) = step_text.trim().strip_prefix('.') {
                    steps.push(step_text.trim().to_string());
                }
            }
        }

        if steps.is_empty() {
            return Err(anyhow::anyhow!("No valid steps found in edited plan"));
        }

        Ok(steps)
    }

    /// Validate edited command (basic syntax check)
    pub fn validate_edited_command(command: &str) -> Result<()> {
        let command = command.trim();
        if command.is_empty() {
            return Err(anyhow::anyhow!("Command cannot be empty"));
        }

        // Basic checks for common dangerous patterns
        let dangerous_patterns = [
            "rm -rf /",
            "dd if=",
            "mkfs",
            ":(){:|:&};:",
            "shutdown",
            "reboot",
            "halt",
        ];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return Err(anyhow::anyhow!("Command contains potentially dangerous pattern: {}", pattern));
            }
        }

        Ok(())
    }
}
