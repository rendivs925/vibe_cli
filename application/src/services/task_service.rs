use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use shared::types::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Paused,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub steps: Vec<TaskStep>,
    pub current_step: usize,
    pub created_at: u64,
    pub updated_at: u64,
    pub result: Option<String>,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl Task {
    pub fn new(title: String, description: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id: format!("task_{}", now),
            title,
            description,
            status: TaskStatus::Pending,
            steps: Vec::new(),
            current_step: 0,
            created_at: now,
            updated_at: now,
            result: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_step(&mut self, description: String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let step = TaskStep {
            id: format!("step_{}_{}", self.id, self.steps.len()),
            description,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            created_at: now,
            completed_at: None,
        };

        self.steps.push(step);
    }

    pub fn get_current_step(&self) -> Option<&TaskStep> {
        self.steps.get(self.current_step)
    }

    pub fn get_current_step_mut(&mut self) -> Option<&mut TaskStep> {
        self.steps.get_mut(self.current_step)
    }

    pub fn next_step(&mut self) -> bool {
        if self.current_step < self.steps.len() {
            self.current_step += 1;
            true
        } else {
            false
        }
    }
}

pub struct TaskService {
    tasks: HashMap<String, Task>,
    storage_path: PathBuf,
}

impl TaskService {
    pub fn new() -> Self {
        let config_dir = infrastructure::storage::get_config_dir();
        let storage_path = config_dir.join("tasks");

        let tasks = Self::load_tasks(&storage_path);

        Self {
            tasks,
            storage_path,
        }
    }

    fn load_tasks(storage_path: &PathBuf) -> HashMap<String, Task> {
        let mut tasks = HashMap::new();

        if let Ok(entries) = fs::read_dir(storage_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(task) = serde_json::from_str::<Task>(&content) {
                            tasks.insert(task.id.clone(), task);
                        }
                    }
                }
            }
        }

        tasks
    }

    pub fn create_task(&mut self, title: String, description: String) -> String {
        let task = Task::new(title, description);
        let id = task.id.clone();

        self.tasks.insert(id.clone(), task);
        self.save_task(&self.tasks.get(&id).unwrap());

        id
    }

    pub fn get_task(&self, task_id: &str) -> Option<&Task> {
        self.tasks.get(task_id)
    }

    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.get_mut(task_id)
    }

    pub fn list_tasks(&self, status_filter: Option<TaskStatus>) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|t| {
                if let Some(ref status) = status_filter {
                    t.status == *status
                } else {
                    true
                }
            })
            .collect()
    }

    pub fn update_task(&mut self, task: Task) -> Result<()> {
        let id = task.id.clone();
        self.tasks.insert(id, task.clone());
        self.save_task(&task);
        Ok(())
    }

    fn save_task(&self, task: &Task) {
        if let Some(parent) = self.storage_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let path = self.storage_path.join(format!("{}.json", task.id));
        if let Ok(content) = serde_json::to_string_pretty(task) {
            let _ = fs::write(path, content);
        }
    }

    pub fn delete_task(&mut self, task_id: &str) -> Result<()> {
        self.tasks.remove(task_id);

        let path = self.storage_path.join(format!("{}.json", task_id));
        if path.exists() {
            fs::remove_file(path).map_err(|e| anyhow!(e.to_string()))?;
        }

        Ok(())
    }

    pub fn decompose_task(&self, task_description: &str) -> Vec<String> {
        let mut steps = Vec::new();

        let parts: Vec<&str> = task_description
            .split(|c| c == ',' || c == ';' || c == '.')
            .collect();

        for (i, part) in parts.iter().enumerate() {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }

            let step = format!("Step {}: {}", i + 1, trimmed);
            steps.push(step);
        }

        if steps.is_empty() {
            steps.push(task_description.to_string());
        }

        steps
    }
}
