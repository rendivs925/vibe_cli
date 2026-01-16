use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

fn find_project_root() -> Option<String> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        // Check for various project indicators
        let project_files = [
            "Cargo.toml",       // Rust
            "package.json",     // Node.js
            "requirements.txt", // Python
            "Pipfile",          // Python
            "pyproject.toml",   // Python
            "setup.py",         // Python
            "Makefile",         // C/C++
            "CMakeLists.txt",   // C/C++
            "configure.ac",     // C/C++
            "go.mod",           // Go
            "Gemfile",          // Ruby
            "composer.json",    // PHP
            ".git",             // Git repo as fallback
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
            blocked_domains: vec!["*.malicious-site.com".to_string()],
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

/// System context information gathered from the environment (like neofetch/fastfetch)
#[derive(Clone, Debug)]
pub struct SystemContext {
    pub os_type: String,
    pub distro: String,
    pub distro_id: String,
    pub kernel: String,
    pub hostname: String,
    pub current_dir: String,
    pub home_dir: String,
    pub shell: String,
    pub user: String,
    pub architecture: String,
    pub cpu_model: String,
    pub cpu_cores: String,
    pub gpu_model: String,
    pub gpu_driver: String,
    pub ram_total: String,
    pub ram_used: String,
    pub terminal: String,
    pub package_manager: String,
    pub desktop_env: String,
    pub window_manager: String,
    pub display_server: String,
    pub uptime: String,
}

impl SystemContext {
    /// Gather comprehensive system context using shell commands (like neofetch)
    pub fn gather() -> Self {
        use std::process::Command;

        // Helper function to run shell command
        let run_cmd = |cmd: &str| -> String {
            Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .unwrap_or_else(|| "Unknown".to_string())
                .trim()
                .to_string()
        };

        // Basic system info
        let os_type = std::env::consts::OS.to_string();
        let architecture = std::env::consts::ARCH.to_string();

        // Distribution info
        let distro = run_cmd("lsb_release -d 2>/dev/null | cut -f2 || grep PRETTY_NAME /etc/os-release 2>/dev/null | cut -d'\"' -f2 || echo 'Unknown'");
        let distro_id = run_cmd("lsb_release -i 2>/dev/null | cut -f2 || grep '^ID=' /etc/os-release 2>/dev/null | cut -d'=' -f2 | tr -d '\"' || echo 'unknown'");

        // Kernel and hostname
        let kernel = run_cmd("uname -r");
        let hostname = run_cmd("hostname");

        // User and directories
        let user = std::env::var("USER").unwrap_or_else(|_| run_cmd("whoami"));
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
        let current_dir = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string());

        // Shell info
        let shell = std::env::var("SHELL").unwrap_or_else(|_| run_cmd("echo $SHELL"));

        // CPU info
        let cpu_model =
            run_cmd("lscpu | grep 'Model name' | sed 's/Model name: *//' | sed 's/  */ /g'");
        let cpu_cores = run_cmd(
            "nproc --all 2>/dev/null || grep -c ^processor /proc/cpuinfo 2>/dev/null || echo '?'",
        );

        // GPU info
        let gpu_model = run_cmd("lspci 2>/dev/null | grep -i 'vga\\|3d\\|display' | head -n1 | sed 's/.*: //' || echo 'Unknown'");
        let gpu_driver = run_cmd("lspci -k 2>/dev/null | grep -A 2 -i 'vga\\|3d' | grep 'Kernel driver' | sed 's/.*: //' | head -n1 || echo 'Unknown'");

        // RAM info
        let ram_total = run_cmd("free -h 2>/dev/null | awk '/^Mem:/ {print $2}' || echo 'Unknown'");
        let ram_used = run_cmd("free -h 2>/dev/null | awk '/^Mem:/ {print $3}' || echo 'Unknown'");

        // Terminal
        let terminal = std::env::var("TERM").unwrap_or_else(|_| {
            std::env::var("TERMINAL")
                .unwrap_or_else(|_| run_cmd("ps -o comm= -p $PPID 2>/dev/null || echo 'Unknown'"))
        });

        // Package manager detection
        let package_manager = if Command::new("which")
            .arg("pacman")
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "pacman (Arch)".to_string()
        } else if Command::new("which")
            .arg("apt")
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "apt (Debian/Ubuntu)".to_string()
        } else if Command::new("which")
            .arg("dnf")
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "dnf (Fedora)".to_string()
        } else if Command::new("which")
            .arg("yum")
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "yum (RHEL/CentOS)".to_string()
        } else if Command::new("which")
            .arg("zypper")
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "zypper (openSUSE)".to_string()
        } else if Command::new("which")
            .arg("emerge")
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            "emerge (Gentoo)".to_string()
        } else {
            "unknown".to_string()
        };

        // Desktop environment and window manager
        let desktop_env = std::env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| std::env::var("DESKTOP_SESSION"))
            .unwrap_or_else(|_| run_cmd("echo $XDG_CURRENT_DESKTOP"));

        let window_manager = std::env::var("WINDOW_MANAGER").unwrap_or_else(|_| {
            run_cmd("wmctrl -m 2>/dev/null | grep 'Name:' | cut -d' ' -f2 || echo 'Unknown'")
        });

        // Display server
        let display_server = if std::env::var("WAYLAND_DISPLAY").is_ok() {
            "Wayland".to_string()
        } else if std::env::var("DISPLAY").is_ok() {
            "X11".to_string()
        } else {
            "Unknown".to_string()
        };

        // Uptime
        let uptime = run_cmd(
            "uptime -p 2>/dev/null | sed 's/up //' || uptime | awk '{print $3,$4}' | sed 's/,//'",
        );

        Self {
            os_type,
            distro,
            distro_id,
            kernel,
            hostname,
            current_dir,
            home_dir,
            shell,
            user,
            architecture,
            cpu_model,
            cpu_cores,
            gpu_model,
            gpu_driver,
            ram_total,
            ram_used,
            terminal,
            package_manager,
            desktop_env,
            window_manager,
            display_server,
            uptime,
        }
    }

    /// Format as a comprehensive string for AI context (like neofetch output)
    pub fn to_context_string(&self) -> String {
        format!(
            r#"=== SYSTEM INFORMATION ===
User: {}@{}
OS: {} ({})
Distro: {} [{}]
Kernel: {}
Architecture: {}
Uptime: {}

=== HARDWARE ===
CPU: {} ({} cores)
GPU: {} (Driver: {})
RAM: {} / {} (used/total)

=== ENVIRONMENT ===
Shell: {}
Terminal: {}
Display Server: {}
Desktop Environment: {}
Window Manager: {}

=== PACKAGE MANAGER ===
{}

=== PATHS ===
Working Directory: {}
Home Directory: {}
"#,
            self.user,
            self.hostname,
            self.os_type,
            self.distro,
            self.distro,
            self.distro_id,
            self.kernel,
            self.architecture,
            self.uptime,
            self.cpu_model,
            self.cpu_cores,
            self.gpu_model,
            self.gpu_driver,
            self.ram_used,
            self.ram_total,
            self.shell,
            self.terminal,
            self.display_server,
            self.desktop_env,
            self.window_manager,
            self.package_manager,
            self.current_dir,
            self.home_dir
        )
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
    pub max_search_results: usize,    // Max results to return from search
    pub max_keywords_for_search: usize,
    pub max_lines_per_keyword: usize,
    pub max_rg_context_snippets: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_file_size_bytes: 10 * 1024 * 1024, // 10MB per file (increased for long files)
            max_files_in_context: 20,              // Increased to handle more context
            max_context_tokens: 64000,             // Larger token budget for big projects
            max_file_preview_lines: 1000,          // Increased to show more content
            token_estimation_ratio: 4.0,           // ~4 chars per token for English
            max_plan_attempts: 5,                  // More attempts for complex tasks
            max_search_candidates: 200,            // Scan more files
            max_search_results: 10,                // Return more results
            max_keywords_for_search: 5,            // More keywords
            max_lines_per_keyword: 10,             // More context per keyword
            max_rg_context_snippets: 15,           // More snippets
        }
    }
}

