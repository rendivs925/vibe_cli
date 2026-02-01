use async_trait::async_trait;
use shared::error::AppError;

/// Application configuration port
#[async_trait]
pub trait ConfigLoader: Send + Sync {
    /// Load configuration from default locations
    async fn load_config(&self) -> Result<AppConfig, AppError>;

    /// Load configuration from specific path
    async fn load_config_from_path(&self, path: &str) -> Result<AppConfig, AppError>;

    /// Save configuration to path
    async fn save_config(&self, config: &AppConfig, path: &str) -> Result<(), AppError>;

    /// Get default configuration
    fn get_default_config(&self) -> AppConfig;

    /// Validate configuration
    fn validate_config(&self, config: &AppConfig) -> Result<(), AppError>;
}

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub ai: AiConfig,
    pub storage: StorageConfig,
    pub cache: CacheConfig,
    pub ui: UiConfig,
    pub safety: SafetyConfig,
    pub logging: LoggingConfig,
}

impl AppConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ai_config(mut self, config: AiConfig) -> Self {
        self.ai = config;
        self
    }

    pub fn with_storage_config(mut self, config: StorageConfig) -> Self {
        self.storage = config;
        self
    }

    pub fn with_cache_config(mut self, config: CacheConfig) -> Self {
        self.cache = config;
        self
    }

    pub fn with_ui_config(mut self, config: UiConfig) -> Self {
        self.ui = config;
        self
    }

    pub fn with_safety_config(mut self, config: SafetyConfig) -> Self {
        self.safety = config;
        self
    }

    pub fn with_logging_config(mut self, config: LoggingConfig) -> Self {
        self.logging = config;
        self
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
            storage: StorageConfig::default(),
            cache: CacheConfig::default(),
            ui: UiConfig::default(),
            safety: SafetyConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

/// AI service configuration
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub model_name: String,
    pub api_endpoint: String,
    pub api_key: Option<String>,
    pub max_tokens: usize,
    pub temperature: f32,
    pub timeout_seconds: u64,
    pub embeddings_model: String,
    pub embedding_dimensions: usize,
}

impl AiConfig {
    pub fn new(model_name: String, api_endpoint: String) -> Self {
        Self {
            model_name,
            api_endpoint,
            api_key: None,
            max_tokens: 4096,
            temperature: 0.7,
            timeout_seconds: 30,
            embeddings_model: "all-minilm".to_string(),
            embedding_dimensions: 384,
        }
    }

    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature.clamp(0.0, 2.0);
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn with_embeddings_model(mut self, model: String) -> Self {
        self.embeddings_model = model;
        self
    }

    pub fn with_embedding_dimensions(mut self, dimensions: usize) -> Self {
        self.embedding_dimensions = dimensions;
        self
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self::new("llama2".to_string(), "http://localhost:11434".to_string())
    }
}

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub database_path: String,
    pub embeddings_table: String,
    pub documents_table: String,
    pub sessions_table: String,
    pub commands_table: String,
    pub connection_pool_size: u32,
    pub backup_enabled: bool,
    pub backup_interval_hours: u32,
}

impl StorageConfig {
    pub fn new(database_path: String) -> Self {
        Self {
            database_path,
            embeddings_table: "embeddings".to_string(),
            documents_table: "documents".to_string(),
            sessions_table: "sessions".to_string(),
            commands_table: "commands".to_string(),
            connection_pool_size: 10,
            backup_enabled: true,
            backup_interval_hours: 24,
        }
    }

    pub fn with_embeddings_table(mut self, table: String) -> Self {
        self.embeddings_table = table;
        self
    }

    pub fn with_documents_table(mut self, table: String) -> Self {
        self.documents_table = table;
        self
    }

    pub fn with_sessions_table(mut self, table: String) -> Self {
        self.sessions_table = table;
        self
    }

    pub fn with_commands_table(mut self, table: String) -> Self {
        self.commands_table = table;
        self
    }

