use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::env;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub agent_execution: AgentExecutionConfig,
    pub resource_limits: ResourceLimitsConfig,
    pub network_security: NetworkSecurityConfig,
    pub content_sanitization: ContentSanitizationConfig,
    pub audit_trail: AuditTrailConfig,
    pub feature_flags: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionConfig {
    pub max_iterations: u32,
    pub max_tools_per_iteration: u32,
    pub max_execution_time_seconds: u64,
    pub verification_timeout_seconds: u64,
    pub allow_iteration_on_failure: bool,
    pub convergence_threshold: f32,
    pub time_bounds_per_iteration_seconds: u64,
    pub memory_limit_mb: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitsConfig {
    pub max_memory_mb: u64,
    pub max_cpu_percentage: u32,
    pub max_file_operations: u32,
    pub max_network_requests: u32,
    pub sandbox_enabled: bool,
    pub cgroups_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSecurityConfig {
    pub allowed_domains: Vec<String>,
    pub blocked_domains: Vec<String>,
    pub max_request_size_kb: u64,
    pub timeout_seconds: u64,
    pub enable_ssl_verification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSanitizationConfig {
    pub prompt_injection_detection: bool,
    pub sql_injection_detection: bool,
    pub secret_detection: bool,
    pub allowed_content_types: Vec<String>,
    pub max_content_length_kb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrailConfig {
    pub enabled: bool,
    pub log_level: String,
    pub max_log_files: u32,
    pub max_log_size_mb: u64,
    pub log_directory: String,
    pub structured_logging: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            agent_execution: AgentExecutionConfig::default(),
            resource_limits: ResourceLimitsConfig::default(),
            network_security: NetworkSecurityConfig::default(),
            content_sanitization: ContentSanitizationConfig::default(),
            audit_trail: AuditTrailConfig::default(),
            feature_flags: HashMap::new(),
        }
    }
}

impl Default for AgentExecutionConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_tools_per_iteration: 3,
            max_execution_time_seconds: 120,
            verification_timeout_seconds: 30,
            allow_iteration_on_failure: true,
            convergence_threshold: 0.8,
            time_bounds_per_iteration_seconds: 60,
            memory_limit_mb: Some(512),
        }
    }
}

impl Default for ResourceLimitsConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 1024,
            max_cpu_percentage: 80,
            max_file_operations: 1000,
            max_network_requests: 50,
            sandbox_enabled: true,
            cgroups_enabled: false,
        }
    }
}

impl Default for NetworkSecurityConfig {
    fn default() -> Self {
        Self {
            allowed_domains: vec![
                "localhost".to_string(),
                "*.githubusercontent.com".to_string(),
                "*.wikipedia.org".to_string(),
            ],
            blocked_domains: vec![
                "*.malicious-site.com".to_string(),
            ],
            max_request_size_kb: 1024,
            timeout_seconds: 30,
            enable_ssl_verification: true,
        }
    }
}

impl Default for ContentSanitizationConfig {
    fn default() -> Self {
        Self {
            prompt_injection_detection: true,
            sql_injection_detection: true,
            secret_detection: true,
            allowed_content_types: vec![
                "text/plain".to_string(),
                "text/markdown".to_string(),
                "application/json".to_string(),
            ],
            max_content_length_kb: 512,
        }
    }
}

impl Default for AuditTrailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            log_level: "INFO".to_string(),
            max_log_files: 10,
            max_log_size_mb: 100,
            log_directory: "./logs".to_string(),
            structured_logging: true,
        }
    }
}