/// Advanced Power User Configuration
/// Supports YAML, JSON, and TOML formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerUserConfig {
    /// Command aliases for power users
    #[serde(default)]
    pub aliases: HashMap<String, String>,

    /// Keyboard shortcuts for interactive modes
    #[serde(default)]
    pub shortcuts: HashMap<String, String>,

    /// UI theme configuration
    #[serde(default)]
    pub theme: ThemeConfig,

    /// Plugin configurations
    #[serde(default)]
    pub plugins: PluginConfig,

    /// Performance tuning options
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// Advanced safety and permission settings
    #[serde(default)]
    pub permissions: PermissionConfig,

    /// Custom editor integrations
    #[serde(default)]
    pub editors: EditorConfig,

    /// Batch operation settings
    #[serde(default)]
    pub batch: BatchConfig,

    /// Custom scripts and macros
    #[serde(default)]
    pub scripts: ScriptConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Color scheme name
    pub name: String,
    /// Custom colors (ANSI codes or named colors)
    pub colors: HashMap<String, String>,
    /// Icons for different operations
    pub icons: HashMap<String, String>,
    /// Layout preferences
    pub layout: HashMap<String, String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        let mut colors = HashMap::new();
        colors.insert("success".to_string(), "green".to_string());
        colors.insert("error".to_string(), "red".to_string());
        colors.insert("warning".to_string(), "yellow".to_string());
        colors.insert("info".to_string(), "blue".to_string());
        colors.insert("accent".to_string(), "cyan".to_string());

        let mut icons = HashMap::new();
        icons.insert("success".to_string(), "✓".to_string());
        icons.insert("error".to_string(), "✗".to_string());
        icons.insert("warning".to_string(), "⚠".to_string());
        icons.insert("info".to_string(), "ℹ".to_string());
        icons.insert("loading".to_string(), "⏳".to_string());

        let mut layout = HashMap::new();
        layout.insert("compact".to_string(), "true".to_string());
        layout.insert("show_timestamps".to_string(), "true".to_string());

        Self {
            name: "default".to_string(),
            colors,
            icons,
            layout,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Enabled plugins
    pub enabled: Vec<String>,
    /// Plugin settings
    pub settings: HashMap<String, HashMap<String, String>>,
    /// Plugin paths
    pub paths: Vec<String>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: vec![],
            settings: HashMap::new(),
            paths: vec![
                "~/.config/vibe_cli/plugins".to_string(),
                "~/.vibe_cli/plugins".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Pre-warm AI models on startup
    pub prewarm_models: bool,
    /// Cache warming strategies
    pub cache_strategy: String,
    /// Parallel processing settings
    pub parallel_jobs: usize,
    /// Memory usage limits
    pub memory_limit_mb: Option<u64>,
    /// Background processing enabled
    pub background_processing: bool,
    /// Startup optimizations
    pub startup_optimizations: bool,
    /// Model pre-warming on startup
    pub model_prewarming: bool,
}

    impl Default for PerformanceConfig {
        fn default() -> Self {
            Self {
                prewarm_models: true, // Enable model pre-warming for ultra-fast responses
                cache_strategy: "lru".to_string(),
                parallel_jobs: num_cpus::get(),
                memory_limit_mb: Some(2048),
                background_processing: true,
                startup_optimizations: true,
                model_prewarming: true, // Ultra-performance: pre-warm models
            }
        }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    /// Command allowlist (regex patterns)
    pub allowed_commands: Vec<String>,
    /// Command blocklist (regex patterns)
    pub blocked_commands: Vec<String>,
    /// File operation restrictions
    pub file_restrictions: HashMap<String, Vec<String>>,
    /// Network access controls
    pub network_access: HashMap<String, bool>,
    /// Interactive confirmation levels
    pub confirmation_level: String,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        let mut file_restrictions = HashMap::new();
        file_restrictions.insert(
            "read_only_paths".to_string(),
            vec!["/etc/passwd".to_string(), "/etc/shadow".to_string()],
        );

        let mut network_access = HashMap::new();
        network_access.insert("localhost".to_string(), true);
        network_access.insert("github.com".to_string(), true);

        Self {
            allowed_commands: vec![
                r"^ls.*$".to_string(),
                r"^cat.*$".to_string(),
                r"^grep.*$".to_string(),
                r"^find.*$".to_string(),
                // System information commands
                r"^free.*$".to_string(),
                r"^df.*$".to_string(),
                r"^du.*$".to_string(),
                r"^top.*$".to_string(),
                r"^htop.*$".to_string(),
                r"^ps.*$".to_string(),
                r"^pgrep.*$".to_string(),
                r"^pkill.*$".to_string(),
                r"^kill.*$".to_string(),
                r"^uptime.*$".to_string(),
                r"^w.*$".to_string(),
                r"^who.*$".to_string(),
                r"^last.*$".to_string(),
                r"^uname.*$".to_string(),
                r"^hostname.*$".to_string(),
                r"^date.*$".to_string(),
                r"^cal.*$".to_string(),
                r"^id.*$".to_string(),
                r"^groups.*$".to_string(),
                r"^pwd.*$".to_string(),
                r"^echo.*$".to_string(),
                r"^printf.*$".to_string(),
                r"^which.*$".to_string(),
                r"^whereis.*$".to_string(),
                r"^type.*$".to_string(),
                r"^file.*$".to_string(),
                r"^stat.*$".to_string(),
                r"^wc.*$".to_string(),
                r"^head.*$".to_string(),
                r"^tail.*$".to_string(),
                r"^sort.*$".to_string(),
                r"^uniq.*$".to_string(),
                r"^cut.*$".to_string(),
                r"^tr.*$".to_string(),
                r"^awk.*$".to_string(),
                r"^sed.*$".to_string(),
                // Network diagnostic commands
                r"^ping.*$".to_string(),
                r"^traceroute.*$".to_string(),
                r"^nslookup.*$".to_string(),
                r"^dig.*$".to_string(),
                r"^host.*$".to_string(),
                r"^curl.*$".to_string(),
                r"^wget.*$".to_string(),
                // Process management (safe operations)
                r"^nice.*$".to_string(),
                r"^renice.*$".to_string(),
                r"^ionice.*$".to_string(),
                // File operations (safe)
                r"^touch.*$".to_string(),
                r"^mkdir.*$".to_string(),
                r"^cp.*$".to_string(),
                r"^mv.*$".to_string(),
                r"^ln.*$".to_string(),
                r"^chmod.*$".to_string(),
                r"^chown.*$".to_string(),
                // Archive operations
                r"^tar.*$".to_string(),
                r"^gzip.*$".to_string(),
                r"^gunzip.*$".to_string(),
                r"^bzip2.*$".to_string(),
                r"^bunzip2.*$".to_string(),
                r"^xz.*$".to_string(),
                r"^unxz.*$".to_string(),
            ],
            blocked_commands: vec![
                r"^rm\s+-rf\s+/.*$".to_string(),
                r"^dd.*$".to_string(),
                r"mkfs.*".to_string(),
            ],
            file_restrictions,
            network_access,
            confirmation_level: "normal".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// Preferred editor command
    pub preferred_editor: String,
    /// Editor-specific integrations
    pub integrations: HashMap<String, HashMap<String, String>>,
    /// LSP configurations
    pub lsp: HashMap<String, String>,
    /// Auto-save settings
    pub auto_save: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        let mut integrations = HashMap::new();

        // VSCode integration
        let mut vscode = HashMap::new();
        vscode.insert(
            "open_file".to_string(),
            "code --goto {file}:{line}".to_string(),
        );
        integrations.insert("vscode".to_string(), vscode);

        // Vim integration
        let mut vim = HashMap::new();
        vim.insert("open_file".to_string(), "vim +{line} {file}".to_string());
        integrations.insert("vim".to_string(), vim);

        let mut lsp = HashMap::new();
        lsp.insert("rust".to_string(), "rust-analyzer".to_string());
        lsp.insert("python".to_string(), "pylsp".to_string());

        Self {
            preferred_editor: env::var("EDITOR").unwrap_or_else(|_| "nano".to_string()),
            integrations,
            lsp,
            auto_save: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Maximum concurrent operations
    pub max_concurrent: usize,
    /// Queue size for batch operations
    pub queue_size: usize,
    /// Retry settings
    pub retry_attempts: usize,
    /// Batch operation timeout
    pub timeout_seconds: u64,
    /// Progress reporting
    pub progress_reporting: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_concurrent: num_cpus::get(),
            queue_size: 1000,
            retry_attempts: 3,
            timeout_seconds: 300,
            progress_reporting: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptConfig {
    /// User-defined scripts
    pub scripts: HashMap<String, String>,
    /// Macro recordings
    pub macros: HashMap<String, Vec<String>>,
    /// Script execution settings
    pub execution: HashMap<String, String>,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self {
            scripts: HashMap::new(),
            macros: HashMap::new(),
            execution: HashMap::new(),
        }
    }
}

impl Default for PowerUserConfig {
    fn default() -> Self {
        Self {
            aliases: HashMap::new(),
            shortcuts: HashMap::new(),
            theme: ThemeConfig::default(),
            plugins: PluginConfig::default(),
            performance: PerformanceConfig::default(),
            permissions: PermissionConfig::default(),
            editors: EditorConfig::default(),
            batch: BatchConfig::default(),
            scripts: ScriptConfig::default(),
        }
    }
}

impl PowerUserConfig {
    /// Load configuration from file (YAML, JSON, or TOML)
    pub fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;

        match path.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => serde_yaml::from_str(&content).map_err(Into::into),
            Some("json") => serde_json::from_str(&content).map_err(Into::into),
            Some("toml") => toml::from_str(&content).map_err(Into::into),
            _ => {
                // Try to detect format from content
                if content.trim().starts_with('{') {
                    serde_json::from_str(&content).map_err(Into::into)
                } else if content.contains("---") || content.contains(": ") {
                    serde_yaml::from_str(&content).map_err(Into::into)
                } else {
                    toml::from_str(&content).map_err(Into::into)
                }
            }
        }
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let content = match path.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => serde_yaml::to_string(self)?,
            Some("json") => serde_json::to_string_pretty(self)?,
            Some("toml") => toml::to_string(self)?,
            _ => {
                // Default to YAML
                serde_yaml::to_string(self)?
            }
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }

    /// Get configuration file paths to search (in order of priority)
    pub fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());

        // Global config files
        paths.push(PathBuf::from(&home).join(".config/vibe_cli/config.yaml"));
        paths.push(PathBuf::from(&home).join(".config/vibe_cli/config.yml"));
        paths.push(PathBuf::from(&home).join(".config/vibe_cli/config.json"));
        paths.push(PathBuf::from(&home).join(".config/vibe_cli/config.toml"));
        paths.push(PathBuf::from(&home).join(".vibe_cli/config.yaml"));

        // Project-specific config files (higher priority)
        if let Some(project_root) = find_project_root() {
            paths.insert(0, PathBuf::from(&project_root).join(".vibe_cli.yaml"));
            paths.insert(0, PathBuf::from(&project_root).join(".vibe_cli.yml"));
            paths.insert(0, PathBuf::from(&project_root).join(".vibe_cli.json"));
            paths.insert(0, PathBuf::from(&project_root).join(".vibe_cli.toml"));
            paths.insert(0, PathBuf::from(&project_root).join("vibe_cli.yaml"));
        }

        paths
    }

    /// Load configuration with fallback to defaults
    pub fn load() -> Self {
        for path in Self::get_config_paths() {
            if path.exists() {
                match Self::load_from_file(&path) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to load config from {}: {}",
                            path.display(),
                            e
                        );
                        // Continue to next path
                    }
                }
            }
        }

        // Load from environment variables if no file found
        Self::load_from_env()
    }

    /// Load configuration from environment variables
    pub fn load_from_env() -> Self {
        let mut config = Self::default();

        // Load aliases from environment
        if let Ok(aliases_str) = env::var("VIBE_ALIASES") {
            if let Ok(aliases) = serde_json::from_str::<HashMap<String, String>>(&aliases_str) {
                config.aliases = aliases;
            }
        }

        // Load performance settings
        if let Ok(prewarm) = env::var("VIBE_PREWARM_MODELS") {
            config.performance.prewarm_models = prewarm.parse().unwrap_or(false);
        }

        if let Ok(parallel) = env::var("VIBE_PARALLEL_JOBS") {
            config.performance.parallel_jobs = parallel.parse().unwrap_or(num_cpus::get());
        }

        // Load theme settings
        if let Ok(theme_name) = env::var("VIBE_THEME") {
            config.theme.name = theme_name;
        }

        config
    }

    /// Get an alias expansion
    pub fn get_alias(&self, command: &str) -> Option<&String> {
        self.aliases.get(command)
    }

    /// Check if a command is allowed by permissions
    pub fn is_command_allowed(&self, command: &str) -> bool {
        use regex::Regex;

        // Check blocklist first
        for pattern in &self.permissions.blocked_commands {
            if let Ok(regex) = Regex::new(pattern) {
                if regex.is_match(command) {
                    return false;
                }
            }
        }

        // If allowlist is not empty, check against it
        if !self.permissions.allowed_commands.is_empty() {
            for pattern in &self.permissions.allowed_commands {
                if let Ok(regex) = Regex::new(pattern) {
                    if regex.is_match(command) {
                        return true;
                    }
                }
            }
            return false; // Not in allowlist
        }

        true // No restrictions
    }

    /// Get editor command for opening a file
    pub fn get_editor_command(
        &self,
        editor_name: Option<&str>,
        file: &str,
        line: Option<usize>,
    ) -> String {
        let editor = editor_name.unwrap_or(&self.editors.preferred_editor);

        if let Some(integration) = self.editors.integrations.get(editor) {
            if let Some(cmd_template) = integration.get("open_file") {
                let line_str = line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "1".to_string());
                return cmd_template
                    .replace("{file}", file)
                    .replace("{line}", &line_str);
            }
        }

        // Fallback to basic editor command
        match line {
            Some(l) => format!("{} +{} {}", editor, l, file),
            None => format!("{} {}", editor, file),
        }
    }
}

