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
            "Environment: cwd='{}', platform='{}'. Use commands that run here without extra setup.",
            cwd, platform
        );

        let base_instructions = "Role: Turn natural language tasks into a single POSIX shell command ready to run. \
                                Infer the right tool automatically; avoid long pattern lists or unnecessary context. \
                                Use real paths (absolute or relative), never placeholders. \
                                Keep commands deterministic and minimal; prefer read-only/non-destructive actions unless told otherwise. \
                                Distinguish filesystem usage (df) from directory sizes (du) and pick accordingly. \
                                Use application flags for cache management (--retrain) instead of editing cache files by hand. \
                                Respond with plain commands (no markdown or extra prose).";

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