/// Context window management configuration
#[derive(Clone)]
pub struct ContextConfig {
    pub max_file_size_bytes: u64,
    pub max_files_in_context: usize,
    pub max_context_tokens: usize,
    pub max_file_preview_lines: usize,
    pub token_estimation_ratio: f32, // chars per token estimate
    pub max_plan_attempts: usize,
    pub max_search_candidates: usize, // Max files to scan when searching
    pub max_search_results: usize, // Max results to return from search
    pub max_keywords_for_search: usize,
    pub max_lines_per_keyword: usize,
    pub max_rg_context_snippets: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_file_size_bytes: 2 * 1024 * 1024, // 2MB per file
            max_files_in_context: 10,
            max_context_tokens: 8000, // Reserve space for response
            max_file_preview_lines: 200,
            token_estimation_ratio: 4.0, // ~4 chars per token for English
            max_plan_attempts: 3,
            max_search_candidates: 100,
            max_search_results: 5,
            max_keywords_for_search: 3,
            max_lines_per_keyword: 4,
            max_rg_context_snippets: 8,
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub db_path: String,
    pub rag_include_patterns: Vec<String>,
    pub rag_exclude_patterns: Vec<String>,
    pub security: SecurityConfig,
    pub context: ContextConfig,
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

        // Load security configuration
        let security = Self::load_security_config();