/// Plugin System
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Get the plugin name
    fn name(&self) -> &str;

    /// Get the plugin version
    fn version(&self) -> &str;

    /// Get the plugin description
    fn description(&self) -> &str;

    /// Initialize the plugin
    async fn initialize(
        &mut self,
        config: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Execute a command provided by this plugin
    async fn execute(
        &self,
        command: &str,
        args: Vec<String>,
    ) -> Result<String, Box<dyn std::error::Error>>;

    /// Check if this plugin can handle the given command
    fn can_handle(&self, command: &str) -> bool;

    /// Get help text for this plugin
    fn help(&self) -> String;
}

/// Plugin Manager
pub struct PluginManager {
    plugins: HashMap<String, Arc<dyn Plugin>>,
    config: PluginConfig,
}

impl PluginManager {
    pub fn new(config: PluginConfig) -> Self {
        Self {
            plugins: HashMap::new(),
            config,
        }
    }

    /// Load built-in plugins
    pub async fn load_builtins(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Add built-in plugins here
        // For now, we'll add a simple example plugin

        // Example: File operations plugin
        let file_plugin = Arc::new(FileOperationsPlugin::new());
        self.plugins
            .insert(file_plugin.name().to_string(), file_plugin);

        // Example: System info plugin
        let system_plugin = Arc::new(SystemInfoPlugin::new());
        self.plugins
            .insert(system_plugin.name().to_string(), system_plugin);

        Ok(())
    }

