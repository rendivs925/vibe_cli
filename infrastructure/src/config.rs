use dotenvy::dotenv;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

fn find_project_root() -> Option<String> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        // Check for various project indicators
        let project_files = [
            "Cargo.toml",      // Rust
            "package.json",    // Node.js
            "requirements.txt", // Python
            "Pipfile",         // Python
            "pyproject.toml",  // Python
            "setup.py",        // Python
            "Makefile",        // C/C++
            "CMakeLists.txt",  // C/C++
            "configure.ac",    // C/C++
            "go.mod",          // Go
            "Gemfile",         // Ruby
            "composer.json",   // PHP
            ".git",            // Git repo as fallback
        ];

        for file in &project_files {
            if current.join(file).exists() {
                return Some(current.display().to_string());
            }
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

#[derive(Clone)]
pub struct Config {
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub db_path: String,
    pub rag_include_patterns: Vec<String>,
    pub rag_exclude_patterns: Vec<String>,
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

        // Default include patterns for common code files
        let rag_include_patterns = env::var("RAG_INCLUDE_PATTERNS")
            .unwrap_or_else(|_| "*.rs,*.js,*.ts,*.py,*.java,*.go,*.md,*.toml,*.json".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        // Default exclude patterns for build artifacts and common irrelevant files
        let rag_exclude_patterns = env::var("RAG_EXCLUDE_PATTERNS")
            .unwrap_or_else(|_| "target/**,node_modules/**,*.lock,Cargo.lock,.git/**,__pycache__/**,*.pyc,dist/**,build/**,.next/**,.cache/**".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        Self {
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            ollama_model: env::var("BASE_MODEL")
                .unwrap_or_else(|_| "qwen2.5:1.5b-instruct".to_string()),
            db_path,
            rag_include_patterns,
            rag_exclude_patterns,
        }
    }
}
