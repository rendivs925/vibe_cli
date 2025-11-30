use dotenvy::dotenv;
use std::env;

pub struct Config {
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub db_path: String,
}

impl Config {
    pub fn load() -> Self {
        dotenv().ok();
        Self {
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            ollama_model: env::var("OLLAMA_MODEL")
                .unwrap_or_else(|_| "deepseek-coder:1.3b".to_string()),
            db_path: env::var("DB_PATH").unwrap_or_else(|_| "embeddings.db".to_string()),
        }
    }
}