    /// Load external plugins from configured paths
    pub async fn load_external(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs;

        // Clone paths to avoid borrow checker issues
        let paths = self.config.paths.clone();

        for path in &paths {
            let expanded_path = shellexpand::tilde(path).to_string();
            let plugin_dir = PathBuf::from(&expanded_path);

            if !plugin_dir.exists() {
                eprintln!("Plugin directory does not exist: {}", expanded_path);
                continue;
            }

            if !plugin_dir.is_dir() {
                eprintln!("Plugin path is not a directory: {}", expanded_path);
                continue;
            }

            // Scan directory for plugin manifest files
            // We look for .plugin.toml files that describe external plugins
            match fs::read_dir(&plugin_dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let path = entry.path();

                        // Look for plugin manifest files
                        if let Some(file_name) = path.file_name() {
                            if let Some(name_str) = file_name.to_str() {
                                if name_str.ends_with(".plugin.toml") {
                                    match self.load_external_plugin_from_manifest(&path).await {
                                        Ok(plugin_name) => {
                                            println!("Loaded external plugin: {}", plugin_name);
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to load plugin from {}: {}", path.display(), e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read plugin directory {}: {}", expanded_path, e);
                }
            }
        }

        Ok(())
    }

    /// Load an external plugin from a manifest file
    async fn load_external_plugin_from_manifest(&mut self, manifest_path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
        use std::fs;

        // Read the manifest file
        let manifest_content = fs::read_to_string(manifest_path)?;

        // Parse the manifest as TOML
        let manifest: toml::Value = toml::from_str(&manifest_content)?;

        // Extract plugin metadata
        let plugin_table = manifest.as_table()
            .ok_or("Invalid manifest: root must be a table")?;

        let name = plugin_table.get("name")
            .and_then(|v| v.as_str())
            .ok_or("Plugin manifest missing 'name' field")?;

        let version = plugin_table.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0");

        let description = plugin_table.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");

        let plugin_type = plugin_table.get("type")
            .and_then(|v| v.as_str())
            .ok_or("Plugin manifest missing 'type' field")?;

        // For now, we support script-based plugins (shell, python, etc.)
        // Future: Add support for native libraries (.so, .dll, .dylib) and WASM
        match plugin_type {
            "script" => {
                let script_path = plugin_table.get("script")
                    .and_then(|v| v.as_str())
                    .ok_or("Script plugin missing 'script' field")?;

                let commands = plugin_table.get("commands")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(String::from)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                // Create a script-based plugin wrapper
                let plugin = ScriptPlugin::new(
                    name.to_string(),
                    version.to_string(),
                    description.to_string(),
                    script_path.to_string(),
                    commands,
                );

                self.plugins.insert(name.to_string(), Arc::new(plugin));
                Ok(name.to_string())
            }
            "native" => {
                // Future: Implement native library loading using libloading crate
                // This would require:
                // 1. Loading the shared library (.so, .dll, .dylib)
                // 2. Looking up the plugin entry point function
                // 3. Creating a wrapper that implements the Plugin trait
                Err("Native plugins not yet supported. Use script plugins instead.".into())
            }
            "wasm" => {
                // Future: Implement WASM plugin loading using wasmer or wasmtime
                Err("WASM plugins not yet supported. Use script plugins instead.".into())
            }
            _ => {
                Err(format!("Unknown plugin type: {}", plugin_type).into())
            }
        }
    }

    /// Initialize all loaded plugins
    pub async fn initialize_plugins(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for (name, plugin) in &self.plugins {
            let plugin_config = self.config.settings.get(name).cloned().unwrap_or_default();
            // Note: We need to modify the plugin trait to allow mutable access for initialization
            // For now, this is a simplified version
            println!("Initialized plugin: {}", name);
        }
        Ok(())
    }

    /// Execute a command using available plugins
    pub async fn execute_command(
        &self,
        command: &str,
        args: Vec<String>,
    ) -> Option<Result<String, Box<dyn std::error::Error>>> {
        for plugin in self.plugins.values() {
            if plugin.can_handle(command) {
                return Some(plugin.execute(command, args).await);
            }
        }
        None
    }

    /// Get help for all plugins
    pub fn get_help(&self) -> String {
        let mut help = String::from("Available Plugins:\n");
        for plugin in self.plugins.values() {
            help.push_str(&format!("  {}: {}\n", plugin.name(), plugin.description()));
            help.push_str(&format!("    {}\n", plugin.help()));
        }
        help
    }

    /// List all loaded plugins
    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }
}

/// Script-based plugin wrapper for external plugins
struct ScriptPlugin {
    name: String,
    version: String,
    description: String,
    script_path: String,
    commands: Vec<String>,
}

impl ScriptPlugin {
    fn new(
        name: String,
        version: String,
        description: String,
        script_path: String,
        commands: Vec<String>,
    ) -> Self {
        Self {
            name,
            version,
            description,
            script_path,
            commands,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for ScriptPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn initialize(
        &mut self,
        _config: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Verify script exists and is executable
        let script_path = PathBuf::from(&self.script_path);
        if !script_path.exists() {
            return Err(format!("Script not found: {}", self.script_path).into());
        }

        // On Unix systems, check if executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&script_path)?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return Err(format!("Script is not executable: {}", self.script_path).into());
            }
        }

        Ok(())
    }

    async fn execute(
        &self,
        command: &str,
        args: Vec<String>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use std::process::Command;

        // Execute the script with the command and arguments
        let mut cmd = Command::new(&self.script_path);
        cmd.arg(command);
        cmd.args(&args);

        let output = cmd.output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("Script execution failed: {}", stderr).into())
        }
    }

