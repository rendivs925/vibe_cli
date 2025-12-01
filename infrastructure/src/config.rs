use dotenvy::dotenv;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

fn find_project_root() -> Option<String> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if current.join("Cargo.toml").exists() {
            return Some(current.display().to_string());
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn project_cache_suffix() -> String {
    if let Some(root) = find_project_root() {
        let mut hasher = DefaultHasher::new();
        root.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    } else {
        "global".to_string()
    }
}

pub struct Config {
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub db_path: String,
}

impl Config {
    pub fn load() -> Self {
        dotenv().ok();
        let db_path = env::var("DB_PATH").unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let mut path = PathBuf::from(home);
            path.push(".local");
            path.push("share");
            path.push("vibe_cli");
            let suffix = project_cache_suffix();
            path.push(format!("{}_embeddings.db", suffix));
            path.to_string_lossy().to_string()
        });
        Self {
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            ollama_model: env::var("BASE_MODEL")
                .unwrap_or_else(|_| "qwen2.5:1.5b-instruct".to_string()),
            db_path,
        }
    }
}