    pub fn with_connection_pool_size(mut self, size: u32) -> Self {
        self.connection_pool_size = size;
        self
    }

    pub fn with_backup_config(mut self, enabled: bool, interval_hours: u32) -> Self {
        self.backup_enabled = enabled;
        self.backup_interval_hours = interval_hours;
        self
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self::new("./vibe_cli.db".to_string())
    }
}

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub cache_dir: String,
    pub max_size_mb: u64,
    pub ttl_seconds: u64,
    pub cleanup_interval_hours: u32,
    pub command_cache_ttl: u64,
    pub query_cache_ttl: u64,
    pub embedding_cache_ttl: u64,
}

impl CacheConfig {
    pub fn new(cache_dir: String) -> Self {
        Self {
            cache_dir,
            max_size_mb: 100,
            ttl_seconds: 3600,
            cleanup_interval_hours: 6,
            command_cache_ttl: 86400,    // 24 hours
            query_cache_ttl: 1800,       // 30 minutes
            embedding_cache_ttl: 604800, // 7 days
        }
    }

    pub fn with_max_size(mut self, size_mb: u64) -> Self {
        self.max_size_mb = size_mb;
        self
    }

    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = ttl_seconds;
        self
    }

    pub fn with_cleanup_interval(mut self, hours: u32) -> Self {
        self.cleanup_interval_hours = hours;
        self
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self::new("./cache".to_string())
    }
}

/// UI configuration
#[derive(Debug, Clone)]
pub struct UiConfig {
    pub theme: String,
    pub color_enabled: bool,
    pub confirm_dangerous_commands: bool,
    pub show_context_lines: usize,
    pub max_results_displayed: usize,
    pub streaming_enabled: bool,
    pub verbose: bool,
}

impl UiConfig {
    pub fn new() -> Self {
        Self {
            theme: "default".to_string(),
            color_enabled: true,
            confirm_dangerous_commands: true,
            show_context_lines: 3,
            max_results_displayed: 10,
            streaming_enabled: true,
            verbose: false,
        }
    }

    pub fn with_theme(mut self, theme: String) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_color_enabled(mut self, enabled: bool) -> Self {
        self.color_enabled = enabled;
        self
    }

    pub fn with_confirm_dangerous_commands(mut self, enabled: bool) -> Self {
        self.confirm_dangerous_commands = enabled;
        self
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Safety configuration
#[derive(Debug, Clone)]
pub struct SafetyConfig {
    pub strict_mode: bool,
    pub allowed_file_patterns: Vec<String>,
    pub forbidden_commands: Vec<String>,
    pub require_confirmation_for_patterns: Vec<String>,
    pub log_violations: bool,
    pub max_command_length: usize,
}

impl SafetyConfig {
    pub fn new() -> Self {
        Self {
            strict_mode: false,
            allowed_file_patterns: vec![],
            forbidden_commands: vec![],
            require_confirmation_for_patterns: vec![],
            log_violations: true,
            max_command_length: 1000,
        }
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    pub fn with_allowed_patterns(mut self, patterns: Vec<String>) -> Self {
        self.allowed_file_patterns = patterns;
        self
    }

    pub fn with_forbidden_commands(mut self, commands: Vec<String>) -> Self {
        self.forbidden_commands = commands;
        self
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: Option<String>,
    pub max_file_size_mb: u64,
    pub max_files: u32,
    pub format: LogFormat,
    pub enable_console: bool,
    pub enable_file: bool,
}

impl LoggingConfig {
    pub fn new() -> Self {
        Self {
            level: "info".to_string(),
            file_path: None,
            max_file_size_mb: 10,
            max_files: 5,
            format: LogFormat::Json,
            enable_console: true,
            enable_file: false,
        }
    }

    pub fn with_level(mut self, level: String) -> Self {
        self.level = level;
        self
    }

    pub fn with_file_path(mut self, path: String) -> Self {
        self.file_path = Some(path);
        self
    }

    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Log format options
#[derive(Debug, Clone)]
pub enum LogFormat {
    Json,
    Text,
    Compact,
}