    fn can_handle(&self, command: &str) -> bool {
        self.commands.contains(&command.to_string())
    }

    fn help(&self) -> String {
        format!(
            "{} (v{}) - {}\nCommands: {}",
            self.name,
            self.version,
            self.description,
            self.commands.join(", ")
        )
    }
}

/// Built-in File Operations Plugin
struct FileOperationsPlugin;

impl FileOperationsPlugin {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Plugin for FileOperationsPlugin {
    fn name(&self) -> &str {
        "fileops"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "File operations and utilities"
    }

    async fn initialize(
        &mut self,
        _config: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn execute(
        &self,
        command: &str,
        args: Vec<String>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        match command {
            "file_count" => {
                if args.is_empty() {
                    return Err("Usage: file_count <directory>. Counts files in directory.".into());
                }
                let path = &args[0];
                let count = std::fs::read_dir(path)?
                    .filter_map(|entry| entry.ok())
                    .count();
                Ok(format!("Files in {}: {}", path, count))
            }
            "file_size" => {
                if args.is_empty() {
                    return Err("Usage: file_size <file>. Shows file size.".into());
                }
                let path = &args[0];
                let metadata = std::fs::metadata(path)?;
                let size = metadata.len();
                Ok(format!("Size of {}: {} bytes", path, size))
            }
            _ => Err(format!("Unknown file operation: {}", command).into()),
        }
    }

    fn can_handle(&self, command: &str) -> bool {
        command.starts_with("file_")
    }

    fn help(&self) -> String {
        "  file_count <dir>  - Count files in directory\n  file_size <file>   - Get file size"
            .to_string()
    }
}

/// Built-in System Info Plugin
struct SystemInfoPlugin;

impl SystemInfoPlugin {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Plugin for SystemInfoPlugin {
    fn name(&self) -> &str {
        "sysinfo"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "System information and monitoring"
    }

