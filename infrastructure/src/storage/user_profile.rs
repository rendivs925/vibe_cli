use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserProfile {
    pub name: Option<String>,
    pub email: Option<String>,
    pub preferred_language: Option<String>,
    pub default_shell: Option<String>,
    pub editor: Option<String>,
    pub skills: Vec<String>,
    pub work_context: Option<String>,
    pub preferences: HashMap<String, String>,
}

impl UserProfile {
    pub fn load(config_dir: &PathBuf) -> Self {
        let path = config_dir.join("user_profile.json");

        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(profile) = serde_json::from_str(&content) {
                    return profile;
                }
            }
        }

        Self::default()
    }

    pub fn save(&self, config_dir: &PathBuf) -> Result<(), String> {
        let path = config_dir.join("user_profile.json");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;

        fs::write(&path, content).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn set_skill(&mut self, skill: String) {
        if !self.skills.contains(&skill) {
            self.skills.push(skill);
        }
    }

    pub fn remove_skill(&mut self, skill: &str) {
        self.skills.retain(|s| s != skill);
    }

    pub fn set_preference(&mut self, key: String, value: String) {
        self.preferences.insert(key, value);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectContext {
    pub path: String,
    pub name: Option<String>,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub test_framework: Option<String>,
    pub last_session: Option<String>,
    pub notes: String,
    pub recent_tasks: Vec<String>,
    pub custom_commands: HashMap<String, String>,
}

impl ProjectContext {
    pub fn new(path: String) -> Self {
        Self {
            path,
            ..Default::default()
        }
    }

    pub fn load(config_dir: &PathBuf, project_path: &str) -> Self {
        let project_name = std::path::Path::new(project_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let path = config_dir
            .join("projects")
            .join(format!("{}.json", project_name));

        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(ctx) = serde_json::from_str(&content) {
                    return ctx;
                }
            }
        }

        Self::new(project_path.to_string())
    }

    pub fn save(&self, config_dir: &PathBuf) -> Result<(), String> {
        let project_name = std::path::Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let path = config_dir
            .join("projects")
            .join(format!("{}.json", project_name));

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;

        fs::write(&path, content).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn add_task(&mut self, task: String) {
        if !self.recent_tasks.contains(&task) {
            self.recent_tasks.insert(0, task);
            if self.recent_tasks.len() > 10 {
                self.recent_tasks.truncate(10);
            }
        }
    }
}

pub fn get_config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("vibe_cli")
}