        // Load context configuration from environment or use defaults
        let defaults = ContextConfig::default();
        let context = ContextConfig {
            max_file_size_bytes: env::var("CONTEXT_MAX_FILE_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_file_size_bytes),
            max_files_in_context: env::var("CONTEXT_MAX_FILES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_files_in_context),
            max_context_tokens: env::var("CONTEXT_MAX_TOKENS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_context_tokens),
            max_file_preview_lines: env::var("CONTEXT_MAX_PREVIEW_LINES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_file_preview_lines),
            token_estimation_ratio: env::var("CONTEXT_TOKEN_RATIO")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.token_estimation_ratio),
            max_plan_attempts: env::var("CONTEXT_MAX_PLAN_ATTEMPTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_plan_attempts),
            max_search_candidates: env::var("CONTEXT_MAX_SEARCH_CANDIDATES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_search_candidates),
            max_search_results: env::var("CONTEXT_MAX_SEARCH_RESULTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_search_results),
            max_keywords_for_search: env::var("CONTEXT_MAX_KEYWORDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_keywords_for_search),
            max_lines_per_keyword: env::var("CONTEXT_MAX_LINES_PER_KEYWORD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_lines_per_keyword),
            max_rg_context_snippets: env::var("CONTEXT_MAX_RG_SNIPPETS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(defaults.max_rg_context_snippets),
        };

        Self {
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            ollama_model: env::var("BASE_MODEL")
                .unwrap_or_else(|_| "qwen2.5:1.5b-instruct".to_string()),
            db_path,
            rag_include_patterns,
            rag_exclude_patterns,
            security,
            context,
        }
    }

    fn load_security_config() -> SecurityConfig {
        // Try to load from environment variable or file
        if let Ok(config_path) = env::var("VIBE_SECURITY_CONFIG") {
            // Try to load from YAML/JSON file
            match Self::load_security_config_from_file(&config_path) {
                Ok(config) => return config,
                Err(e) => {
                    eprintln!("Failed to load security config from {}: {}", config_path, e);
                    // Fall back to environment variables or defaults
                }
            }
        }

        // Load from environment variables or use defaults
        Self::load_security_config_from_env()
    }

    fn load_security_config_from_file(path: &str) -> Result<SecurityConfig, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;

        // Load from JSON file
        if path.ends_with(".json") {
            serde_json::from_str(&content).map_err(Into::into)
        } else {
            // For other extensions or no extension, try JSON
            serde_json::from_str(&content).map_err(Into::into)
        }
    }

    fn load_security_config_from_env() -> SecurityConfig {
        SecurityConfig {
            agent_execution: AgentExecutionConfig {
                max_iterations: env::var("VIBE_MAX_ITERATIONS")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse().unwrap_or(5),
                max_tools_per_iteration: env::var("VIBE_MAX_TOOLS_PER_ITERATION")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse().unwrap_or(3),
                max_execution_time_seconds: env::var("VIBE_MAX_EXECUTION_TIME_SECONDS")
                    .unwrap_or_else(|_| "120".to_string())
                    .parse().unwrap_or(120),
                verification_timeout_seconds: env::var("VIBE_VERIFICATION_TIMEOUT_SECONDS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse().unwrap_or(30),
                allow_iteration_on_failure: env::var("VIBE_ALLOW_ITERATION_ON_FAILURE")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
                convergence_threshold: env::var("VIBE_CONVERGENCE_THRESHOLD")
                    .unwrap_or_else(|_| "0.8".to_string())
                    .parse().unwrap_or(0.8),
                time_bounds_per_iteration_seconds: env::var("VIBE_TIME_BOUNDS_PER_ITERATION_SECONDS")
                    .unwrap_or_else(|_| "60".to_string())
                    .parse().unwrap_or(60),
                memory_limit_mb: env::var("VIBE_MEMORY_LIMIT_MB")
                    .ok()
                    .and_then(|s| s.parse().ok()),
            },
            resource_limits: ResourceLimitsConfig {
                max_memory_mb: env::var("VIBE_MAX_MEMORY_MB")
                    .unwrap_or_else(|_| "1024".to_string())
                    .parse().unwrap_or(1024),
                max_cpu_percentage: env::var("VIBE_MAX_CPU_PERCENTAGE")
                    .unwrap_or_else(|_| "80".to_string())
                    .parse().unwrap_or(80),
                max_file_operations: env::var("VIBE_MAX_FILE_OPERATIONS")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse().unwrap_or(1000),
                max_network_requests: env::var("VIBE_MAX_NETWORK_REQUESTS")
                    .unwrap_or_else(|_| "50".to_string())
                    .parse().unwrap_or(50),
                sandbox_enabled: env::var("VIBE_SANDBOX_ENABLED")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
                cgroups_enabled: env::var("VIBE_CGROUPS_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse().unwrap_or(false),
            },
            network_security: NetworkSecurityConfig {
                allowed_domains: env::var("VIBE_ALLOWED_DOMAINS")
                    .unwrap_or_else(|_| "localhost,*.githubusercontent.com,*.wikipedia.org".to_string())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                blocked_domains: env::var("VIBE_BLOCKED_DOMAINS")
                    .unwrap_or_else(|_| "".to_string())
                    .split(',')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string())
                    .collect(),
                max_request_size_kb: env::var("VIBE_MAX_REQUEST_SIZE_KB")
                    .unwrap_or_else(|_| "1024".to_string())
                    .parse().unwrap_or(1024),
                timeout_seconds: env::var("VIBE_NETWORK_TIMEOUT_SECONDS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse().unwrap_or(30),
                enable_ssl_verification: env::var("VIBE_ENABLE_SSL_VERIFICATION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
            },
            content_sanitization: ContentSanitizationConfig {
                prompt_injection_detection: env::var("VIBE_PROMPT_INJECTION_DETECTION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
                sql_injection_detection: env::var("VIBE_SQL_INJECTION_DETECTION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
                secret_detection: env::var("VIBE_SECRET_DETECTION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
                allowed_content_types: env::var("VIBE_ALLOWED_CONTENT_TYPES")
                    .unwrap_or_else(|_| "text/plain,text/markdown,application/json".to_string())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                max_content_length_kb: env::var("VIBE_MAX_CONTENT_LENGTH_KB")
                    .unwrap_or_else(|_| "512".to_string())
                    .parse().unwrap_or(512),
            },
            audit_trail: AuditTrailConfig {
                enabled: env::var("VIBE_AUDIT_TRAIL_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse().unwrap_or(false),
                log_level: env::var("VIBE_LOG_LEVEL")
                    .unwrap_or_else(|_| "INFO".to_string()),
                max_log_files: env::var("VIBE_MAX_LOG_FILES")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse().unwrap_or(10),
                max_log_size_mb: env::var("VIBE_MAX_LOG_SIZE_MB")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse().unwrap_or(100),
                log_directory: env::var("VIBE_LOG_DIRECTORY")
                    .unwrap_or_else(|_| "./logs".to_string()),
                structured_logging: env::var("VIBE_STRUCTURED_LOGGING")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse().unwrap_or(true),
            },
            feature_flags: HashMap::new(), // Can be extended later
        }
    }
}