    async fn initialize(
        &mut self,
        _config: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn execute(
        &self,
        command: &str,
        args: Vec<String>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        match command {
            "sys_uptime" => {
                let output = std::process::Command::new("uptime").arg("-p").output()?;
                let uptime = String::from_utf8(output.stdout)?;
                Ok(format!("System uptime: {}", uptime.trim()))
            }
            "sys_load" => {
                let output = std::process::Command::new("uptime").output()?;
                let load = String::from_utf8(output.stdout)?;
                Ok(format!("System load: {}", load.trim()))
            }
            "sys_memory" => {
                let output = std::process::Command::new("free").arg("-h").output()?;
                let memory = String::from_utf8(output.stdout)?;
                Ok(format!("Memory usage:\n{}", memory))
            }
            _ => Err(format!("Unknown system command: {}", command).into()),
        }
    }

    fn can_handle(&self, command: &str) -> bool {
        command.starts_with("sys_")
    }

    fn help(&self) -> String {
        "  sys_uptime       - Show system uptime\n  sys_load          - Show system load average\n  sys_memory        - Show memory usage".to_string()
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
    pub power_user: PowerUserConfig,
    pub plugin_manager: Option<Arc<tokio::sync::RwLock<PluginManager>>>,
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
            power_user: PowerUserConfig::load(),
            plugin_manager: None,
        }
    }

