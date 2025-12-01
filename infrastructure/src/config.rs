use dotenvy::dotenv;
use std::env;
use std::path::PathBuf;

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
            path.push("embeddings.db");
            path.to_string_lossy().to_string()
        });
        Self {
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            ollama_model: env::var("BASE_MODEL")
                .unwrap_or_else(|_| "qwen2.5-coder:3b".to_string()),
            db_path,
        }
    }
}
