use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Clone)]
pub struct ChatSession {
    pub messages: Vec<Message>,
}

impl ChatSession {
    pub fn new(safe_mode: bool) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "/home/user".to_string());
        let platform = if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "unknown"
        };

        let env_context = format!(
            "Environment: Current working directory is '{}', running on {} platform.",
            cwd, platform
        );

        let base_instructions = "Convert natural language requests into POSIX shell commands. \
                               Use actual paths, not placeholders like '/path/to/'. \
                               Commands should work in the current environment. \
                               Prefer robust commands that handle errors gracefully.";

        let safety_note = if safe_mode {
            "Avoid destructive operations, never format disks, and avoid sudo. \
             When in doubt, prefer read-only commands and conservative actions."
        } else {
            "The user will review all commands before running."
        };

        let system_prompt = format!(
            "You are a CLI assistant. {}\n\n{}\n\n{}",
            env_context, base_instructions, safety_note
        );

        let messages = vec![Message {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        }];

        Self { messages }
    }

    pub fn push_user(&mut self, content: String) {
        self.messages.push(Message {
            role: "user".to_string(),
            content,
        });
    }

    pub fn push_assistant(&mut self, content: String) {
        self.messages.push(Message {
            role: "assistant".to_string(),
            content,
        });
    }
}