    /// Initialize plugins asynchronously
    pub async fn initialize_plugins(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(plugin_manager) = &self.plugin_manager {
            let mut manager = plugin_manager.write().await;
            manager.load_builtins().await?;
            manager.load_external().await?;
            manager.initialize_plugins().await?;
        }
        Ok(())
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

    fn load_security_config_from_file(
        path: &str,
    ) -> Result<SecurityConfig, Box<dyn std::error::Error>> {
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
                    .parse()
                    .unwrap_or(5),
                max_tools_per_iteration: env::var("VIBE_MAX_TOOLS_PER_ITERATION")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse()
                    .unwrap_or(3),
                max_execution_time_seconds: env::var("VIBE_MAX_EXECUTION_TIME_SECONDS")
                    .unwrap_or_else(|_| "120".to_string())
                    .parse()
                    .unwrap_or(120),
                verification_timeout_seconds: env::var("VIBE_VERIFICATION_TIMEOUT_SECONDS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
                allow_iteration_on_failure: env::var("VIBE_ALLOW_ITERATION_ON_FAILURE")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                convergence_threshold: env::var("VIBE_CONVERGENCE_THRESHOLD")
                    .unwrap_or_else(|_| "0.8".to_string())
                    .parse()
                    .unwrap_or(0.8),
                time_bounds_per_iteration_seconds: env::var(
                    "VIBE_TIME_BOUNDS_PER_ITERATION_SECONDS",
                )
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
                memory_limit_mb: env::var("VIBE_MEMORY_LIMIT_MB")
                    .ok()
                    .and_then(|s| s.parse().ok()),
            },
            resource_limits: ResourceLimitsConfig {
                max_memory_mb: env::var("VIBE_MAX_MEMORY_MB")
                    .unwrap_or_else(|_| "1024".to_string())
                    .parse()
                    .unwrap_or(1024),
                max_cpu_percentage: env::var("VIBE_MAX_CPU_PERCENTAGE")
                    .unwrap_or_else(|_| "80".to_string())
                    .parse()
                    .unwrap_or(80),
                max_file_operations: env::var("VIBE_MAX_FILE_OPERATIONS")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .unwrap_or(1000),
                max_network_requests: env::var("VIBE_MAX_NETWORK_REQUESTS")
                    .unwrap_or_else(|_| "50".to_string())
                    .parse()
                    .unwrap_or(50),
                sandbox_enabled: env::var("VIBE_SANDBOX_ENABLED")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                cgroups_enabled: env::var("VIBE_CGROUPS_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
            },
            network_security: NetworkSecurityConfig {
                allowed_domains: env::var("VIBE_ALLOWED_DOMAINS")
                    .unwrap_or_else(|_| {
                        "localhost,*.githubusercontent.com,*.wikipedia.org".to_string()
                    })
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
                    .parse()
                    .unwrap_or(1024),
                timeout_seconds: env::var("VIBE_NETWORK_TIMEOUT_SECONDS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
                enable_ssl_verification: env::var("VIBE_ENABLE_SSL_VERIFICATION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
            },
            content_sanitization: ContentSanitizationConfig {
                prompt_injection_detection: env::var("VIBE_PROMPT_INJECTION_DETECTION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                sql_injection_detection: env::var("VIBE_SQL_INJECTION_DETECTION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                secret_detection: env::var("VIBE_SECRET_DETECTION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                allowed_content_types: env::var("VIBE_ALLOWED_CONTENT_TYPES")
                    .unwrap_or_else(|_| "text/plain,text/markdown,application/json".to_string())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                max_content_length_kb: env::var("VIBE_MAX_CONTENT_LENGTH_KB")
                    .unwrap_or_else(|_| "512".to_string())
                    .parse()
                    .unwrap_or(512),
            },
            audit_trail: AuditTrailConfig {
                enabled: env::var("VIBE_AUDIT_TRAIL_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                log_level: env::var("VIBE_LOG_LEVEL").unwrap_or_else(|_| "INFO".to_string()),
                max_log_files: env::var("VIBE_MAX_LOG_FILES")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
                max_log_size_mb: env::var("VIBE_MAX_LOG_SIZE_MB")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .unwrap_or(100),
                log_directory: env::var("VIBE_LOG_DIRECTORY")
                    .unwrap_or_else(|_| "./logs".to_string()),
                structured_logging: env::var("VIBE_STRUCTURED_LOGGING")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
            },
            feature_flags: HashMap::new(), // Can be extended later
        }
    }
}
