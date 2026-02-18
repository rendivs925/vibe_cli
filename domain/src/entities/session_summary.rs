use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Production,
    Development,
    Research,
    Staging,
}

impl Environment {
    pub fn from_env() -> Self {
        if std::env::var("PRODUCTION").is_ok() {
            Environment::Production
        } else if std::env::var("STAGING").is_ok() {
            Environment::Staging
        } else if std::env::var("RESEARCH").is_ok() {
            Environment::Research
        } else {
            Environment::Development
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub project: String,
    pub environment: Environment,
    pub temporal_anchor: DateTime<Utc>,
    pub session_id: String,
    pub iteration: u32,
    pub max_iterations: u32,
    pub task_type: Option<String>,
}

impl SessionSummary {
    pub fn new(_task: &str) -> Self {
        Self {
            project: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .to_string_lossy()
                .to_string(),
            environment: Environment::from_env(),
            temporal_anchor: Utc::now(),
            session_id: uuid::Uuid::new_v4().to_string(),
            iteration: 1,
            max_iterations: 10,
            task_type: None,
        }
    }

    pub fn with_session_id(mut self, session_id: &str) -> Self {
        if !session_id.trim().is_empty() {
            self.session_id = session_id.to_string();
        }
        self
    }

    pub fn with_task_type(mut self, task_type: &str) -> Self {
        if !task_type.trim().is_empty() {
            self.task_type = Some(task_type.to_string());
        }
        self
    }

    pub fn with_iteration(mut self, iteration: u32, max_iterations: u32) -> Self {
        self.iteration = iteration.max(1);
        self.max_iterations = max_iterations.max(1);
        self
    }

    pub fn to_markdown(&self) -> String {
        format!(
            "### ## SESSION_SUMMARY\n- **Project**: {}\n- **Environment**: {:?}\n- **Temporal_Anchor**: {}\n- **Session ID**: {}\n- **Progress**: {}/{}\n\n---\n\n",
            self.project,
            self.environment,
            self.temporal_anchor.format("%Y-%m-%d %H:%M:%S UTC"),
            self.session_id,
            self.iteration,
            self.max_iterations
        )
    }
}
