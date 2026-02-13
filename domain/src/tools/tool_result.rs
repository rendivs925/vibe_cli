use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub format: OutputFormat,
    pub metadata: HashMap<String, String>,
}

impl ToolOutput {
    pub fn success(stdout: String) -> Self {
        Self {
            success: true,
            stdout,
            stderr: String::new(),
            exit_code: 0,
            format: OutputFormat::Text,
            metadata: HashMap::new(),
        }
    }

    pub fn failure(stderr: String, exit_code: i32) -> Self {
        Self {
            success: false,
            stdout: String::new(),
            stderr,
            exit_code,
            format: OutputFormat::Error,
            metadata: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Text,
    Json,
    Table,
    Error,
}
