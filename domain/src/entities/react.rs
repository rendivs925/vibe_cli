use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactSession {
    pub id: String,
    pub query: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: ReactStatus,
    pub steps: Vec<ReactStep>,
    pub context: HashMap<String, String>,
    pub neurosymbolic_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactStatus {
    Running,
    Completed,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactStep {
    pub id: String,
    pub session_id: String,
    pub step_type: ReactStepType,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub status: ReactStepStatus,
    pub commands: Vec<ProposedCommand>,
    pub observations: Vec<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactStepType {
    Thought,
    Action,
    Observation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactStepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedCommand {
    pub id: String,
    pub command: String,
    pub description: String,
    pub reasoning: String,
    pub approved: Option<bool>,
    pub executed: bool,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactContext {
    pub current_step: usize,
    pub iteration_count: u32,
    pub max_iterations: u32,
    pub available_tools: Vec<String>,
    pub user_preferences: HashMap<String, String>,
}

impl ReactSession {
    pub fn new(query: String, neurosymbolic_enabled: bool) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            query,
            created_at: now,
            updated_at: now,
            status: ReactStatus::Running,
            steps: Vec::new(),
            context: HashMap::new(),
            neurosymbolic_enabled,
        }
    }

    pub fn add_step(&mut self, step: ReactStep) {
        self.updated_at = Utc::now();
        self.steps.push(step);
    }

    pub fn current_step(&self) -> Option<&ReactStep> {
        self.steps.last()
    }

    pub fn complete(&mut self) {
        self.status = ReactStatus::Completed;
        self.updated_at = Utc::now();
    }

    pub fn abort(&mut self) {
        self.status = ReactStatus::Aborted;
        self.updated_at = Utc::now();
    }

    pub fn fail(&mut self) {
        self.status = ReactStatus::Failed;
        self.updated_at = Utc::now();
    }
}

impl ReactStep {
    pub fn new(session_id: String, step_type: ReactStepType, content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            step_type,
            content,
            created_at: Utc::now(),
            status: ReactStepStatus::Pending,
            commands: Vec::new(),
            observations: Vec::new(),
            reasoning: None,
        }
    }

    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = Some(reasoning);
        self
    }

    pub fn add_command(&mut self, command: ProposedCommand) {
        self.commands.push(command);
    }

    pub fn add_observation(&mut self, observation: String) {
        self.observations.push(observation);
    }

    pub fn start(&mut self) {
        self.status = ReactStepStatus::InProgress;
    }

    pub fn complete(&mut self) {
        self.status = ReactStepStatus::Completed;
    }

    pub fn fail(&mut self) {
        self.status = ReactStepStatus::Failed;
    }

    pub fn skip(&mut self) {
        self.status = ReactStepStatus::Skipped;
    }
}

impl ProposedCommand {
    pub fn new(command: String, description: String, reasoning: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            command,
            description,
            reasoning,
            approved: None,
            executed: false,
            exit_code: None,
            stdout: None,
            stderr: None,
        }
    }

    pub fn approve(&mut self) {
        self.approved = Some(true);
    }

    pub fn reject(&mut self) {
        self.approved = Some(false);
    }

    pub fn execute(&mut self, exit_code: i32, stdout: String, stderr: String) {
        self.executed = true;
        self.exit_code = Some(exit_code);
        self.stdout = Some(stdout);
        self.stderr = Some(stderr);
    }
}

impl ReactContext {
    pub fn new(max_iterations: u32) -> Self {
        Self {
            current_step: 0,
            iteration_count: 0,
            max_iterations,
            available_tools: vec![
                "read".to_string(),
                "grep".to_string(),
                "fd".to_string(),
                "rag".to_string(),
                "sed".to_string(),
                "perl".to_string(),
                "awk".to_string(),
                "apply_patch".to_string(),
                "write".to_string(),
                "remove".to_string(),
                "update".to_string(),
                "shell".to_string(),
                "pkg".to_string(),
                "svc".to_string(),
            ],
            user_preferences: HashMap::new(),
        }
    }

    pub fn next_step(&mut self) {
        self.current_step += 1;
    }

    pub fn increment_iteration(&mut self) {
        self.iteration_count += 1;
    }

    pub fn should_continue(&self) -> bool {
        self.iteration_count < self.max_iterations
    }

    pub fn add_tool(&mut self, tool: String) {
        self.available_tools.push(tool);
    }

    pub fn set_preference(&mut self, key: String, value: String) {
        self.user_preferences.insert(key, value);
    }
}
