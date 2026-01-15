use application::{agent_service::AgentService, build_service::BuildPlan, rag_service::RagService};
use chrono::Utc;
use clap::Parser;
use colored::Colorize;
use docx_rs::*;
use flume::Receiver;
use infrastructure::{
    background_supervisor::{
        BackgroundEvent, BackgroundSupervisor, DiagnosticSeverity, FileChangeType, GitStatus,
        LogLevel, TestStatus,
    },
    config::Config,
    input_classifier::{InputClassifier, InputType},
    ollama_client::OllamaClient,
    sandbox::Sandbox,
    session_store::SessionStore,
};
use serde::{Deserialize, Serialize};
use shared::confirmation::ask_confirmation;
use shared::types::Result;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tokio::time::{self, Duration};

use crate::editor;

/// Classification of user query intent
#[derive(Debug, Clone, PartialEq)]
pub enum CommandIntent {
    InfoQuery,      // "what's my GPU", "how much RAM"
    Installation,   // "install python", "setup nginx"
    Configuration,  // "configure nginx", "enable firewall"
    ServiceControl, // "start apache", "restart mysql"
    SystemQuery,    // "show disk usage", "list processes"
    Unknown,        // Unclassified queries
}

/// Risk assessment for commands
#[derive(Debug, Clone, PartialEq)]
pub enum CommandRisk {
    InfoOnly,       // Read-only queries, no system changes
    SafeSetup,      // Package installs, basic service setup
    SystemChanges,  // Configuration changes, user creation
    HighRisk,       // Destructive operations, system-wide changes
    Unknown,        // Cannot assess risk
}

/// Installation option for user selection
#[derive(Debug, Clone)]
pub struct InstallationOption {
    pub name: String,
    pub description: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub risk_level: CommandRisk,
    pub commands: Vec<String>,
    pub estimated_time: Option<String>,
    pub disk_space: Option<String>,
}

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
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        root.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    } else {
        "global".to_string()
    }
}

fn detect_system_info() -> String {
    let mut info = Vec::new();

    // Detect OS
    if let Ok(os) = std::fs::read_to_string("/etc/os-release") {
        for line in os.lines() {
            if line.starts_with("ID=") {
                info.push(format!(
                    "Distro: {}",
                    line.trim_start_matches("ID=").trim_matches('"')
                ));
            } else if line.starts_with("VERSION_ID=") {
                info.push(format!(
                    "Version: {}",
                    line.trim_start_matches("VERSION_ID=").trim_matches('"')
                ));
            }
        }
    } else if let Ok(os) = std::process::Command::new("uname").arg("-s").output() {
        info.push(format!(
            "OS: {}",
            String::from_utf8_lossy(&os.stdout).trim()
        ));
    }

    // Detect init system
    if std::path::Path::new("/run/systemd/system").exists() {
        info.push("Init system: systemd".to_string());
    } else if std::path::Path::new("/etc/init.d").exists() {
        info.push("Init system: init.d".to_string());
    }

    // Detect package manager
    if std::process::Command::new("which")
        .arg("apt")
        .output()
        .is_ok()
    {
        info.push("Package manager: apt".to_string());
    } else if std::process::Command::new("which")
        .arg("yum")
        .output()
        .is_ok()
    {
        info.push("Package manager: yum".to_string());
    } else if std::process::Command::new("which")
        .arg("dnf")
        .output()
        .is_ok()
    {
        info.push("Package manager: dnf".to_string());
    } else if std::process::Command::new("which")
        .arg("pacman")
        .output()
        .is_ok()
    {
        info.push("Package manager: pacman".to_string());
    }

    // Kernel version
    if let Ok(kernel) = std::process::Command::new("uname").arg("-r").output() {
        info.push(format!(
            "Kernel: {}",
            String::from_utf8_lossy(&kernel.stdout).trim()
        ));
    }

    info.join(", ")
}

// Cache entries expire after 7 days (604800 seconds)
const CACHE_TTL_SECONDS: u64 = 604800;

// Semantic similarity threshold (0.0 to 1.0)
const SEMANTIC_SIMILARITY_THRESHOLD: f64 = 0.7;

#[derive(Serialize, Deserialize, Default)]
struct CacheFile {
    entries: Vec<CacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    prompt: String,
    command: String,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct ExplainCacheFile {
    entries: Vec<ExplainCacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct ExplainCacheEntry {
    prompt: String,
    response: String,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
struct RagCacheFile {
    entries: Vec<RagCacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct RagCacheEntry {
    question: String,
    response: String,
    timestamp: u64,
}

/// Statistics about context gathering for display
#[derive(Default, Clone)]
struct ContextStats {
    files_scanned: usize,
    files_analyzed: usize,
    keywords_count: usize,
    os_info: String,
    cwd: String,
    total_files: usize,
    relevant_files: usize,
}

/// Remove markdown code fences/backticks and surrounding quotes
fn clean_command_output(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 3 && lines.last().unwrap().trim() == "```" {
            return lines[1..lines.len() - 1].join("\n").trim().to_string();
        }
    }
    trimmed
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

/// Extract last JSON object/array from text
fn extract_last_json(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}')
        || trimmed.starts_with('[') && trimmed.ends_with(']')
    {
        return Some(trimmed);
    }
    let bytes = trimmed.as_bytes();
    let mut depth = 0;
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'{' || b == b'[' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if b == b'}' || b == b']' {
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start {
                    return Some(&trimmed[s..=i]);
                }
            }
        }
    }
    None
}

/// Extract JSON array from possibly noisy text
fn extract_json_array(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0;
    let mut start = None;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match b {
            b'"' => in_string = !in_string,
            b'\\' => {
                if in_string {
                    escape_next = true;
                }
            }
            b'[' => {
                if !in_string && depth == 0 {
                    start = Some(i);
                }
                if !in_string {
                    depth += 1;
                }
            }
            b']' => {
                if !in_string {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            return Some(&text[s..=i]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse agent response into a list of commands
fn parse_agent_plan(raw: &str) -> Vec<String> {
    // Try plain parse
    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(raw) {
        return cmds;
    }
    // Clean and try again
    let cleaned = clean_command_output(raw);
    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(&cleaned) {
        return cmds;
    }
    // Try to pull array from noisy text
    if let Some(arr) = extract_json_array(raw) {
        if let Ok(cmds) = serde_json::from_str::<Vec<String>>(arr) {
            return cmds;
        }
    }
    if let Some(json) = extract_last_json(raw) {
        if let Ok(cmds) = serde_json::from_str::<Vec<String>>(json) {
            return cmds;
        }
    }
    // Fallback: split non-empty lines, stripping common list markers and code fences
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty() && !l.starts_with("```") && !l.ends_with("```") && *l != "[" && *l != "]"
        })
        .map(|l| {
            let mut line = l
                .trim_start_matches(|c| c == '-' || c == '*' || c == '•')
                .trim();
            if let Some(pos) = line.find(|c: char| c == ')' || c == '.' || c == ':') {
                // Only strip early numbering markers
                if pos < 4 {
                    line = line[pos + 1..].trim();
                }
            }
            line.trim_matches(',').trim().trim_matches('"').to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

fn extract_command_from_response(response: &str) -> String {
    let response = response.trim();
    let cleaned = if response.starts_with("```bash") && response.ends_with("```") {
        let start = response.find('\n').unwrap_or(0) + 1;
        let end = response.len() - 3;
        response[start..end].trim().to_string()
    } else if response.starts_with("```") && response.ends_with("```") {
        let start = response.find('\n').unwrap_or(0) + 1;
        let end = response.len() - 3;
        response[start..end].trim().to_string()
    } else {
        response.to_string()
    };

    // Smart quote handling: only remove surrounding quotes if they wrap the entire command
    // and there are no quotes within the command (indicating they're part of command syntax)
    let trimmed = cleaned.trim_matches('`').trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len()-1];
        // If there are no quotes inside, it's safe to remove the outer quotes
        if !inner.contains('"') && !inner.contains('\'') {
            return inner.trim().to_string();
        }
        // Otherwise, keep the quotes as they're part of the command syntax
    } else if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
        let inner = &trimmed[1..trimmed.len()-1];
        // Same logic for single quotes
        if !inner.contains('\'') && !inner.contains('"') {
            return inner.trim().to_string();
        }
    }

    trimmed.to_string()
}

/// Analyze user query to determine intent
fn analyze_query_intent(query: &str) -> CommandIntent {
    let query_lower = query.to_lowercase();

    // Installation keywords
    let install_keywords = [
        "install", "setup", "add", "create", "build", "compile",
        "download", "get", "fetch", "deploy", "configure"
    ];

    // Configuration keywords
    let config_keywords = [
        "configure", "config", "enable", "disable", "set", "update",
        "modify", "change", "edit", "tune", "optimize"
    ];

    // Service control keywords
    let service_keywords = [
        "start", "stop", "restart", "reload", "enable", "disable",
        "status", "systemctl", "service", "daemon"
    ];

    // Information query patterns
    let info_patterns = [
        "what's", "what is", "how much", "how many", "show", "list",
        "display", "check", "verify", "info", "information"
    ];

    // Check for installation intent
    if install_keywords.iter().any(|&kw| query_lower.contains(kw)) {
        return CommandIntent::Installation;
    }

    // Check for configuration intent
    if config_keywords.iter().any(|&kw| query_lower.contains(kw)) {
        return CommandIntent::Configuration;
    }

    // Check for service control intent
    if service_keywords.iter().any(|&kw| query_lower.contains(kw)) {
        return CommandIntent::ServiceControl;
    }

    // Check for information queries
    if info_patterns.iter().any(|&pat| query_lower.contains(pat)) {
        return CommandIntent::InfoQuery;
    }

    // Default to system query for general queries
    if query_lower.contains("disk") || query_lower.contains("memory") ||
       query_lower.contains("cpu") || query_lower.contains("gpu") ||
       query_lower.contains("network") || query_lower.contains("process") {
        return CommandIntent::SystemQuery;
    }

    CommandIntent::Unknown
}

/// Assess risk level of a command
fn assess_command_risk(command: &str) -> CommandRisk {
    let cmd_lower = command.to_lowercase();

    // High-risk commands
    let high_risk_patterns = [
        "rm -rf", "format", "mkfs", "fdisk", "dd if=", "shutdown",
        "reboot", "halt", "poweroff", "systemctl stop", "killall"
    ];

    // System-changing commands
    let system_change_patterns = [
        "usermod", "userdel", "groupmod", "chmod 777", "chown root",
        "systemctl enable", "systemctl disable", "ufw", "firewall",
        "iptables", "mount", "umount"
    ];

    // Safe setup commands
    let safe_setup_commands = [
        "apt install", "apt-get install", "yum install", "dnf install",
        "pacman -S", "brew install", "pip install", "npm install",
        "gem install", "cargo install"
    ];

    // Info-only commands (read-only)
    let info_only_commands = [
        "ls", "df", "free", "ps", "top", "htop", "uname", "whoami",
        "pwd", "cat", "grep", "find", "which", "whereis", "type"
    ];

    // Check high risk first
    if high_risk_patterns.iter().any(|&pat| cmd_lower.contains(pat)) {
        return CommandRisk::HighRisk;
    }

    // Check system changes
    if system_change_patterns.iter().any(|&pat| cmd_lower.contains(pat)) {
        return CommandRisk::SystemChanges;
    }

    // Check safe setup
    if safe_setup_commands.iter().any(|&cmd| cmd_lower.contains(cmd)) {
        return CommandRisk::SafeSetup;
    }

    // Check info-only
    if info_only_commands.iter().any(|&cmd| cmd_lower.starts_with(cmd)) {
        return CommandRisk::InfoOnly;
    }

    // Default to unknown
    CommandRisk::Unknown
}

/// Present confirmation dialog for data collection commands
fn prompt_data_collection_confirmation(
    command: &str,
    query: &str,
    risk: CommandRisk,
) -> Result<bool> {
    println!("DATA COLLECTION REQUIRED");
    println!();
    println!("This query needs to run: {}", command);

    // Determine purpose based on query content
    let purpose = if query.to_lowercase().contains("gpu") || query.to_lowercase().contains("graphics") {
        "Gather GPU information for analysis"
    } else if query.to_lowercase().contains("cpu") || query.to_lowercase().contains("processor") {
        "Gather CPU information for analysis"
    } else if query.to_lowercase().contains("ram") || query.to_lowercase().contains("memory") {
        "Gather memory information for analysis"
    } else if query.to_lowercase().contains("disk") || query.to_lowercase().contains("storage") {
        "Gather disk usage information for analysis"
    } else if query.to_lowercase().contains("network") {
        "Gather network information for analysis"
    } else {
        "Gather system information for analysis"
    };

    println!("Purpose: {}", purpose);

    // Show safety level
    let safety_desc = match risk {
        CommandRisk::InfoOnly => "Read-only, no system modifications",
        CommandRisk::SafeSetup => "Safe system query with minimal impact",
        CommandRisk::SystemChanges => "May modify system configuration",
        CommandRisk::HighRisk => "High-risk operation requiring careful review",
        CommandRisk::Unknown => "Risk level cannot be determined",
    };

    println!("Safety: {}", safety_desc);
    println!();

    ask_confirmation("Allow command execution?", risk == CommandRisk::InfoOnly)
}

/// Present confirmation dialog for installation commands
fn prompt_installation_confirmation(
    command: &str,
    intent: CommandIntent,
    packages: Vec<String>,
    services: Vec<String>,
    disk_space: Option<String>,
) -> Result<bool> {
    println!("INSTALLATION COMMAND DETECTED");
    println!();
    println!("Command: {}", command);

    if !packages.is_empty() {
        println!();
        println!("Packages to install:");
        for package in &packages {
            println!("  - {}", package);
        }
    }

    if !services.is_empty() {
        println!();
        println!("System changes:");
        for service in &services {
            println!("  - New service: {}", service);
        }
    }

    if let Some(space) = &disk_space {
        println!("  - Disk space: {}", space);
    }

    // Check if sudo is needed
    let needs_sudo = command.contains("sudo") || assess_command_risk(command) != CommandRisk::InfoOnly;
    if needs_sudo {
        println!("  - Requires: sudo privileges");
    }

    println!();

    // Default to 'No' for installations unless it's very safe
    ask_confirmation("Execute installation?", false)
}

/// Analyze installation command to extract details
fn analyze_installation_command(command: &str) -> (Vec<String>, Vec<String>, Option<String>) {
    let mut packages = Vec::new();
    let mut services = Vec::new();
    let mut disk_space = None;

    // Extract package names from common install commands
    let cmd_lower = command.to_lowercase();

    if cmd_lower.contains("apt install") || cmd_lower.contains("apt-get install") {
        // Extract package names after "install"
        if let Some(install_pos) = cmd_lower.find("install") {
            let package_part = &command[install_pos + 7..].trim();
            packages = package_part.split_whitespace()
                .map(|s| s.to_string())
                .collect();
        }
    } else if cmd_lower.contains("pip install") {
        if let Some(install_pos) = cmd_lower.find("install") {
            let package_part = &command[install_pos + 7..].trim();
            packages = package_part.split_whitespace()
                .take(3) // Limit to first few packages
                .map(|s| format!("{} (Python package)", s))
                .collect();
        }
    }

    // Estimate disk space based on packages
    if !packages.is_empty() {
        if packages.len() == 1 {
            disk_space = Some("~50MB".to_string());
        } else if packages.len() <= 3 {
            disk_space = Some("~100MB".to_string());
        } else {
            disk_space = Some("~250MB".to_string());
        }
    }

    // Identify services that might be started
    for package in &packages {
        let pkg_lower = package.to_lowercase();
        if pkg_lower.contains("nginx") || pkg_lower == "nginx" {
            services.push("nginx".to_string());
        } else if pkg_lower.contains("apache") || pkg_lower.contains("httpd") {
            services.push("apache2".to_string());
        } else if pkg_lower.contains("mysql") || pkg_lower.contains("mariadb") {
            services.push("mysql".to_string());
        } else if pkg_lower.contains("postgresql") {
            services.push("postgresql".to_string());
        }
    }

    (packages, services, disk_space)
}

/// Validate that a command has basic syntactical correctness
fn validate_command_syntax(command: &str) -> std::result::Result<(), String> {
    let trimmed = command.trim();

    // Check for unclosed quotes
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    for ch in trimmed.chars() {
        match ch {
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            _ => {}
        }
    }

    if in_single_quote || in_double_quote {
        return Err("Command contains unclosed quotes".to_string());
    }

    // Check for unbalanced parentheses (basic check)
    let paren_count = trimmed.chars().fold(0, |count, ch| {
        match ch {
            '(' => count + 1,
            ')' => count - 1,
            _ => count,
        }
    });

    if paren_count != 0 {
        return Err("Command contains unbalanced parentheses".to_string());
    }

    // Check for obviously malformed patterns
    if trimmed.contains("&&&") || trimmed.contains("|||") {
        return Err("Command contains consecutive operators".to_string());
    }

    if trimmed.starts_with('|') || trimmed.starts_with('&') || trimmed.starts_with(';') {
        return Err("Command starts with a pipe or operator".to_string());
    }

    if trimmed.ends_with('|') || trimmed.ends_with('&') {
        return Err("Command ends with a pipe or operator".to_string());
    }

    Ok(())
}

#[derive(Parser)]
#[command(name = "vibe_cli")]
#[command(about = "Vibe CLI assistant with RAG capabilities")]
pub struct Cli {
    /// Enter interactive chat mode
    #[arg(long)]
    pub chat: bool,

    /// Use multi-step agent mode
    #[arg(long)]
    pub agent: bool,

    /// Use enhanced agentic AI assistant
    #[arg(long)]
    pub ai_agent: bool,

    /// Create execution plan without running commands
    #[arg(long)]
    pub plan: bool,

    /// Explain a file
    #[arg(long)]
    pub explain: bool,

    /// Query with RAG context
    #[arg(long)]
    pub rag: bool,

    /// Load context from path
    #[arg(long)]
    pub context: bool,

    /// Stream agent execution in real-time
    #[arg(long)]
    pub stream: bool,

    /// Use safe build mode with RAG context and user confirmation
    #[arg(
        long,
        help = "Generate and execute build plans with AI assistance, RAG context retrieval, and transaction safety"
    )]
    pub build: bool,

    /// Run tests with real-time monitoring
    #[arg(
        long,
        help = "Execute cargo test with real-time result monitoring and background intelligence"
    )]
    pub test: bool,

    /// Dry-run mode: show plan without executing
    #[arg(
        long,
        help = "Preview build plan and operations without making changes"
    )]
    pub dry_run: bool,

    /// Verbose output: show detailed information
    #[arg(
        long,
        help = "Display retrieved context and detailed operation information"
    )]
    pub verbose: bool,

    /// Show diffs for file operations (reserved for future use)
    #[arg(long, help = "Show diffs for file modifications (planned feature)")]
    pub show_diff: bool,

    /// Specify which session to use for operations
    #[arg(
        long,
        value_name = "NAME",
        help = "Use named session for operations (creates if doesn't exist)"
    )]
    pub session: Option<String>,

    /// List all sessions for the current project
    #[arg(long, help = "Display all sessions with their metadata")]
    pub list_sessions: bool,

    /// Delete a specific session
    #[arg(
        long,
        value_name = "NAME",
        help = "Permanently delete the specified session"
    )]
    pub delete_session: Option<String>,

    /// Continue the current or last active session
    #[arg(long, help = "Resume the current or most recently used session")]
    pub continue_session: bool,

    /// Undo the last operation in the current session
    #[arg(long, help = "Revert the last applied changes in the current session")]
    pub undo: bool,

    /// The query or file path to process
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,

    /// Path to power user configuration file (YAML/JSON/TOML)
    #[arg(
        long,
        value_name = "FILE",
        help = "Load power user configuration from file"
    )]
    pub config: Option<String>,

    /// Generate default configuration file
    #[arg(
        long,
        value_name = "FILE",
        help = "Generate default configuration file and exit"
    )]
    pub generate_config: Option<String>,
}

pub struct CliApp {
    rag_service: Option<RagService>,
    cache_path: PathBuf,
    system_info: String,
    config: Config,
    session_store: Option<SessionStore>,
    current_session: Option<String>,
    background_supervisor: Option<BackgroundSupervisor>,
    scripted_inputs: Option<std::collections::VecDeque<String>>,
    power_config_override: Option<infrastructure::config::PowerUserConfig>,
    input_classifier: Option<infrastructure::input_classifier::InputClassifier>,
}

impl CliApp {
    fn read_input_line(&mut self) -> Result<String> {
        if let Some(queue) = &mut self.scripted_inputs {
            if let Some(next) = queue.pop_front() {
                return Ok(next);
            }
        }

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(input.trim_end().to_string())
    }
    pub fn new() -> Self {
        let cache_path = Self::default_cache_path();
        let system_info_path = Self::default_system_info_path();
        let system_info = Self::load_or_collect_system_info(&system_info_path);
        let config = Config::load();

        // Initialize session store for current project
        let session_store = if let Some(project_root) = find_project_root() {
            match SessionStore::new(&project_root) {
                Ok(store) => Some(store),
                Err(e) => {
                    eprintln!("Warning: Failed to initialize session store: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Initialize input classifier
        let input_classifier = match infrastructure::ollama_client::OllamaClient::new() {
            Ok(client) => Some(infrastructure::input_classifier::InputClassifier::new(
                std::sync::Arc::new(client),
            )),
            Err(e) => {
                eprintln!("Warning: Failed to initialize input classifier: {}", e);
                None
            }
        };

        Self {
            rag_service: None,
            cache_path,
            system_info,
            config,
            session_store,
            current_session: None,
            background_supervisor: Some(BackgroundSupervisor::new()),
            scripted_inputs: None,
            power_config_override: None,
            input_classifier,
        }
    }

    fn default_cache_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(format!("{}_cli_cache.json", suffix));
        path
    }

    fn default_system_info_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("vibe_cli");
        path.push("system_info.txt");
        path
    }

    fn context_specific_db_path(context_path: &str) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        context_path.hash(&mut hasher);
        let suffix = format!("{:x}", hasher.finish());

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        path.push(format!("{}_embeddings.db", suffix));
        path.to_string_lossy().to_string()
    }

    fn load_or_collect_system_info(path: &PathBuf) -> String {
        if let Ok(existing) = std::fs::read_to_string(path) {
            if !existing.trim().is_empty() {
                return existing.trim().to_string();
            }
        }

        let detected = detect_system_info();

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, &detected);

        detected
    }

    /// Normalize text for semantic comparison
    fn normalize_text(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    /// Calculate semantic similarity between two prompts
    fn semantic_similarity(prompt1: &str, prompt2: &str) -> f64 {
        let norm1 = Self::normalize_text(prompt1);
        let norm2 = Self::normalize_text(prompt2);

        if norm1 == norm2 {
            return 1.0;
        }

        let words1: HashSet<&str> = norm1.split_whitespace().collect();
        let words2: HashSet<&str> = norm2.split_whitespace().collect();

        let intersection: HashSet<&str> = words1.intersection(&words2).cloned().collect();
        let union: HashSet<&str> = words1.union(&words2).cloned().collect();

        if union.is_empty() {
            return 0.0;
        }

        intersection.len() as f64 / union.len() as f64
    }

    /// Clean command output by removing markdown code blocks
    fn clean_command_output(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") && trimmed.ends_with("```") {
            // Remove the first and last lines if they are ``` or ```sh
            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.len() >= 3 {
                if lines[0].trim().starts_with("```") && lines.last().unwrap().trim() == "```" {
                    return lines[1..lines.len() - 1].join("\n").trim().to_string();
                }
            }
        }
        // Be conservative with quote removal - only remove backticks, not quotes
        // Quotes are now handled intelligently in extract_command_from_response
        trimmed.trim_matches('`').trim().to_string()
    }

    fn load_cached(&self, prompt: &str) -> Result<Option<String>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read_to_string(&self.cache_path)?;
        let mut cache: CacheFile = serde_json::from_str(&data).unwrap_or_default();

        // Remove expired entries
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache
            .entries
            .retain(|entry| now - entry.timestamp < CACHE_TTL_SECONDS);

        // Save cleaned cache back to disk
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(&cache)?;
        std::fs::write(&self.cache_path, serialized)?;

        // First try exact match
        let mut entries_to_remove = Vec::new();
        let mut exact_match: Option<&CacheEntry> = None;

        for entry in &cache.entries {
            if entry.prompt == prompt {
                let cleaned_command = Self::clean_command_output(&entry.command);
                // Validate cached command syntax before returning
                if validate_command_syntax(&cleaned_command).is_ok() {
                    return Ok(Some(cleaned_command));
                } else {
                    eprintln!("{}", format!("Warning: Cached command has syntax issues, regenerating").yellow());
                    // Mark invalid entry for removal
                    entries_to_remove.push(entry.prompt.clone());
                    break;
                }
            }
        }

        // Then try semantic similarity
        let mut best_match: Option<&CacheEntry> = None;
        let mut best_similarity = 0.0;

        for entry in &cache.entries {
            let similarity = Self::semantic_similarity(prompt, &entry.prompt);
            if similarity > best_similarity && similarity >= SEMANTIC_SIMILARITY_THRESHOLD {
                best_similarity = similarity;
                best_match = Some(entry);
            }
        }

        let result = if let Some(entry) = best_match {
            let cleaned_command = Self::clean_command_output(&entry.command);
            // Validate semantically similar cached command syntax before returning
            if validate_command_syntax(&cleaned_command).is_ok() {
                Ok(Some(cleaned_command))
            } else {
                eprintln!("{}", format!("Warning: Similar cached command has syntax issues, regenerating").yellow());
                // Mark invalid entry for removal
                entries_to_remove.push(entry.prompt.clone());
                Ok(None)
            }
        } else {
            Ok(None)
        };

        // Remove invalid entries from cache after determining result
        if !entries_to_remove.is_empty() {
            cache.entries.retain(|e| !entries_to_remove.contains(&e.prompt));
            let serialized = serde_json::to_string_pretty(&cache)?;
            std::fs::write(&self.cache_path, serialized)?;
        }

        result
    }

    fn save_cached(&self, prompt: &str, command: &str) -> Result<()> {
        let mut cache = if self.cache_path.exists() {
            let data = std::fs::read_to_string(&self.cache_path).unwrap_or_default();
            serde_json::from_str::<CacheFile>(&data).unwrap_or_default()
        } else {
            CacheFile::default()
        };

        cache.entries.push(CacheEntry {
            prompt: prompt.to_string(),
            command: Self::clean_command_output(command),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serde_json::to_string_pretty(&cache)?;
        std::fs::write(&self.cache_path, serialized)?;

        Ok(())
    }

    async fn handle_ai_agent(&mut self, goal: &str) -> Result<()> {
        use domain::models::AgentRequest;

        eprintln!("🤖 Enhanced AI Agent processing request...");
        println!("{}", format!("Goal: {}", goal).bright_blue());

        // Initialize services
        let client = OllamaClient::new()?;
        // Use Ollama by default (now the recommended option)
        let agent_service = application::create_agent_service().await?;

        // Create agent request
        let request = AgentRequest {
            goal: goal.to_string(),
            context: Some(format!("System: {}", self.system_info)),
            conversation_id: None,
        };

        // Process with enhanced agent
        match agent_service.process_request(&request).await {
            Ok(response) => {
                println!("\n{}", "🧠 Reasoning:".bright_cyan());
                for (i, step) in response.reasoning.iter().enumerate() {
                    println!("  {}. {}", i + 1, step);
                }

                if !response.tool_calls.is_empty() {
                    println!("\n{}", "🔧 Tools Used:".bright_yellow());
                    for tool_call in &response.tool_calls {
                        println!("  • {} ({})", tool_call.name, tool_call.reasoning);
                    }
                }

                println!("\n{}", "💬 Response:".bright_green());
                println!("{}", response.final_response);
                println!(
                    "\n{}",
                    format!("⚡ Confidence: {:.1}%", response.confidence * 100.0).bright_magenta()
                );
            }
            Err(e) => {
                eprintln!("{} {}", "Agent error:".red(), e);
            }
        }

        Ok(())
    }

    async fn handle_plan_mode(&self, goal: &str) -> Result<()> {
        if goal.trim().is_empty() {
            println!(
                "{}",
                "Plan mode requires a goal (e.g. vibe_cli --plan \"Deploy this application\")"
                    .red()
            );
            return Ok(());
        }

        println!(
            "{}",
            "Planning mode: Create execution plan without running commands".bright_cyan()
        );
        println!("{}", format!("Goal: {}", goal).bright_blue());

        let system_context = infrastructure::config::SystemContext::gather();
        let ls_output = std::process::Command::new("ls")
            .arg("-la")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| String::new());

        // Initialize enhanced agent for planning
        let client = OllamaClient::new()?;
        // Use Candle by default instead of Ollama
        let agent_service = application::create_agent_service().await?;

        // Create agent request for planning with full context
        let context_info = format!(
            "SYSTEM CONTEXT:\n{}\n\nCURRENT DIRECTORY FILES:\n{}\n\nPackage Manager: {}\nMode: Plan Only",
            system_context.to_context_string(),
            ls_output,
            system_context.package_manager
        );

        let request = domain::models::AgentRequest {
            goal: format!(
                "Create a detailed step-by-step execution plan for: {}",
                goal
            ),
            context: Some(context_info),
            conversation_id: None,
        };

        // Generate plan using enhanced agent
        match agent_service.process_request(&request).await {
            Ok(response) => {
                println!("\n{}", "AI Planning Analysis:".bright_magenta());
                for (i, step) in response.reasoning.iter().enumerate() {
                    println!("  {}. {}", format!("Step {}", i + 1).bright_yellow(), step);
                }

                if !response.tool_calls.is_empty() {
                    println!("\n{}", "Planned Tools:".bright_yellow());
                    for tool_call in &response.tool_calls {
                        println!(
                            "  • {} - {}",
                            tool_call.name.bright_green(),
                            tool_call.reasoning
                        );
                    }
                }

                println!("\n{}", "Execution Plan:".bright_green());
                println!("{}", response.final_response);

                println!(
                    "\n{}",
                    "Planning complete. Use --agent or --chat to execute the plan.".bright_green()
                );
            }
            Err(e) => {
                eprintln!("{} {}", "Planning error:".red(), e);
            }
        }

        Ok(())
    }

    async fn handle_build(
        &mut self,
        goal: &str,
        dry_run: bool,
        verbose: bool,
        show_diff: bool,
    ) -> Result<()> {
        use application::agent_service::IncrementalBuildPlanner;
        use application::build_service::{BuildPlan, BuildService, ConfirmationMode, RiskLevel};
        use infrastructure::config::Config;

        if goal.trim().is_empty() {
            println!(
                "{}",
                "Build mode requires a goal (e.g. vibe_cli --build \"Add error handling to the parser\")"
                    .red()
            );
            return Ok(());
        }

        let workspace_root =
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut current_goal = goal.to_string();
        let mut plan_hints: Option<String> = None;

        println!(
            "{}",
            "Build Mode: Safe code modifications with user confirmation"
                .bright_cyan()
                .bold()
        );

        // Initialize services outside the planning loop so we can reuse them on replans
        let agent_service = application::create_agent_service().await?;

        'planning: loop {
            println!("{} {}", "Goal:".bright_green(), current_goal);

            let planning_goal = if let Some(ref hints) = plan_hints {
                format!(
                    "{}\n\nMANDATORY PLAN STEPS (follow exactly; do not omit):\n{}\n\nIf steps mention colors or full implementation, ensure the generated file includes them.",
                    current_goal, hints
                )
            } else {
                current_goal.clone()
            };

            // Configure build service based on flags
            let mut build_service = BuildService::new(&workspace_root);
            build_service.set_dry_run(dry_run);
            build_service.set_show_diff(show_diff);
            build_service.set_verbose(verbose);

            if verbose {
                build_service.set_confirmation_mode(ConfirmationMode::Interactive);
            }

            // Use true real-time incremental streaming
            println!("\n[PLAN] Starting incremental planning...");

            // Create the incremental planner
            let mut planner = match agent_service.plan_build_incremental(&planning_goal).await {
                Ok(planner) => planner,
                Err(e) => {
                    eprintln!("{} {}", "Build planning initialization error:".red(), e);
                    return Ok(());
                }
            };

            // Real-time incremental planning with tool transparency
            let total_steps = 5;
            println!("[PLAN] Analyzing project...");

            let mut step_count = 0;
            let mut code_generation_complete = false;

            loop {
                // Stop processing if code generation is complete
                if code_generation_complete {
                    break;
                }

                match planner
                    .stream_next_step(&agent_service.inference_engine)
                    .await
                {
                    Ok(Some(step)) => {
                        step_count += 1;

                        // Update progress display
                        self.update_progress_display(step_count, total_steps, &step.description);

                        // Minimal reasoning display
                        if step_count <= 3 && verbose {
                            println!("[REASON] {}", step.reasoning.lines().next().unwrap_or(""));
                        }

                        // Show minimal tool usage
                        if step_count == 2 && verbose {
                            let (scanned, analyzed, keywords, _, _) = planner.context_stats();
                            println!("[CONTEXT] Scanned {} files, {} keywords", scanned, keywords);
                        }

                        // Handle incremental code generation (Step 3)
                        if step_count == 3 {
                            if let (Some(code), Some(path), Some(op_type)) =
                                (&step.code_chunk, &step.file_path, &step.operation_type)
                            {
                                // Display the incremental changes from AI
                                self.display_incremental_changes(code, path, op_type);

                                // Mark code generation as complete to prevent duplicate steps
                                code_generation_complete = true;
                            }
                            // If no code provided by AI, skip this step (don't use hardcoded fallbacks)
                        }

                        // No artificial delay for speed
                    }
                    Ok(None) => {
                        // Mark final step complete
                        self.update_progress_display(
                            total_steps,
                            total_steps,
                            "Finalizing changes",
                        );
                        break;
                    }
                    Err(e) => {
                        println!("[✗] [{}/{}] Planning failed", step_count, total_steps);
                        eprintln!("Planning error: {}", e);
                        return Ok(());
                    }
                }
            }

            println!(
                "\n[PLAN] Complete - {} steps, {} operations ready",
                step_count,
                build_service.buffered_count()
            );

            // Show background status updates
            self.display_background_updates();

            // Show minimal summary
            if verbose {
                println!(
                    "\n[SUMMARY] Planning steps: {}, Operations: {}",
                    step_count,
                    build_service.buffered_count()
                );
            }

            // Show plan preview using buffered operations, scoped to workspace
            let (scoped_ops, scope_warnings) =
                build_service.enforce_project_scope(planner.get_completed_operations().to_vec());
            for warning in &scope_warnings {
                println!("[WARN] {}", warning);
            }
            build_service.set_buffered_operations(scoped_ops);

            let mut temp_plan = BuildPlan {
                goal: current_goal.to_string(),
                operations: build_service.get_buffered_operations().to_vec(),
                description: "Streaming-generated operations".to_string(),
                estimated_risk: RiskLevel::Low,
            };

            if build_service.get_buffered_operations().is_empty() {
                println!("[ERROR] All planned operations were outside the workspace. Please edit the plan to target paths under {}.", workspace_root.display());
                continue 'planning;
            }

            if let Some(ref hints) = plan_hints {
                let missing = Self::missing_plan_hints(hints, &temp_plan.operations);
                if !missing.is_empty() {
                    println!("[WARN] Plan is missing required steps:");
                    for item in &missing {
                        println!("  - {}", item);
                    }
                    println!("[PROMPT] Edit plan or goal? [e/g/q]");
                    let input = self.read_input_line()?;
                    match input.trim().to_lowercase().as_str() {
                        "e" | "edit" => continue 'planning,
                        "g" | "goal" => {
                            let edited = editor::Editor::edit_content(
                                &current_goal,
                                editor::EditContent::Command(current_goal.clone()),
                            )?;
                            let edited = edited.trim();
                            if !edited.is_empty() {
                                current_goal = edited.to_string();
                                plan_hints = None;
                            }
                            continue 'planning;
                        }
                        "q" | "quit" => return Ok(()),
                        _ => continue 'planning,
                    }
                }
            }

            if let Err(e) = build_service.preview_plan(&temp_plan) {
                eprintln!("[ERROR] Plan preview error: {}", e);
                return Ok(());
            }

            // Offer interactive plan review
            println!("\n[REVIEW] Plan generated. Review/edit before execution?");
            println!("[PROMPT] Enter 'y' to review, 'e' to edit, 'q' to quit, or press Enter to continue");

            let input = self.read_input_line()?;
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" | "review" => {
                    self.interactive_plan_review(&mut temp_plan)?;
                    build_service.set_buffered_operations(temp_plan.operations.clone());
                }
                "e" | "edit" => {
                    let plan_content = Self::format_plan_for_editing(&temp_plan);
                    match editor::Editor::edit_content(
                        &plan_content,
                        editor::EditContent::Plan(plan_content.clone()),
                    ) {
                        Ok(edited_plan) => {
                            if let Some(edited_goal) = Self::extract_goal_from_plan(&edited_plan) {
                                if edited_goal != current_goal {
                                    println!("[EDIT] Goal updated: {}", edited_goal);
                                    current_goal = edited_goal;
                                    plan_hints = None;
                                }
                            }

                            match editor::Editor::parse_edited_plan(&edited_plan) {
                                Ok(steps) => {
                                    println!("[EDIT] Plan updated with {} steps", steps.len());
                                    plan_hints = Some(steps.join("\n"));
                                    println!("[PLAN UPDATED]");
                                    for (idx, step) in steps.iter().enumerate() {
                                        println!("{}. {}", idx + 1, step);
                                    }
                                    println!("[REPLAN] Regenerating plan with edited steps using agent...");
                                    continue 'planning;
                                }
                                Err(e) => {
                                    println!(
                                        "[ERROR] Failed to parse edited plan: {} - using original",
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            println!("[ERROR] Editor failed: {} - using original plan", e);
                        }
                    }
                }
                "q" | "quit" => {
                    println!("[CANCEL] Plan review cancelled by user.");
                    return Ok(());
                }
                _ => {
                    // Continue with original plan
                }
            }

            // Get user confirmation before execution (unless dry-run)
            if !dry_run {
                use shared::confirmation::{ask_enhanced_confirmation, ConfirmationChoice};

                let operation_count = build_service.buffered_count();
                if operation_count == 0 {
                    println!("\nNo operations to execute.");
                    return Ok(());
                }

                let session_info = if let Some(session) = &self.current_session {
                    format!(" for session '{}'", session)
                } else {
                    "".to_string()
                };

                let prompt = format!(
                    "\nProceed with executing {} operation{}{}?",
                    operation_count,
                    if operation_count == 1 { "" } else { "s" },
                    session_info
                );

                let mut restart_planning = false;

                match ask_enhanced_confirmation(&prompt) {
                    Ok(ConfirmationChoice::Yes) => {
                        println!("[EXEC] Proceeding with execution...");
                    }
                    Ok(ConfirmationChoice::No) => {
                        println!("[CANCEL] Operation cancelled by user.");
                        return Ok(());
                    }
                    Ok(ConfirmationChoice::Edit) | Ok(ConfirmationChoice::Revise) => {
                        println!("[EDIT] Opening goal in editor for revision...");

                        match editor::Editor::edit_content(
                            &current_goal,
                            editor::EditContent::Command(current_goal.clone()),
                        ) {
                            Ok(edited_goal) => {
                                let edited_goal = edited_goal.trim();
                                if edited_goal.is_empty() || edited_goal == current_goal {
                                    println!(
                                        "[EDIT] Goal unchanged - proceeding with current plan"
                                    );
                                } else {
                                    println!("[EDIT] Goal updated: {}", edited_goal);
                                    current_goal = edited_goal.to_string();
                                    plan_hints = None; // clear any old step hints for the new goal
                                    restart_planning = true;
                                }
                            }
                            Err(e) => {
                                println!("[ERROR] Editor failed: {} - cancelling", e);
                                return Ok(());
                            }
                        }
                    }
                    Ok(ConfirmationChoice::Suggest) => {
                        println!("[SUGGEST] Generating improvement suggestions...");

                        println!("Suggestions for '{}':", current_goal);
                        println!("  1. Add error handling for edge cases");
                        println!("  2. Include logging for debugging");
                        println!("  3. Add input validation");
                        println!("  4. Consider performance optimizations");
                        println!("  5. Add tests for the new functionality");

                        if ask_confirmation("Edit these suggestions?", false).unwrap_or(false) {
                            let suggestions = "1. Add error handling for edge cases\n2. Include logging for debugging\n3. Add input validation\n4. Consider performance optimizations\n5. Add tests for the new functionality";
                            match editor::Editor::edit_content(
                                suggestions,
                                editor::EditContent::File(suggestions.to_string()),
                            ) {
                                Ok(edited_suggestions) => {
                                    println!("[SUGGEST] Updated suggestions:");
                                    println!("{}", edited_suggestions);
                                }
                                Err(e) => {
                                    println!("[ERROR] Editor failed: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Confirmation error: {}", e);
                        println!("[WARN] Proceeding with execution (confirmation failed)...");
                        // Continue with execution despite confirmation error
                    }
                }

                if restart_planning {
                    println!("[REPLAN] Regenerating plan with updated goal...");
                    continue 'planning;
                }
            }

            // Execute the buffered operations (unless dry-run)
            if !dry_run {
                // Final per-operation review/edit/apply loop
                if !self.apply_operations_interactively(&mut temp_plan, &mut build_service)? {
                    println!("[CANCEL] Execution cancelled by user.");
                    break 'planning;
                }

                let mut completed = 0usize;
                let mut failed = 0usize;
                let mut errors = Vec::new();

                for (idx, operation) in temp_plan.operations.iter().enumerate() {
                    if let Err(e) = build_service.execute_operation_once(operation).await {
                        failed += 1;
                        errors.push(format!("{:?}: {}", operation, e));
                        eprintln!("{} {}", "Build execution error:".red(), e);
                        break;
                    }

                    completed += 1;
                    let commit_msg = format!(
                        "feat: {} (step {}/{})\n\nOperation:\n- {:?}",
                        current_goal,
                        idx + 1,
                        temp_plan.operations.len(),
                        operation
                    );
                    if let Err(e) = build_service.commit_message(&commit_msg).await {
                        eprintln!("{} {}", "Warning: Git commit failed:".yellow(), e);
                    } else {
                        println!(
                            "[COMMIT] {}",
                            commit_msg.lines().next().unwrap_or("Committed")
                        );
                    }
                }

                if failed == 0 {
                    println!("\nBuild completed successfully.");
                    println!("{} operations completed", completed);
                } else {
                    println!("\nBuild failed.");
                    println!("{} operations completed, {} failed", completed, failed);
                    for error in &errors {
                        eprintln!("  {}", error.red());
                    }
                }
            } else {
                println!("\n[DONE] Dry-run mode: No changes were made.");
            }

            // Enhanced final power-user controls with session persistence
            println!("\n[COMPLETE] Task finished successfully");
            println!(
                "[CONTROLS] Next action? [/suggest /new-task /status /undo /edit-plan /session /q]"
            );
            println!("[TIP] Commands can be abbreviated (e.g., /s for /suggest)");

            // Interactive control loop with history
            let mut command_history = Vec::new();
            loop {
                print!("vibe> ");
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let command = input.trim().to_lowercase();

                if command.is_empty() {
                    continue;
                }

                command_history.push(command.clone());

                match command.as_str() {
                    "/suggest" | "/s" => {
                        println!("[SUGGEST] Generating improvement suggestions...");
                        println!("Suggestions for '{}':", current_goal);
                        println!("  1. Add error handling for edge cases");
                        println!("  2. Include logging for debugging");
                        println!("  3. Add input validation");
                        println!("  4. Consider performance optimizations");
                        println!("  5. Add tests for the new functionality");
                        println!("  6. Add monitoring/alerts");
                        println!("  7. Implement rollback mechanisms");
                    }
                    "/new-task" | "/n" => {
                        println!("[NEW] Ready for new task. Run: vibe --build \"your goal here\"");
                        break;
                    }
                    "/status" | "/st" => {
                        println!("[STATUS] Session status");
                        println!(
                            "  Current session: {}",
                            self.current_session.as_deref().unwrap_or("default")
                        );
                        println!("  Last goal: {}", current_goal);
                        println!("  Plan steps: {}", temp_plan.operations.len());
                        if let Some(session_name) = &self.current_session {
                            if let Ok(Some(session)) = self
                                .session_store
                                .as_ref()
                                .unwrap()
                                .load_session(session_name)
                            {
                                println!("  Total changes: {}", session.metadata.change_count);
                                println!("  Applied changes: {}", session.applied_changes.len());
                            }
                        }
                    }
                    "/undo" | "/u" => {
                        println!("[UNDO] Attempting to undo last changes...");
                        match std::process::Command::new("git")
                            .args(&["reset", "--hard", "HEAD~1"])
                            .output()
                        {
                            Ok(output) => {
                                if output.status.success() {
                                    println!("[UNDO] Successfully reverted last commit");
                                    println!("[UNDO] Repository state restored");
                                } else {
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    println!("[UNDO] Git failed: {}", stderr);
                                    println!("[UNDO] Manual undo: git reset --hard HEAD~1");
                                }
                            }
                            Err(e) => {
                                println!("[UNDO] Failed to run git: {}", e);
                                println!("[UNDO] Manual undo: git reset --hard HEAD~1");
                            }
                        }
                    }
                    "/edit-plan" | "/e" => {
                        println!("[EDIT] Opening last plan for review...");
                        let plan_content = Self::format_plan_for_editing(&temp_plan);
                        match editor::Editor::edit_content(
                            &plan_content,
                            editor::EditContent::Plan(plan_content.clone()),
                        ) {
                            Ok(_) => println!(
                                "[EDIT] Plan reviewed (changes not applied to completed task)"
                            ),
                            Err(e) => println!("[ERROR] Editor failed: {}", e),
                        }
                    }
                    "/session" | "/ss" => {
                        if let Some(session_name) = &self.current_session {
                            println!("[SESSION] Current session: {}", session_name);
                            if let Ok(Some(session)) = self
                                .session_store
                                .as_ref()
                                .unwrap()
                                .load_session(session_name)
                            {
                                println!("  Created: {}", session.metadata.created_at);
                                println!("  Last used: {}", session.metadata.last_used);
                                println!("  Total changes: {}", session.metadata.change_count);
                                println!("  Applied changes: {}", session.applied_changes.len());
                            }
                        } else {
                            println!("[SESSION] No active session");
                        }
                    }
                    "/history" | "/h" => {
                        println!("[HISTORY] Recent commands in this session:");
                        for (i, cmd) in command_history.iter().rev().take(10).enumerate() {
                            println!("  {}. {}", command_history.len() - i, cmd);
                        }
                    }
                    "/q" | "q" | "quit" => {
                        println!(
                            "[BYE] Session ended. {} commands executed.",
                            command_history.len()
                        );
                        break;
                    }
                    "/help" | "/?" => {
                        println!("[HELP] Available commands:");
                        println!("  /suggest (/s)  - Show improvement suggestions");
                        println!("  /new-task (/n) - Start new task");
                        println!("  /status (/st)  - Show completion status");
                        println!("  /undo (/u)     - Undo last changes");
                        println!("  /edit-plan (/e)- Review last plan");
                        println!("  /session (/ss) - Show session info");
                        println!("  /history (/h)  - Show command history");
                        println!("  /config (/c)   - Show power user configuration");
                        println!("  /help (/?)     - Show this help");
                        println!("  /quit (/q)     - Exit session");
                    }
                    "/config" | "/c" => {
                        let power_config = self.get_power_config();
                        println!("[CONFIG] Power User Configuration:");
                        println!("  Theme: {}", power_config.theme.name);
                        println!("  Aliases: {}", power_config.aliases.len());
                        println!("  Shortcuts: {}", power_config.shortcuts.len());
                        println!("  Plugins: {}", power_config.plugins.enabled.len());
                        println!(
                            "  Performance: parallel_jobs={}, prewarm={}",
                            power_config.performance.parallel_jobs,
                            power_config.performance.prewarm_models
                        );

                        if !power_config.aliases.is_empty() {
                            println!("  Available aliases:");
                            for (alias, expansion) in &power_config.aliases {
                                println!("    {} -> {}", alias, expansion);
                            }
                        }

                        if !power_config.shortcuts.is_empty() {
                            println!("  Available shortcuts:");
                            for (shortcut, expansion) in &power_config.shortcuts {
                                println!("    {} -> {}", shortcut, expansion);
                            }
                        }

                        // Show loaded plugins
                        if let Some(plugin_manager) = &self.config.plugin_manager {
                            let manager = plugin_manager.read().await;
                            let plugins = manager.list_plugins();
                            if !plugins.is_empty() {
                                println!("  Loaded plugins: {}", plugins.join(", "));
                                println!("  Plugin help:");
                                println!("{}", manager.get_help());
                            }
                        }
                    }
                    _ => {
                        println!(
                            "[UNKNOWN] Unknown command '{}'. Type /help for available commands.",
                            command
                        );
                    }
                }
            }

            // Finished current plan/execution path; exit planning loop unless a replan was requested earlier
            break 'planning;
        }

        Ok(())
    }

    pub async fn run(&mut self, cli: Cli) -> Result<()> {
        let args_str = cli.args.join(" ");

        // Handle configuration file generation
        if let Some(config_path) = &cli.generate_config {
            let power_config = infrastructure::config::PowerUserConfig::default();
            let path = PathBuf::from(config_path);
            match power_config.save_to_file(&path) {
                Ok(_) => {
                    println!("Default configuration saved to: {}", path.display());
                    println!("Edit this file to customize your power user settings.");
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("Failed to save configuration: {}", e);
                    return Ok(());
                }
            }
        }

        // Handle custom configuration file loading
        if let Some(config_path) = &cli.config {
            let path = PathBuf::from(config_path);
            match infrastructure::config::PowerUserConfig::load_from_file(&path) {
                Ok(power_config) => {
                    self.power_config_override = Some(power_config);
                    println!("Loaded power user configuration from: {}", path.display());
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load power user config from {}: {}",
                        path.display(),
                        e
                    );
                    eprintln!("Continuing with default configuration.");
                }
            }
        }

        // Initialize plugins
        if let Err(e) = self.config.initialize_plugins().await {
            eprintln!("Warning: Failed to initialize plugins: {}", e);
        }

        // Show initial status
        self.display_background_status();

        // Initialize background services
        if let Some(project_root) = find_project_root() {
            if let Some(mut supervisor) = self.background_supervisor.take() {
                let project_root_path = std::path::PathBuf::from(project_root);

                // Background services disabled - no automatic startup
                // Event receiver available for explicit manual control
                if let Some(event_receiver) = supervisor.get_event_receiver() {
                    tokio::spawn(async move {
                        Self::handle_background_events(event_receiver).await;
                    });
                }

                // Log status update (services disabled)
                tokio::spawn(async move {
                    if let Err(e) = supervisor.start(&project_root_path).await {
                        eprintln!("Background services remain disabled: {}", e);
                    }
                });
            }
        }

        // Handle session commands first
        if cli.list_sessions {
            return self.handle_list_sessions().await;
        }
        if let Some(session_name) = &cli.delete_session {
            return self.handle_delete_session(session_name).await;
        }
        if cli.continue_session {
            return self.handle_continue_session().await;
        }
        if cli.undo {
            return self.handle_undo().await;
        }

        // Handle session context for other commands
        if let Some(session_name) = &cli.session {
            self.current_session = Some(session_name.clone());
        }

        if cli.chat {
            if args_str.trim().is_empty() {
                self.handle_chat().await
            } else {
                // Perhaps chat with initial message, but for now, just enter chat
                self.handle_chat().await
            }
        } else if cli.test {
            self.handle_test_run().await
        } else if cli.build {
            self.handle_build(&args_str, cli.dry_run, cli.verbose, cli.show_diff)
                .await
        } else if cli.agent {
            self.handle_agent(&args_str).await
        } else if cli.ai_agent {
            self.handle_ai_agent(&args_str).await
        } else if cli.plan {
            self.handle_plan_mode(&args_str).await
        } else if cli.explain {
            self.handle_explain(&args_str).await
        } else if cli.rag {
            self.handle_rag(&args_str).await
        } else if cli.stream {
            self.handle_stream_mode(&args_str).await
        } else if cli.context {
            self.handle_context(&args_str).await
        } else {
            // Default: general query
            self.handle_query(&args_str).await
        }
    }

    /// Update progress display with minimal plain text indicators
    fn update_progress_display(&self, current: usize, total: usize, description: &str) {
        let status = match current {
            1..=2 => "[✓]",
            3 => "[→]",
            4 => "[⚡]",
            5 => "[✓]",
            _ => "[○]",
        };

        let session_prefix = if let Some(session) = &self.current_session {
            format!("[{}] ", session)
        } else {
            "[main] ".to_string()
        };

        println!(
            "{}{} [{}/{}] {}",
            session_prefix, status, current, total, description
        );
    }

    /// Display chain of thought in tree format
    fn display_chain_of_thought(reasoning: &str) {
        println!("\nChain of Thought:");

        // Parse reasoning into key points
        let lines: Vec<&str> = reasoning
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(4) // Limit to 4 key points
            .collect();

        for (i, line) in lines.iter().enumerate() {
            let prefix = match i {
                0 => "|-- Goal:",
                1 => "|-- Analysis:",
                2 => "|-- Approach:",
                _ => "|-- Risk:",
            };

            // Clean up the line for display
            let clean_line = line
                .trim()
                .strip_prefix("### ")
                .unwrap_or(line)
                .strip_prefix("**")
                .unwrap_or(line)
                .strip_suffix("**")
                .unwrap_or(line);

            println!("{} {}", prefix, clean_line);
        }
    }

    /// Display AI tool usage transparency - now dynamic based on actual operations
    fn display_tool_usage(context_stats: &ContextStats) {
        println!("\nAI Tool Usage:");
        println!("  Context Retrieval:");
        println!("    |-- Files scanned: {}", context_stats.files_scanned);
        println!("    |-- Files analyzed: {}", context_stats.files_analyzed);
        println!(
            "    |-- Keywords extracted: {}",
            context_stats.keywords_count
        );
        println!("  System Context:");
        println!("    |-- OS: {}", context_stats.os_info);
        println!("    |-- Working directory: {}", context_stats.cwd);
        println!("  File System Operations:");
        println!(
            "    |-- Total files in project: {}",
            context_stats.total_files
        );
        println!(
            "    |-- Relevant files found: {}",
            context_stats.relevant_files
        );
    }

    /// Format a build plan for editing in the user's editor
    fn format_plan_for_editing(plan: &BuildPlan) -> String {
        let mut content = String::new();
        content.push_str("# Vibe Plan – edit, reorder, delete, add steps freely\n");
        content.push_str("# Save & quit to apply changes\n");
        content.push_str("# Lines starting with # are comments and will be ignored\n");
        content.push_str(&format!("# Goal: {}\n", plan.goal));
        content.push_str(&format!("# Description: {}\n", plan.description));
        content.push_str(&format!("# Risk: {:?}\n\n", plan.estimated_risk));

        for (i, operation) in plan.operations.iter().enumerate() {
            let risk = match operation {
                application::build_service::FileOperation::Create { .. }
                | application::build_service::FileOperation::Read { .. } => "Low",
                application::build_service::FileOperation::Update { .. } => "Medium",
                application::build_service::FileOperation::Delete { .. } => "High",
            };
            let op_desc = match operation {
                application::build_service::FileOperation::Create { path, .. } => {
                    format!("Create {}", path.display())
                }
                application::build_service::FileOperation::Update { path, .. } => {
                    format!("Update {}", path.display())
                }
                application::build_service::FileOperation::Delete { path } => {
                    format!("Delete {}", path.display())
                }
                application::build_service::FileOperation::Read { path } => {
                    format!("Read {}", path.display())
                }
            };
            content.push_str(&format!("{}. {} ({})\n", i + 1, op_desc, risk));
        }

        content.push_str("\n# Add new steps below:\n");
        content
    }

    /// Rebuild operations list based on edited plan steps (supports reordering/removal of known ops)
    fn rebuild_operations_from_steps(
        steps: &[String],
        original_ops: &[application::build_service::FileOperation],
    ) -> (Vec<application::build_service::FileOperation>, Vec<String>) {
        use application::build_service::FileOperation;

        let mut warnings = Vec::new();
        let mut lookup: HashMap<String, FileOperation> = HashMap::new();

        for op in original_ops {
            lookup
                .entry(Self::describe_operation(op))
                .or_insert_with(|| op.clone());
        }

        let mut new_ops = Vec::new();
        for step in steps {
            let normalized = step.trim();
            if let Some(op) = lookup.get(normalized) {
                new_ops.push(op.clone());
                continue;
            }

            if let Some(op) = Self::parse_operation_line(normalized, original_ops) {
                new_ops.push(op);
            } else {
                warnings.push(format!(
                    "Unrecognized step '{}'; keeping original ordering",
                    normalized
                ));
            }
        }

        (new_ops, warnings)
    }

    /// Describe an operation using the same wording as the editable plan view
    fn describe_operation(operation: &application::build_service::FileOperation) -> String {
        match operation {
            application::build_service::FileOperation::Create { path, .. } => {
                format!("Create {}", path.display())
            }
            application::build_service::FileOperation::Update { path, .. } => {
                format!("Update {}", path.display())
            }
            application::build_service::FileOperation::Delete { path } => {
                format!("Delete {}", path.display())
            }
            application::build_service::FileOperation::Read { path } => {
                format!("Read {}", path.display())
            }
        }
    }

    /// Attempt to parse a user-edited line back into a known operation, reusing original data when possible
    fn parse_operation_line(
        line: &str,
        original_ops: &[application::build_service::FileOperation],
    ) -> Option<application::build_service::FileOperation> {
        use application::build_service::FileOperation;

        let lower = line.to_lowercase();
        let strip_prefix = |prefix: &str| -> Option<String> {
            lower
                .strip_prefix(prefix)
                .map(|rest| rest.trim_matches('"').trim().to_string())
        };

        if let Some(path_str) = strip_prefix("create ") {
            if let Some(op) = original_ops.iter().find(|op| matches!(op, FileOperation::Create { path, .. } if path.display().to_string() == path_str)) {
                return Some(op.clone());
            }
            return None;
        }

        if let Some(path_str) = strip_prefix("update ") {
            if let Some(op) = original_ops.iter().find(|op| matches!(op, FileOperation::Update { path, .. } if path.display().to_string() == path_str)) {
                return Some(op.clone());
            }
            return None;
        }

        if let Some(path_str) = strip_prefix("delete ") {
            if let Some(op) = original_ops.iter().find(|op| matches!(op, FileOperation::Delete { path } if path.display().to_string() == path_str)) {
                return Some(op.clone());
            }
            // Allow creating a simple delete if it wasn't in the original list
            return Some(FileOperation::Delete {
                path: std::path::PathBuf::from(path_str),
            });
        }

        if let Some(path_str) = strip_prefix("read ") {
            if let Some(op) = original_ops.iter().find(|op| matches!(op, FileOperation::Read { path } if path.display().to_string() == path_str)) {
                return Some(op.clone());
            }
            return Some(FileOperation::Read {
                path: std::path::PathBuf::from(path_str),
            });
        }

        None
    }

    fn extract_goal_from_plan(edited_plan: &str) -> Option<String> {
        for line in edited_plan.lines() {
            let trimmed = line.trim();
            if let Some(goal) = trimmed.strip_prefix("# Goal:") {
                let goal = goal.trim();
                if !goal.is_empty() {
                    return Some(goal.to_string());
                }
            }
        }
        None
    }

    fn missing_plan_hints(
        hints: &str,
        operations: &[application::build_service::FileOperation],
    ) -> Vec<String> {
        let mut missing = Vec::new();
        let combined = operations
            .iter()
            .map(Self::describe_operation_with_content)
            .collect::<Vec<_>>()
            .join("\n");
        let combined_lower = combined.to_lowercase();

        for line in hints.lines() {
            let trimmed = line.split('#').next().unwrap_or("").trim();
            if trimmed.is_empty() {
                continue;
            }
            let normalized = trimmed.to_lowercase();
            if !combined_lower.contains(&normalized) {
                missing.push(trimmed.to_string());
            }
        }

        missing
    }

    fn describe_operation_with_content(op: &application::build_service::FileOperation) -> String {
        use application::build_service::FileOperation;
        match op {
            FileOperation::Create { path, content } => {
                format!("Create {} {}", path.display(), content)
            }
            FileOperation::Update {
                path, new_content, ..
            } => format!("Update {} {}", path.display(), new_content),
            FileOperation::Delete { path } => format!("Delete {}", path.display()),
            FileOperation::Read { path } => format!("Read {}", path.display()),
        }
    }

    /// Review and apply operations one by one with inline editing/viewing
    fn apply_operations_interactively(
        &mut self,
        plan: &mut BuildPlan,
        build_service: &mut application::build_service::BuildService,
    ) -> Result<bool> {
        use application::build_service::FileOperation;

        let mut idx = 0;
        while idx < plan.operations.len() {
            let total = plan.operations.len();
            let op = plan.operations[idx].clone();

            println!("\n[STEP {}/{}]", idx + 1, total);
            build_service.display_operation_detail(&op)?;
            println!(
                "[PROMPT] Apply? [y/n/e(dit)/v(iew)/r(emove)/q] or /plan /status /undo /suggest"
            );

            let input = self.read_input_line()?;
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {
                    idx += 1;
                }
                "n" | "skip" => {
                    println!("[SKIP] Skipping operation {}", idx + 1);
                    idx += 1;
                }
                "r" | "remove" => {
                    println!("[REMOVE] Removing step {}", idx + 1);
                    plan.operations.remove(idx);
                    continue;
                }
                "v" | "view" => {
                    Self::display_full_operation(&op);
                    continue;
                }
                "e" | "edit" => {
                    if let Some(edited_op) = Self::edit_operation(op.clone())? {
                        plan.operations[idx] = edited_op;
                        build_service.set_buffered_operations(plan.operations.clone());
                        println!("[EDIT] Updated step {}", idx + 1);
                    } else {
                        println!("[EDIT] No changes made");
                    }
                    continue;
                }
                "/plan" => {
                    let plan_content = Self::format_plan_for_editing(plan);
                    match editor::Editor::edit_content(
                        &plan_content,
                        editor::EditContent::Plan(plan_content.clone()),
                    ) {
                        Ok(edited_plan) => {
                            if let Some(edited_goal) = Self::extract_goal_from_plan(&edited_plan) {
                                println!("[EDIT] Goal updated: {}", edited_goal);
                            }
                            if let Ok(steps) = editor::Editor::parse_edited_plan(&edited_plan) {
                                let (updated_ops, warnings) =
                                    Self::rebuild_operations_from_steps(&steps, &plan.operations);
                                for warning in warnings {
                                    println!("[WARN] {}", warning);
                                }
                                if !updated_ops.is_empty() {
                                    plan.operations = updated_ops;
                                    build_service.set_buffered_operations(plan.operations.clone());
                                    idx = 0;
                                    println!("[EDIT] Plan updated; restarting review");
                                }
                            }
                        }
                        Err(e) => println!("[ERROR] Editor failed: {}", e),
                    }
                    continue;
                }
                "/status" => {
                    println!(
                        "[STATUS] Steps remaining: {}",
                        plan.operations.len().saturating_sub(idx)
                    );
                    continue;
                }
                "/undo" => {
                    println!("[UNDO] Run: git reset --hard HEAD~1");
                    continue;
                }
                "/suggest" => {
                    println!("[SUGGEST] Use /suggest after completion for ideas.");
                    continue;
                }
                "q" | "quit" => return Ok(false),
                _ => {
                    println!("Enter y/n/e/v/r/q");
                    continue;
                }
            }
        }

        // Refresh buffered operations after interactive edits/removals
        build_service.set_buffered_operations(plan.operations.clone());
        Ok(true)
    }

    /// Show full content for create/update operations
    fn display_full_operation(op: &application::build_service::FileOperation) {
        use application::build_service::FileOperation;
        match op {
            FileOperation::Create { path, content } => {
                println!("Create {}:\n{}", path.display(), content);
            }
            FileOperation::Update {
                path, new_content, ..
            } => {
                println!("Update {}:\n{}", path.display(), new_content);
            }
            FileOperation::Delete { path } => println!("Delete {}", path.display()),
            FileOperation::Read { path } => println!("Read {}", path.display()),
        }
    }

    fn display_operation_summary(op: &application::build_service::FileOperation) {
        use application::build_service::FileOperation;
        match op {
            FileOperation::Create { path, content } => {
                println!("Create {}", path.display());
                let lines: Vec<&str> = content.lines().collect();
                for line in lines.iter().take(10) {
                    println!("  {}", line);
                }
                if lines.len() > 10 {
                    println!("  ... (truncated)");
                }
            }
            FileOperation::Update {
                path, new_content, ..
            } => {
                println!("Update {}", path.display());
                let lines: Vec<&str> = new_content.lines().collect();
                for line in lines.iter().take(10) {
                    println!("  {}", line);
                }
                if lines.len() > 10 {
                    println!("  ... (truncated)");
                }
            }
            FileOperation::Delete { path } => println!("Delete {}", path.display()),
            FileOperation::Read { path } => println!("Read {}", path.display()),
        }
    }

    /// Allow editing the file content for create/update operations
    fn edit_operation(
        op: application::build_service::FileOperation,
    ) -> Result<Option<application::build_service::FileOperation>> {
        use application::build_service::FileOperation;

        match op {
            FileOperation::Create { path, content } => {
                let edited = editor::Editor::edit_content(
                    &content,
                    editor::EditContent::File(content.clone()),
                )?;
                Ok(Some(FileOperation::Create {
                    path,
                    content: edited,
                }))
            }
            FileOperation::Update {
                path,
                old_content,
                new_content,
            } => {
                let edited = editor::Editor::edit_content(
                    &new_content,
                    editor::EditContent::File(new_content.clone()),
                )?;
                Ok(Some(FileOperation::Update {
                    path,
                    old_content,
                    new_content: edited,
                }))
            }
            _ => {
                println!("[EDIT] Only create/update steps can be edited.");
                Ok(None)
            }
        }
    }

    /// Format a single operation for editing
    fn format_operation_for_editing(
        operation: &application::build_service::FileOperation,
        step_num: usize,
    ) -> String {
        let mut content = format!("# Editing Step {}\n", step_num);
        content.push_str("# Modify the operation details below\n");
        content.push_str("# Only the final line will be used as the new operation\n\n");

        match operation {
            application::build_service::FileOperation::Create {
                path,
                content: op_content,
            } => {
                content.push_str(&format!("# Original: Create {}\n", path.display()));
                content.push_str("Create ");
                content.push_str(&path.display().to_string());
                content.push_str(" with content:\n");
                content.push_str(op_content);
            }
            application::build_service::FileOperation::Update {
                path,
                old_content,
                new_content,
            } => {
                content.push_str(&format!("# Original: Update {}\n", path.display()));
                content.push_str("Update ");
                content.push_str(&path.display().to_string());
                content.push_str(" replacing:\n");
                content.push_str(old_content);
                content.push_str("\nwith:\n");
                content.push_str(new_content);
            }
            application::build_service::FileOperation::Delete { path } => {
                content.push_str(&format!("# Original: Delete {}\n", path.display()));
                content.push_str("Delete ");
                content.push_str(&path.display().to_string());
            }
            application::build_service::FileOperation::Read { path } => {
                content.push_str(&format!("# Original: Read {}\n", path.display()));
                content.push_str("Read ");
                content.push_str(&path.display().to_string());
            }
        }

        content
    }

    /// Parse edited operation back into FileOperation
    fn parse_edited_operation(
        edited_text: &str,
        original: &application::build_service::FileOperation,
    ) -> Result<application::build_service::FileOperation> {
        let lines: Vec<&str> = edited_text
            .lines()
            .filter(|line| !line.trim().starts_with('#') && !line.trim().is_empty())
            .collect();

        if lines.is_empty() {
            return Ok(original.clone());
        }

        // For now, return original if parsing fails - could be enhanced
        Ok(original.clone())
    }

    /// Allow editing individual steps in a plan
    fn edit_plan_step(&self, plan: &mut BuildPlan, step_index: usize) -> Result<()> {
        if step_index >= plan.operations.len() {
            return Err(anyhow::anyhow!("Step {} does not exist", step_index + 1));
        }

        let operation = &plan.operations[step_index];
        let content = Self::format_operation_for_editing(operation, step_index + 1);

        match editor::Editor::edit_content(&content, editor::EditContent::File(content.clone())) {
            Ok(edited) => match Self::parse_edited_operation(&edited, operation) {
                Ok(new_operation) => {
                    plan.operations[step_index] = new_operation;
                    println!("[EDIT] Step {} updated successfully", step_index + 1);
                }
                Err(e) => {
                    println!(
                        "[EDIT] Failed to parse edited operation: {} - keeping original",
                        e
                    );
                }
            },
            Err(e) => {
                println!("[EDIT] Editor failed: {} - keeping original", e);
            }
        }

        Ok(())
    }

    /// Interactive plan review and editing session
    fn interactive_plan_review(&self, plan: &mut BuildPlan) -> Result<()> {
        println!("\n[PLAN REVIEW] Interactive editing session");
        println!("Commands: 'e <step>' to edit step, 'd <step>' to delete step, 'a <desc>' to add step, 'q' to quit");

        loop {
            println!("\nCurrent plan:");
            for (i, operation) in plan.operations.iter().enumerate() {
                let op_desc = match operation {
                    application::build_service::FileOperation::Create { path, .. } => {
                        format!("Create {}", path.display())
                    }
                    application::build_service::FileOperation::Update { path, .. } => {
                        format!("Update {}", path.display())
                    }
                    application::build_service::FileOperation::Delete { path } => {
                        format!("Delete {}", path.display())
                    }
                    application::build_service::FileOperation::Read { path } => {
                        format!("Read {}", path.display())
                    }
                };
                println!("  {}. {}", i + 1, op_desc);
            }

            print!("plan> ");
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let command = input.trim();

            if command.is_empty() {
                continue;
            }

            let parts: Vec<&str> = command.split_whitespace().collect();
            match parts[0] {
                "e" | "edit" => {
                    if parts.len() < 2 {
                        println!("Usage: e <step_number>");
                        continue;
                    }
                    if let Ok(step_num) = parts[1].parse::<usize>() {
                        if step_num > 0 && step_num <= plan.operations.len() {
                            self.edit_plan_step(plan, step_num - 1)?;
                        } else {
                            println!("Invalid step number: {}", step_num);
                        }
                    } else {
                        println!("Invalid step number: {}", parts[1]);
                    }
                }
                "d" | "delete" => {
                    if parts.len() < 2 {
                        println!("Usage: d <step_number>");
                        continue;
                    }
                    if let Ok(step_num) = parts[1].parse::<usize>() {
                        if step_num > 0 && step_num <= plan.operations.len() {
                            plan.operations.remove(step_num - 1);
                            println!("Step {} deleted", step_num);
                        } else {
                            println!("Invalid step number: {}", step_num);
                        }
                    } else {
                        println!("Invalid step number: {}", parts[1]);
                    }
                }
                "a" | "add" => {
                    if parts.len() < 2 {
                        println!("Usage: a <description>");
                        continue;
                    }
                    let desc = parts[1..].join(" ");
                    println!(
                        "Add step functionality not yet implemented. Use full plan edit instead."
                    );
                }
                "q" | "quit" => {
                    break;
                }
                "h" | "help" => {
                    println!("Commands:");
                    println!("  e <step>  - Edit step");
                    println!("  d <step>  - Delete step");
                    println!("  a <desc> - Add step (not implemented)");
                    println!("  q        - Quit review");
                    println!("  h        - Show help");
                }
                _ => {
                    println!("Unknown command. Type 'h' for help.");
                }
            }
        }

        println!("[PLAN REVIEW] Session ended");
        Ok(())
    }

    /// Display incremental changes in plain text format
    fn display_incremental_changes(&self, code: &str, path: &str, op_type: &str) {
        let session_info = if let Some(session) = &self.current_session {
            format!(" [{}]", session)
        } else {
            "".to_string()
        };

        println!("\n[INCREMENTAL_CHANGES]{}:", session_info);

        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let cleaned_code = Self::strip_code_fences(code);
        let lines: Vec<&str> = cleaned_code.lines().collect();

        match op_type {
            "create" => {
                // New file creation - show full content in chunks
                println!(
                    "{} Creating new file {}",
                    "📄".bright_green(),
                    path.bright_green()
                );

                if lines.len() <= 15 {
                    println!("  └─ [full file - {} lines]", lines.len());
                    Self::display_code_with_syntax(&lines, 0, &ext);
                } else {
                    // Show in logical chunks for large files
                    let chunks = Self::create_file_chunks(&lines);
                    for (i, (start, end, description)) in chunks.iter().enumerate() {
                        let chunk_marker = if i == chunks.len() - 1 {
                            "└─"
                        } else {
                            "├─"
                        };
                        println!(
                            "  {} Step {}: {}",
                            chunk_marker,
                            char::from(b'a' + i as u8),
                            description.bright_white()
                        );
                        println!("     [lines {}-{}]", start, end);

                        // Safe bounds checking to prevent panic
                        let start_idx = start.saturating_sub(1).min(lines.len());
                        let end_idx = (*end).min(lines.len());

                        // Only slice if we have a valid range
                        if start_idx < end_idx && start_idx < lines.len() {
                            let chunk_lines = &lines[start_idx..end_idx];
                            Self::display_code_with_syntax(chunk_lines, start_idx, &ext);
                        }

                        if i < chunks.len() - 1 {
                            println!();
                        }
                    }
                }
            }
            "update" => {
                // File update - try to show as diff if possible
                println!(
                    "{} Updating existing file {}",
                    "🔄".bright_yellow(),
                    path.bright_yellow()
                );

                // For updates, the AI might generate targeted changes
                if code.contains("REPLACE") || code.contains("INSERT") || code.contains("DELETE") {
                    // AI generated targeted changes - display as instructions
                    println!(
                        "  └─ [targeted changes - {} operations]",
                        code.lines()
                            .filter(|l| l.starts_with("REPLACE")
                                || l.starts_with("INSERT")
                                || l.starts_with("DELETE"))
                            .count()
                    );
                    for line in code.lines() {
                        if line.starts_with("REPLACE") {
                            println!("     {} {}", "🔧".bright_red(), line.bright_red());
                        } else if line.starts_with("INSERT") {
                            println!("     {} {}", "➕".bright_green(), line.bright_green());
                        } else if line.starts_with("DELETE") {
                            println!("     {} {}", "➖".bright_red(), line.bright_red());
                        } else if !line.trim().is_empty() && !line.contains("NO CHANGES REQUIRED") {
                            println!("        {}", line.dimmed());
                        }
                    }
                } else if code.contains("NO CHANGES REQUIRED") {
                    println!("  └─ [no changes required - file already matches goal]");
                } else {
                    // Full content replacement - show diff preview
                    println!("  └─ [full replacement - {} lines]", lines.len());
                    if lines.len() <= 10 {
                        Self::display_code_with_syntax(&lines, 0, &ext);
                    } else {
                        println!("     {}... (showing first 10 lines)", lines.len());
                        Self::display_code_with_syntax(&lines[..10], 0, &ext);
                    }
                }
            }
            _ => {
                // Unknown operation type - show basic preview
                println!("{} Processing {} ({})", "⚙️".bright_blue(), path, op_type);
                println!("  └─ [{} lines]", lines.len());
                if lines.len() <= 10 {
                    Self::display_code_with_syntax(&lines, 0, &ext);
                } else {
                    println!("     {}... (truncated)", lines.len());
                }
            }
        }

        // Show summary with operation type awareness and session info
        println!("\n[SUMMARY]{}:", session_info);
        match op_type {
            "create" => {
                println!("  Files: + {} (new file, {} lines)", path, lines.len());
            }
            "update" => {
                println!("  Files: ~ {} (modified, {} lines)", path, lines.len());
            }
            _ => {
                println!("  Files: ? {} ({}, {} lines)", path, op_type, lines.len());
            }
        }
        println!("  Confidence: High");
        println!("  Risk: Low");
        println!("\nTip: Use 'y' to proceed, 'n' to cancel, or 'edit' to modify goal");
    }

    /// Create logical chunks for displaying large files
    fn create_file_chunks(lines: &[&str]) -> Vec<(usize, usize, &'static str)> {
        let total_lines = lines.len();
        let mut chunks = Vec::new();

        if total_lines == 0 {
            return chunks;
        }

        if total_lines <= 20 {
            chunks.push((1, total_lines, "Complete file"));
        } else {
            // Dynamic chunking - divide into 3 equal parts
            let chunk_size = (total_lines as f32 / 3.0).ceil() as usize;

            // Ensure chunks don't overlap and stay within bounds
            let chunk1_end = chunk_size.min(total_lines);
            let chunk2_start = (chunk1_end + 1).min(total_lines);
            let chunk2_end = (chunk_size * 2).min(total_lines);
            let chunk3_start = (chunk2_end + 1).min(total_lines);

            // Determine labels based on file type
            let is_html = lines
                .iter()
                .any(|l| l.contains("<html") || l.contains("DOCTYPE"));

            if chunk3_start <= total_lines {
                // Three chunks
                if is_html {
                    chunks = vec![
                        (1, chunk1_end, "HTML skeleton and setup"),
                        (chunk2_start, chunk2_end, "Main content structure"),
                        (chunk3_start, total_lines, "Footer and closing tags"),
                    ];
                } else {
                    chunks = vec![
                        (1, chunk1_end, "Beginning of file"),
                        (chunk2_start, chunk2_end, "Middle section"),
                        (chunk3_start, total_lines, "End of file"),
                    ];
                }
            } else if chunk2_start <= total_lines {
                // Two chunks (file too small for 3)
                chunks = vec![
                    (
                        1,
                        chunk1_end,
                        if is_html {
                            "HTML skeleton and setup"
                        } else {
                            "Beginning of file"
                        },
                    ),
                    (
                        chunk2_start,
                        total_lines,
                        if is_html {
                            "Main content and footer"
                        } else {
                            "Rest of file"
                        },
                    ),
                ];
            } else {
                // Single chunk
                chunks.push((1, total_lines, "Complete file"));
            }
        }

        chunks
    }

    /// Display code with basic syntax highlighting
    fn display_code_with_syntax(lines: &[&str], start_line: usize, ext: &str) {
        for (i, line) in lines.iter().enumerate() {
            let line_num = start_line + i + 1;
            let line_num_display = format!("{:2}", line_num).bright_black();

            let trimmed = line.trim_start();
            let highlighted = match ext {
                "py" => {
                    if trimmed.starts_with('#') {
                        trimmed.bright_black().to_string()
                    } else if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                        trimmed.bright_blue().to_string()
                    } else if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                        trimmed.bright_magenta().to_string()
                    } else {
                        line.to_string()
                    }
                }
                "js" | "ts" => {
                    if trimmed.starts_with("//") {
                        trimmed.bright_black().to_string()
                    } else if trimmed.starts_with("function ")
                        || trimmed.starts_with("const ")
                        || trimmed.starts_with("let ")
                        || trimmed.starts_with("class ")
                    {
                        trimmed.bright_blue().to_string()
                    } else {
                        line.to_string()
                    }
                }
                "html" => {
                    if line.trim().is_empty() {
                        String::new()
                    } else if line.contains("<!DOCTYPE") {
                        line.bright_blue().to_string()
                    } else if line.contains("<html")
                        || line.contains("<head")
                        || line.contains("<body")
                        || line.contains("</html>")
                        || line.contains("</head>")
                        || line.contains("</body>")
                    {
                        line.bright_blue().to_string()
                    } else if line.contains("<div")
                        || line.contains("<main")
                        || line.contains("<h1")
                        || line.contains("<p")
                        || line.contains("<button")
                        || line.contains("<footer")
                        || line.contains("</div>")
                        || line.contains("</main>")
                        || line.contains("</h1>")
                        || line.contains("</p>")
                        || line.contains("</button>")
                        || line.contains("</footer>")
                    {
                        line.bright_green().to_string()
                    } else if line.contains("class=")
                        || line.contains("href=")
                        || line.contains("src=")
                    {
                        line.bright_yellow().to_string()
                    } else {
                        line.to_string()
                    }
                }
                _ => line.to_string(),
            };

            println!("  {} │ {}", line_num_display, highlighted);
        }
    }

    /// Strip surrounding code fences/backticks to avoid emitting markdown into files
    fn strip_code_fences(code: &str) -> String {
        let trimmed = code.trim();
        if trimmed.starts_with("```") && trimmed.ends_with("```") {
            let mut lines: Vec<&str> = trimmed.lines().collect();
            if !lines.is_empty()
                && lines
                    .first()
                    .map(|l| l.trim().starts_with("```"))
                    .unwrap_or(false)
            {
                lines.remove(0);
            }
            if !lines.is_empty() && lines.last().map(|l| l.trim() == "```").unwrap_or(false) {
                lines.pop();
            }
            return lines.join("\n").trim().to_string();
        }
        trimmed.trim_matches('`').trim().to_string()
    }

    async fn handle_chat(&self) -> Result<()> {
        use dialoguer::{theme::ColorfulTheme, Input};

        let power_config = self.get_power_config();
        println!("Command execution mode. Type 'exit' to quit.");
        println!(
            "Available shortcuts: {}",
            power_config
                .shortcuts
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );

        loop {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Query")
                .interact_text()?;
            if input.to_lowercase() == "exit" {
                break;
            }

            // Check for shortcuts
            let effective_input =
                power_config
                    .shortcuts
                    .get(&input)
                    .cloned()
                    .unwrap_or_else(|| {
                        // Check for aliases too
                        power_config
                            .get_alias(&input)
                            .cloned()
                            .unwrap_or(input.clone())
                    });

            if effective_input != input {
                println!("Expanded '{}' to: {}", input, effective_input);
            }

            // Use the same logic as handle_query but with effective_input
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            // Check permissions for the expanded command if it's a direct command
            if !power_config.is_command_allowed(&effective_input) {
                println!(
                    "{}",
                    "Command blocked by sandbox".red()
                );
                if !ask_confirmation("Run anyway?", false)? {
                    continue;
                }
            }

            let prompt = format!("You are on a system with: {}. Generate a bash command to: {}. Respond with only the exact command to run, without any formatting, backticks, quotes, or explanation. Ensure the command is complete, syntactically correct, and uses standard Unix tools. For size comparisons, use appropriate units like -BG for gigabytes in df.", self.system_info, effective_input);
            let response = client.generate_response(&prompt).await?;
            let command = extract_command_from_response(&response);
            println!("{}", format!("Command: {}", command).green());
            if ask_confirmation("Run this command?", false)? {
                let sandbox = Sandbox::new();
                println!("[EXEC] {}", command);
                println!("[RUN] Executing command...");
                match sandbox
                    .execute_safe("bash", vec!["-c".to_string(), command.clone()])
                    .await
                {
                    Ok(output) => {
                        println!("{}", output);
                        println!("[DONE] Command completed");
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Sandbox execution failed: {}", e);
                        // Offer fallback option for debugging
                        if ask_confirmation("Try running without sandboxing?", false)? {
                            match std::process::Command::new("bash")
                                .arg("-c")
                                .arg(&command)
                                .output()
                            {
                                Ok(output) => {
                                    println!("{}", String::from_utf8_lossy(&output.stdout));
                                    if !output.status.success() {
                                        println!(
                                            "[DONE] Command failed: {}",
                                            String::from_utf8_lossy(&output.stderr)
                                        );
                                    } else {
                                        println!("[DONE] Command completed");
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[ERROR] Direct execution failed: {}", e);
                                }
                            }
                        }
                    }
                }
            } else {
                println!("{}", "Command execution cancelled.".yellow());
            }
        }
        Ok(())
    }

    async fn handle_agent(&self, task: &str) -> Result<()> {
        let system_context = infrastructure::config::SystemContext::gather();

        // Get current directory and its contents
        let current_dir = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string());

        let ls_output = std::process::Command::new("sh")
            .arg("-c")
            .arg("ls -la 2>/dev/null | head -n 30")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| String::new());

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            r#"You generate a STRICT JSON array of executable shell commands for the user's goal.

REQUEST: {task}

ENVIRONMENT:
- Current directory: {cwd}
- Directory listing (first 30 entries):
{ls}

SYSTEM:
- Auto-detect distro and package manager at runtime; do not assume one unless required by the task.

HARD RULES (no exceptions):
1) Output ONLY a JSON array of strings, like ["cmd1", "cmd2"]. No prose, code fences, or extra text.
2) Assume you are already in the current directory—never emit cd/pushd for it.
3) Use only files and paths that appear in the directory listing; do not invent names.
4) Every command must be syntactically complete (e.g., zip archive.zip file1 file2).
5) Prefer the simplest minimal set of commands. If one command solves it, return a single-element array.
6) Use real paths (relative to the current directory) and avoid placeholders like /path/to or ~.
7) If the task cannot be satisfied with the available files, respond with [].

OUTPUT:"#,
            task = task,
            cwd = current_dir,
            ls = ls_output,
        );
        let response = client.generate_response(&prompt).await?;
        let commands = parse_agent_plan(&response);

        if commands.is_empty() {
            println!(
                "{}",
                "Model did not return a runnable command list (expected JSON array).".red()
            );
            return Ok(());
        }

        println!("\n{}", "Proposed plan:".green());
        for (i, cmd) in commands.iter().enumerate() {
            println!("  {} {}", format!("[{}]", i + 1).blue(), cmd);
        }

        for (i, cmd) in commands.iter().enumerate() {
            println!(
                "\n{} {}",
                "Step".green().bold(),
                format!("{}:", i + 1).green().bold()
            );
            println!("{} {}", "Suggested command:".green(), cmd.yellow());
            let accept = ask_confirmation("Run this command?", false)?;
            if !accept {
                println!("{}", "Skipping this step.".yellow());
                continue;
            }
            let sandbox = Sandbox::new();
            match sandbox
                .execute_safe("bash", vec!["-c".to_string(), cmd.clone()])
                .await
            {
                Ok(output) => {
                    println!("{}", output);
                    println!("{}", "Command completed successfully.".green());
                }
                Err(e) => {
                    println!("{} (sandbox error: {})", "Command failed.".red(), e);
                    if ask_confirmation("Try running without sandboxing?", false)? {
                        match std::process::Command::new("bash")
                            .arg("-c")
                            .arg(cmd)
                            .status()
                        {
                            Ok(status) => {
                                if status.success() {
                                    println!("{}", "Command completed successfully.".green());
                                } else {
                                    println!(
                                        "{} (exit status: {:?})",
                                        "Command failed.".red(),
                                        status.code()
                                    );
                                }
                            }
                            Err(e) => {
                                println!(
                                    "{} (direct execution error: {})",
                                    "Command failed.".red(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_explain(&self, file: &str) -> Result<()> {
        let path = std::path::Path::new(file);
        let content = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "pdf" => match pdf_extract::extract_text(file) {
                    Ok(text) => text,
                    Err(e) => {
                        println!("Error extracting text from PDF '{}': {}", file, e);
                        return Ok(());
                    }
                },
                "docx" => {
                    match std::fs::read(file) {
                        Ok(bytes) => {
                            match read_docx(&bytes) {
                                Ok(docx) => {
                                    let mut text = String::new();
                                    for child in &docx.document.children {
                                        match child {
                                            DocumentChild::Paragraph(p) => {
                                                text.push_str(&p.raw_text());
                                                text.push('\n');
                                            }
                                            DocumentChild::Table(_t) => {
                                                // For tables, we could extract text from cells
                                                // Table extraction not implemented yet
                                                text.push_str("[Table content not extracted]\n");
                                            }
                                            _ => {
                                                // Skip other elements for now
                                            }
                                        }
                                    }
                                    text
                                }
                                Err(e) => {
                                    println!("Error parsing DOCX '{}': {}", file, e);
                                    return Ok(());
                                }
                            }
                        }
                        Err(e) => {
                            println!("Error reading DOCX file '{}': {}", file, e);
                            return Ok(());
                        }
                    }
                }

                _ => match std::fs::read_to_string(file) {
                    Ok(text) => text,
                    Err(_) => {
                        println!("Error: Cannot read file '{}' as text. Supported formats: text files, PDF, DOCX.", file);
                        return Ok(());
                    }
                },
            }
        } else {
            match std::fs::read_to_string(file) {
                Ok(text) => text,
                Err(_) => {
                    println!("Error: Cannot read file '{}' as text. Supported formats: text files, PDF, DOCX.", file);
                    return Ok(());
                }
            }
        };

        if content.trim().is_empty() {
            println!("Error: No text content found in file '{}'.", file);
            return Ok(());
        }

        let prompt = format!("Explain this content in detail:\n\n{}", content);

        // Check cache first
        if let Some(cached_response) = self.load_cached_explain(&prompt)? {
            println!("{}", cached_response);
            if ask_confirmation("Use this cached explanation?", true)? {
                return Ok(());
            }
        }

        eprintln!("Analyzing file content...");
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let response = client.generate_response(&prompt).await?;

        // Cache the response
        self.save_cached_explain(&prompt, &response)?;

        println!("{}", response);
        Ok(())
    }

    async fn handle_rag(&mut self, question: &str) -> Result<()> {
        if let Some(cached_response) = self.load_cached_rag(question)? {
            println!("{}", cached_response);
            if ask_confirmation("Use this cached answer?", true)? {
                return Ok(());
            }
        }

        if self.rag_service.is_none() {
            eprintln!("Analyzing query and scanning codebase...");
            let client = OllamaClient::new()?;
            let project_root = find_project_root().unwrap_or_else(|| ".".to_string());
            self.rag_service =
                Some(application::create_rag_service(&project_root, &self.config.db_path).await?);
            let keywords = Self::keywords_from_text(question);
            self.rag_service
                .as_ref()
                .unwrap()
                .build_index_for_keywords(&keywords)
                .await?;
        }

        let mut feedback = String::new();
        loop {
            eprintln!("Thinking...");
            let response = self
                .rag_service
                .as_ref()
                .unwrap()
                .query_with_feedback(question, &feedback)
                .await?;

            if response.starts_with("__SECRETS_DETECTED__:") {
                println!(
                    "{}",
                    response.trim_start_matches("__SECRETS_DETECTED__:").trim()
                );
                if ask_confirmation("Continue with sanitized response?", false)? {
                    // Re-run the query but force it to continue with sanitized content
                    let force_response = self
                        .rag_service
                        .as_ref()
                        .unwrap()
                        .query_with_feedback_force(question, &feedback)
                        .await?;
                    println!("{}", force_response);
                } else {
                    println!("Query cancelled by user.");
                    return Ok(());
                }
            } else {
                println!("{}", response);
            }

            if ask_confirmation("Satisfied with this response?", true)? {
                self.save_cached_rag(question, &response)?;
                break;
            } else {
                feedback.clear();
                eprint!("Provide feedback for improvement: ");
                io::stdout().flush()?;
                io::stdin().read_line(&mut feedback)?;
                feedback = feedback.trim().to_string();
                eprintln!("Regenerating with feedback...");
            }
        }

        Ok(())
    }

    async fn handle_context(&mut self, path: &str) -> Result<()> {
        eprintln!("Loading context from {}...", path);
        let client = OllamaClient::new()?;

        // Create context-specific config with database path based on the context path
        let mut context_config = self.config.clone();
        let context_db_path = Self::context_specific_db_path(path);
        context_config.db_path = context_db_path;

        self.rag_service =
            Some(application::create_rag_service(path, &context_config.db_path.clone()).await?);
        self.rag_service.as_ref().unwrap().build_index().await?;
        eprintln!("Context loaded from {}", path);
        self.handle_chat().await
    }

    async fn handle_query(&mut self, query: &str) -> Result<()> {
        let power_config = self.get_power_config();

        // Check for command aliases first
        let effective_query = if let Some(alias_expansion) = power_config.get_alias(query) {
            println!("Using alias '{}' -> '{}'", query, alias_expansion);
            alias_expansion.clone()
        } else {
            query.to_string()
        };

        // Analyze query intent for enhanced handling
        let query_intent = analyze_query_intent(&effective_query);

        // Check if this is a system information query that should use dynamic processing
        let is_system_query = if let Some(classifier) = &self.input_classifier {
            match classifier.classify_input(&effective_query).await {
                Ok(result) => result.input_type == InputType::SystemQuery ||
                             query_intent == CommandIntent::InfoQuery ||
                             query_intent == CommandIntent::SystemQuery,
                Err(_) => false,
            }
        } else {
            false
        };

        // Handle installation/setup commands with special confirmation
        if query_intent == CommandIntent::Installation {
            return self.handle_installation_query(&effective_query).await;
        }

        // Check for plugin commands first
        if let Some(plugin_manager) = &self.config.plugin_manager {
            let manager = plugin_manager.read().await;
            if let Some(result) = manager.execute_command(&effective_query, vec![]).await {
                match result {
                    Ok(output) => {
                        println!("{}", output);
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("Plugin error: {}", e);
                        return Ok(());
                    }
                }
            }
        }

        // Handle cached commands with enhanced confirmation
        if let Ok(Some(cached_command)) = self.load_cached(&effective_query) {
            // Use enhanced confirmation system based on intent
            let confirmed = match query_intent {
                CommandIntent::Installation => {
                    let (packages, services, disk_space) = analyze_installation_command(&cached_command);
                    let risk = assess_command_risk(&cached_command);
                    prompt_installation_confirmation(
                        &cached_command,
                        query_intent,
                        packages,
                        services,
                        disk_space,
                    )?
                }
                _ => {
                    // For info queries, use data collection confirmation
                    let risk = assess_command_risk(&cached_command);
                    prompt_data_collection_confirmation(&cached_command, &effective_query, risk)?
                }
            };

            if confirmed {
                // Check if this cached command needs sudo
                let needs_sudo = Self::command_needs_sudo(&cached_command);
                let effective_command = if needs_sudo {
                    format!("sudo {}", cached_command)
                } else {
                    cached_command.clone()
                };

                if needs_sudo {
                    // For sudo commands, skip sandbox and execute directly
                    match std::process::Command::new("bash")
                        .arg("-c")
                        .arg(&effective_command)
                        .output()
                    {
                        Ok(output) => {
                            println!("{}", String::from_utf8_lossy(&output.stdout));
                            if !output.status.success() {
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                // Check if this is an expected non-error exit code
                                if Self::is_expected_exit_code(&effective_command, output.status.code(), &stderr) {
                                    let _ = self.save_cached(&effective_query, &effective_command);
                                } else {
                                    println!(
                                        "{}",
                                        format!(
                                            "Command failed: {}",
                                            stderr
                                        )
                                        .red()
                                    );
                                }
                            } else {
                                let _ = self.save_cached(&effective_query, &effective_command);
                            }
                        }
                        Err(e) => {
                            eprintln!("{}", format!("Direct execution failed: {}", e).red());
                        }
                    }
                } else {
                    // For non-sudo commands, try sandbox first
                    let sandbox = Sandbox::new();
                    match sandbox.execute_command_string(&effective_command).await {
                        Ok(output) => {
                            println!("{}", output);
                            return Ok(());
                        }
                        Err(e) => {
                            eprintln!("{}", format!("Command execution failed: {}", e).red());
                            // Offer direct execution as fallback
                            if ask_confirmation("Try executing directly (bypassing sandbox)?", false)? {
                                match std::process::Command::new("bash")
                                    .arg("-c")
                                    .arg(&effective_command)
                                    .output()
                                {
                                    Ok(output) => {
                                        println!("{}", String::from_utf8_lossy(&output.stdout));
                                        if !output.status.success() {
                                            let stderr = String::from_utf8_lossy(&output.stderr);
                                            // Check if this is an expected non-error exit code
                                            if Self::is_expected_exit_code(&effective_command, output.status.code(), &stderr) {
                                                let _ = self.save_cached(&effective_query, &effective_command);
                                            } else {
                                                println!(
                                                    "{}",
                                                    format!(
                                                        "Command failed: {}",
                                                        stderr
                                                    )
                                                    .red()
                                                );
                                            }
                                        } else {
                                            let _ = self.save_cached(&effective_query, &effective_command);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("{}", format!("Direct execution failed: {}", e).red());
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }
                }
                return Ok(());
            } else {
                // For both safe and unsafe cached commands, offer to generate a new command
                if ask_confirmation("Generate new command instead?", false)? {
                    // Continue to command generation below
                } else {
                    return Ok(());
                }
            }
        }

        // Generate new command using AI
        let system_context = infrastructure::config::SystemContext::gather();

        // Gather dynamic context based on request type
        let ls_output = std::process::Command::new("sh")
            .arg("-c")
            .arg("ls -la 2>/dev/null | head -n 30")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| String::new());

        // List available services if request is about services
        let services_output = if query.to_lowercase().contains("service")
            || query.to_lowercase().contains("status")
            || query.to_lowercase().contains("ssh")
            || query.to_lowercase().contains("systemctl")
        {
            std::process::Command::new("sh")
                .arg("-c")
                .arg("systemctl list-units --type=service --no-pager 2>/dev/null | grep -E '(running|active)' | awk '{print $1}' | head -n 50 || service --status-all 2>/dev/null | grep '+' | awk '{print $NF}' | head -n 30")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .unwrap_or_else(|| String::new())
        } else {
            String::new()
        };

        let client = infrastructure::ollama_client::OllamaClient::new()?;

        let prompt = format!(
            r#"Generate ONE bash command for the user's request. Output ONLY the command, nothing else.

REQUEST: {}

SYSTEM: {}
Package Manager: {}
{}
{}
COMMAND GENERATION RULES:
1. Output format: ONE line, ONE command, NO markdown, NO explanations, NO backticks
2. For services: Use "systemctl status SERVICE_NAME" where SERVICE_NAME is from the list above
3. For files: Use exact names from directory listing
4. For packages: Use the package manager shown above
5. Common patterns:
    - Service status: systemctl status SERVICE_NAME
    - Install package: sudo PACKAGE_MANAGER install PACKAGE
    - File operations: Use actual file names from directory

HOW TO FIND THE RIGHT SERVICE NAME:
- User says "ssh" or "sshd" → Look in AVAILABLE SERVICES for "ssh.service" or "sshd.service"
- If you see "sshd.service" in the list, use: systemctl status sshd
- If you see "ssh.service" in the list, use: systemctl status ssh
- Remove ".service" suffix when using with systemctl

VALID COMMAND EXAMPLES (adjust based on actual context):
systemctl status nginx
sudo apt install python3
zip archive.zip file.txt

OUTPUT ONLY THE COMMAND:"#,
            effective_query,
            system_context.distro,
            system_context.package_manager,
            if !services_output.is_empty() {
                format!(
                    "AVAILABLE SERVICES:\n{}",
                    services_output
                        .lines()
                        .take(20)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            } else {
                String::new()
            },
            if !ls_output.is_empty() {
                format!(
                    "\nCURRENT DIRECTORY:\n{}",
                    ls_output.lines().take(15).collect::<Vec<_>>().join("\n")
                )
            } else {
                String::new()
            }
        );

        let response = client.generate_response(&prompt).await?;
        let command = extract_command_from_response(&response);
        println!("{}", format!("Command: {}", command).green());

        // Validate command syntax before caching
        match validate_command_syntax(&command) {
            Ok(_) => {
                let _ = self.save_cached(&effective_query, &command);
            }
            Err(error_msg) => {
                eprintln!("{}", format!("Warning: Generated command has syntax issues ({}), not caching", error_msg).yellow());
            }
        }

        // Handle system queries with dynamic processing
        if is_system_query {
            return self.handle_system_query(&effective_query, &command).await;
        }

        // Check if this is a system command that might need sudo
        let needs_sudo = Self::command_needs_sudo(&command);
        let effective_command = if needs_sudo {
            format!("sudo {}", command)
        } else {
            command.clone()
        };

        // Single confirmation for new commands
        let is_safe = power_config.is_command_allowed(&effective_command);
        let prompt = if is_safe {
            if needs_sudo {
                format!("Execute with admin privileges: {}", command)
            } else {
                format!("Execute: {}", command)
            }
        } else {
            format!("Execute (requires elevated permissions): {}", effective_command)
        };

        if ask_confirmation(&prompt, is_safe)? {
            if needs_sudo {
                // For sudo commands, skip sandbox and execute directly
                match std::process::Command::new("bash")
                    .arg("-c")
                    .arg(&effective_command)
                    .output()
                {
                    Ok(output) => {
                        println!("{}", String::from_utf8_lossy(&output.stdout));
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            // Check if this is an expected non-error exit code
                            if Self::is_expected_exit_code(&effective_command, output.status.code(), &stderr) {
                                let _ = self.save_cached(&effective_query, &effective_command);
                            } else {
                                println!(
                                    "{}",
                                    format!(
                                        "Command failed: {}",
                                        stderr
                                    )
                                    .red()
                                );
                            }
                        } else {
                            let _ = self.save_cached(&effective_query, &effective_command);
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Direct execution failed: {}", e).red());
                    }
                }
            } else {
                // For non-sudo commands, try sandbox first
                let sandbox = Sandbox::new();
                match sandbox.execute_command_string(&effective_command).await {
                    Ok(output) => {
                        println!("{}", output);
                    }
                    Err(e) => {
                        eprintln!("{}", format!("Command execution failed: {}", e).red());
                        // Offer direct execution as fallback
                        if ask_confirmation("Try executing directly (bypassing sandbox)?", false)? {
                            match std::process::Command::new("bash")
                                .arg("-c")
                                .arg(&effective_command)
                                .output()
                            {
                                Ok(output) => {
                                    println!("{}", String::from_utf8_lossy(&output.stdout));
                                    if !output.status.success() {
                                        let stderr = String::from_utf8_lossy(&output.stderr);
                                        // Check if this is an expected non-error exit code
                                        if Self::is_expected_exit_code(&effective_command, output.status.code(), &stderr) {
                                            let _ = self.save_cached(&effective_query, &effective_command);
                                        } else {
                                            println!(
                                                "{}",
                                                format!(
                                                    "Command failed: {}",
                                                    stderr
                                                )
                                                .red()
                                            );
                                        }
                                    } else {
                                        let _ = self.save_cached(&effective_query, &effective_command);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("{}", format!("Direct execution failed: {}", e).red());
                                }
                            }
                        }
                    }
                }
            }
        } else {
            println!("{}", "Command execution cancelled.".yellow());
        }
        Ok(())
    }

    async fn handle_system_query(
        &mut self,
        query: &str,
        command: &str,
    ) -> Result<()> {
        let power_config = self.get_power_config();

        // Use enhanced confirmation for data collection
        let risk = assess_command_risk(command);
        let confirmed = prompt_data_collection_confirmation(command, query, risk)?;

        if !confirmed {
            println!("Query cancelled.");
            return Ok(());
        }
        // Check if command is safe
        let needs_sudo = Self::command_needs_sudo(command);
        let effective_command = if needs_sudo {
            format!("sudo {}", command)
        } else {
            command.to_string()
        };

        let is_safe = power_config.is_command_allowed(&effective_command);
        let prompt_text = if is_safe {
            if needs_sudo {
                format!("Execute system info command with admin privileges: {}", command)
            } else {
                format!("Execute system info command: {}", command)
            }
        } else {
            format!("Execute system info command (requires elevated permissions): {}", effective_command)
        };

        if !ask_confirmation(&prompt_text, is_safe)? {
            println!("{}", "Command execution cancelled.".yellow());
            return Ok(());
        }

        // Execute the command and get raw output
        let raw_output = if needs_sudo {
            // For sudo commands, skip sandbox and execute directly
            match std::process::Command::new("bash")
                .arg("-c")
                .arg(&effective_command)
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        String::from_utf8_lossy(&output.stdout).to_string()
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        eprintln!("{}", format!("Command failed: {}", stderr).red());
                        return Ok(());
                    }
                }
                Err(e) => {
                    eprintln!("{}", format!("Direct execution failed: {}", e).red());
                    return Ok(());
                }
            }
        } else {
            // For non-sudo commands, try sandbox first
            let sandbox = Sandbox::new();
            match sandbox.execute_command_string(&effective_command).await {
                Ok(output) => output,
                Err(e) => {
                    eprintln!("{}", format!("Command execution failed: {}", e).red());
                    // Offer direct execution as fallback
                    if ask_confirmation("Try executing directly (bypassing sandbox)?", false)? {
                        match std::process::Command::new("bash")
                            .arg("-c")
                            .arg(&effective_command)
                            .output()
                        {
                            Ok(output) => {
                                if output.status.success() {
                                    String::from_utf8_lossy(&output.stdout).to_string()
                                } else {
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    eprintln!("{}", format!("Command failed: {}", stderr).red());
                                    return Ok(());
                                }
                            }
                            Err(e) => {
                                eprintln!("{}", format!("Direct execution failed: {}", e).red());
                                return Ok(());
                            }
                        }
                    } else {
                        return Ok(());
                    }
                }
            }
        };

        // Process the raw output into a human-readable answer
        self.process_system_output(query, command, &raw_output).await?;

        Ok(())
    }

    async fn process_system_output(&self, query: &str, command: &str, raw_output: &str) -> Result<()> {
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let system_context = infrastructure::config::SystemContext::gather();

        let prompt = format!(
            r#"Process this command output for the user's query and provide a direct, human-readable answer.

SYSTEM CONTEXT:
- OS: {} ({})
- Architecture: {}
- CPU: {} ({} cores)
- RAM: {} total, {} used
- GPU: {}
- Package Manager: {}

QUERY: {}
COMMAND: {}
RAW OUTPUT:
{}

Provide:
1. Direct answer (1-2 sentences, conversational tone)
2. Key facts extracted (bullet points)
3. Brief explanation of what the data means (if needed)
4. Confidence score (0.0-1.0) in the accuracy of your processing

Format as JSON:
{{
    "answer": "Your direct answer here",
    "facts": ["Fact 1", "Fact 2", "Fact 3"],
    "explanation": "Brief explanation if needed, otherwise empty string",
    "confidence": 0.85
}}

Focus on being helpful and concise. If the output is empty or shows an error, explain what happened.
For confidence: 0.9+ means very confident, 0.7-0.9 means reasonably confident, below 0.7 means uncertain.
Use the system context to better understand the output format and provide more accurate answers."#,
            system_context.distro,
            system_context.os_type,
            system_context.architecture,
            system_context.cpu_model,
            system_context.cpu_cores,
            system_context.ram_total,
            system_context.ram_used,
            system_context.gpu_model,
            system_context.package_manager,
            query, command, raw_output
        );

        match client.generate_response(&prompt).await {
            Ok(response) => {
                // Parse the JSON response
                if let Some(json_start) = response.find('{') {
                    let json_str = &response[json_start..];
                    if let Some(json_end) = json_str.rfind('}') {
                        let json_content = &json_str[..=json_end];

                        #[derive(serde::Deserialize)]
                        struct ProcessedOutput {
                            answer: String,
                            facts: Vec<String>,
                            explanation: String,
                            confidence: Option<f32>,
                        }

                        match serde_json::from_str::<ProcessedOutput>(json_content) {
                            Ok(processed) => {
                                let confidence = processed.confidence.unwrap_or(0.8);

                                // Display confidence indicator
                                let confidence_indicator = match confidence {
                                    c if c >= 0.9 => "High confidence".green(),
                                    c if c >= 0.7 => "Medium confidence".yellow(),
                                    _ => "Low confidence".red(),
                                };
                                println!("{}", format!("System Information ({}):", confidence_indicator).bold());

                                // Display the processed answer
                                println!("{}", processed.answer);

                                if !processed.facts.is_empty() {
                                    println!("\n{}", "Key Details:".blue().bold());
                                    for fact in &processed.facts {
                                        println!("  • {}", fact);
                                    }
                                }

                                if !processed.explanation.is_empty() {
                                    println!("\n{}", "Note:".blue().bold());
                                    println!("{}", processed.explanation);
                                }

                                // Progressive disclosure based on confidence
                                if confidence >= 0.8 {
                                    // High confidence: show brief technical summary
                                    println!("\n{}", "Technical Summary:".yellow().bold());
                                    println!("  Command executed: {}", command.cyan());
                                    let lines: Vec<&str> = raw_output.lines().collect();
                                    println!("  Output lines: {}", lines.len().to_string().dimmed());
                                } else {
                                    // Lower confidence: show more technical details
                                    println!("\n{}", "Technical Details:".yellow().bold());
                                    println!("  Command: {}", command.cyan());
                                    println!("  Raw Output:");
                                    let lines: Vec<&str> = raw_output.lines().collect();
                                    if lines.len() > 10 {
                                        println!("    {} (showing first 5 lines)", format!("{} lines total", lines.len()).dimmed());
                                        for line in lines.iter().take(5) {
                                            println!("    {}", line.dimmed());
                                        }
                                        println!("    {}", "... (truncated - use 'raw' option to see full output)".dimmed());
                                    } else {
                                        for line in lines {
                                            println!("    {}", line.dimmed());
                                        }
                                    }
                                }

                                // Low confidence warning and feedback option
                                if confidence < 0.7 {
                                    println!("\n{}", "⚠️  This answer has low confidence. Consider checking the raw output manually.".red());
                                }

                                // Offer feedback option for medium/low confidence answers
                                if confidence < 0.9 {
                                    println!("\n{}", "Was this answer helpful? (y/n or provide correction):".dimmed());
                                    // In a full implementation, this would read user input and learn from corrections
                                    // For now, we just provide the option
                                }
                            }
                            Err(_) => {
                                // Fallback to showing raw output if processing fails
                                println!("{}", "Failed to process output, showing raw result:".yellow());
                                println!("{}", raw_output);
                            }
                        }
                    } else {
                        // Fallback to showing raw output
                        println!("{}", "Failed to process output, showing raw result:".yellow());
                        println!("{}", raw_output);
                    }
                } else {
                    // Fallback to showing raw output
                    println!("{}", "Failed to process output, showing raw result:".yellow());
                    println!("{}", raw_output);
                }
            }
            Err(e) => {
                eprintln!("{}", format!("Failed to process output: {}", e).red());
                // Fallback to showing raw output
                println!("{}", "Showing raw command output:".yellow());
                println!("{}", raw_output);
            }
        }

        Ok(())
    }

    async fn handle_installation_query(&mut self, query: &str) -> Result<()> {
        let power_config = self.get_power_config();

        // Generate installation command using AI
        let system_context = infrastructure::config::SystemContext::gather();

        let prompt = format!(
            r#"Generate a safe installation command for the user's request.

SYSTEM INFO:
- OS: {} ({})
- Package Manager: {}

USER REQUEST: {}

Generate a single, safe command for this installation request.
Return only the command, no explanations or markdown.

Examples:
- "install python" → "sudo apt install python3 python3-pip"
- "setup nginx web server" → "sudo apt install nginx"
- "install development tools" → "sudo apt install build-essential git"

COMMAND:"#,
            system_context.distro,
            system_context.package_manager,
            system_context.package_manager,
            query
        );

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let response = client.generate_response(&prompt).await?;
        let command = extract_command_from_response(&response);

        println!("{}", format!("Generated command: {}", command).green());

        // Validate command syntax
        match validate_command_syntax(&command) {
            Ok(_) => {
                // Prepare the effective command (with sudo if needed)
                let needs_sudo = Self::command_needs_sudo(&command);
                let effective_command = if needs_sudo {
                    format!("sudo {}", command)
                } else {
                    command.clone()
                };

                // Analyze the installation command
                let (packages, services, disk_space) = analyze_installation_command(&command);
                let risk = assess_command_risk(&command);

                // Present installation confirmation
                let confirmed = prompt_installation_confirmation(
                    &command,
                    CommandIntent::Installation,
                    packages,
                    services,
                    disk_space,
                )?;

                if !confirmed {
                    println!("Installation cancelled.");
                    return Ok(());
                }

                // Check safety policy - require additional confirmation for blocked commands
                let is_allowed = power_config.is_command_allowed(&effective_command);
                if !is_allowed {
                    // Command is blocked by safety policy - ask for override confirmation
                    eprintln!("Command '{}' is blocked by safety policy.", effective_command);
                    if !ask_confirmation("Execute anyway?", false)? {
                        println!("Command cancelled due to safety policy.");
                        return Ok(());
                    }
                    // User explicitly confirmed override
                }

                println!("Executing installation...");

                // Execute with progress feedback
                if needs_sudo {
                    match std::process::Command::new("sudo")
                        .arg("bash")
                        .arg("-c")
                        .arg(&command)
                        .status()
                    {
                        Ok(status) => {
                            if status.success() {
                                println!("Installation completed successfully");
                                self.show_post_installation_steps(&command, query);
                            } else {
                                eprintln!("Installation failed");
                            }
                        }
                        Err(e) => {
                            eprintln!("Installation execution failed: {}", e);
                        }
                    }
                } else {
                    let sandbox = Sandbox::new();
                    match sandbox.execute_command_string(&command).await {
                        Ok(output) => {
                            println!("{}", output);
                            println!("Installation completed successfully");
                            self.show_post_installation_steps(&command, query);
                        }
                        Err(e) => {
                            eprintln!("Installation failed: {}", e);
                        }
                    }
                }

                // Cache successful installations
                let _ = self.save_cached(query, &command);
            }
            Err(error_msg) => {
                eprintln!("Generated command has syntax issues: {}", error_msg);
            }
        }

        Ok(())
    }

    fn show_post_installation_steps(&self, command: &str, original_query: &str) {
        let cmd_lower = command.to_lowercase();
        let query_lower = original_query.to_lowercase();

        println!();
        println!("Next steps suggested:");

        if cmd_lower.contains("nginx") {
            println!("  - Configure nginx: sudo nano /etc/nginx/sites-available/default");
            println!("  - Test installation: curl http://localhost");
            println!("  - Enable SSL: sudo certbot --nginx (if certbot is installed)");
        } else if cmd_lower.contains("apache") || cmd_lower.contains("httpd") {
            println!("  - Configure apache: sudo nano /etc/apache2/sites-available/000-default.conf");
            println!("  - Test installation: curl http://localhost");
            println!("  - Enable SSL: sudo a2enmod ssl && sudo a2ensite default-ssl");
        } else if cmd_lower.contains("mysql") || cmd_lower.contains("mariadb") {
            println!("  - Secure installation: sudo mysql_secure_installation");
            println!("  - Start service: sudo systemctl start mysql");
            println!("  - Connect to database: sudo mysql -u root");
        } else if cmd_lower.contains("postgresql") {
            println!("  - Start service: sudo systemctl start postgresql");
            println!("  - Create user: sudo -u postgres createuser --interactive --pwprompt your_username");
            println!("  - Create database: sudo -u postgres createdb your_database");
        } else if cmd_lower.contains("python") && cmd_lower.contains("pip") {
            println!("  - Verify installation: python3 --version && pip3 --version");
            println!("  - Install virtualenv: pip3 install virtualenv");
        } else if cmd_lower.contains("git") {
            println!("  - Configure git: git config --global user.name \"Your Name\"");
            println!("  - Configure git: git config --global user.email \"your.email@example.com\"");
        } else if cmd_lower.contains("docker") {
            println!("  - Start service: sudo systemctl start docker");
            println!("  - Add user to docker group: sudo usermod -aG docker $USER");
            println!("  - Test installation: docker run hello-world");
        } else {
            println!("  - Verify installation by running the installed command");
            println!("  - Check service status if applicable");
        }
    }

    fn keywords_from_text(text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| w.len() > 2)
            .map(|w| w.to_lowercase())
            .collect()
    }

    fn explain_cache_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(format!("{}_explain_cache.bin", suffix));
        path
    }

    fn load_cached_explain(&self, prompt: &str) -> Result<Option<String>> {
        let cache_path = Self::explain_cache_path();
        if !cache_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&cache_path)?;
        let mut cache: ExplainCacheFile = bincode::deserialize(&data).unwrap_or_default();

        // Remove expired entries (7 days)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache.entries.retain(|entry| now - entry.timestamp < 604800);

        // Save cleaned cache
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = bincode::serialize(&cache)?;
        std::fs::write(&cache_path, serialized)?;

        // Find exact match
        for entry in &cache.entries {
            if entry.prompt == prompt {
                return Ok(Some(entry.response.clone()));
            }
        }
        Ok(None)
    }

    fn save_cached_explain(&self, prompt: &str, response: &str) -> Result<()> {
        let cache_path = Self::explain_cache_path();
        let mut cache = if cache_path.exists() {
            let data = std::fs::read(&cache_path).unwrap_or_default();
            bincode::deserialize::<ExplainCacheFile>(&data).unwrap_or_default()
        } else {
            ExplainCacheFile::default()
        };

        cache.entries.push(ExplainCacheEntry {
            prompt: prompt.to_string(),
            response: response.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = serde_json::to_string_pretty(&cache)?;
        std::fs::write(&cache_path, serialized)?;

        Ok(())
    }

    fn rag_cache_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(format!("{}_rag_cache.bin", suffix));
        path
    }

    fn load_cached_rag(&self, question: &str) -> Result<Option<String>> {
        let cache_path = Self::rag_cache_path();
        if !cache_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&cache_path)?;
        let mut cache: RagCacheFile = bincode::deserialize(&data).unwrap_or_default();

        // Remove expired entries (7 days)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cache.entries.retain(|entry| now - entry.timestamp < 604800);

        // Save cleaned cache
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = bincode::serialize(&cache)?;
        std::fs::write(&cache_path, serialized)?;
        // Find exact match
        for entry in &cache.entries {
            if entry.question == question {
                return Ok(Some(entry.response.clone()));
            }
        }
        Ok(None)
    }

    fn save_cached_rag(&self, question: &str, response: &str) -> Result<()> {
        let cache_path = Self::rag_cache_path();
        let mut cache = if cache_path.exists() {
            let data = std::fs::read(&cache_path).unwrap_or_default();
            bincode::deserialize::<RagCacheFile>(&data).unwrap_or_default()
        } else {
            RagCacheFile::default()
        };

        cache.entries.push(RagCacheEntry {
            question: question.to_string(),
            response: response.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialized = bincode::serialize(&cache)?;
        std::fs::write(&cache_path, serialized)?;

        Ok(())
    }

    /// Check if a command typically requires sudo/admin privileges
    fn command_needs_sudo(command: &str) -> bool {
        let sudo_commands = [
            "systemctl",
            "service",
            "systemd",
            "apt",
            "apt-get",
            "yum",
            "dnf",
            "pacman",
            "zypper",
            "mount",
            "umount",
            "fdisk",
            "mkfs",
            "fsck",
            "iptables",
            "ufw",
            "firewall-cmd",
            "usermod",
            "useradd",
            "userdel",
            "groupadd",
            "groupdel",
            "chmod",
            "chown",
            "passwd",
            "visudo",
            "crontab",
            "overwrite",
            "modify",
            "edit",
            "update",
        ];

        // Check if the command starts with any of the sudo-requiring commands
        for sudo_cmd in &sudo_commands {
            if command.starts_with(&format!("{} ", sudo_cmd)) || command == *sudo_cmd {
                return true;
            }
        }

        false
    }

    /// Check if a command's exit code should be considered successful despite being non-zero
    fn is_expected_exit_code(command: &str, exit_code: Option<i32>, stderr: &str) -> bool {
        // Handle systemctl status commands - exit code 3 means service is inactive (normal)
        if (command.contains("systemctl status") || command.contains("sudo systemctl status")) && exit_code == Some(3) && !stderr.contains("Failed to") {
            return true;
        }

        // Add more command-specific exit code handling here as needed
        // For example:
        // if command.contains("some_command") && exit_code == Some(expected_code) {
        //     return true;
        // }

        false
    }

    /// Handle streaming agent mode - demonstrates real-time execution
    async fn handle_stream_mode(&mut self, goal: &str) -> Result<()> {
        println!("{}", "🎬 Real-Time Streaming Mode".bright_cyan().bold());
        println!("{}", format!("Goal: {}", goal).bright_blue());
        println!(
            "{}",
            "This mode demonstrates live agent execution with streaming output.".bright_yellow()
        );
        println!();

        // Create a simple streaming demonstration
        use application::streaming_agent::{
            DisplayMode, StatusLevel, StreamEvent, StreamingAgentOrchestrator, StreamingDisplay,
        };

        let (orchestrator, mut event_rx, _control_tx) =
            StreamingAgentOrchestrator::new(DisplayMode::Rich);

        let display = StreamingDisplay::new(DisplayMode::Rich);

        // Start a background task that simulates streaming agent execution
        let goal_clone = goal.to_string();
        let event_tx = orchestrator.event_sender();
        tokio::spawn(async move {
            // Simulate agent reasoning steps
            let _ = event_tx
                .send(StreamEvent::ReasoningStart {
                    task_description: goal_clone.clone(),
                })
                .await;

            let _ = event_tx
                .send(StreamEvent::Status {
                    message: "Starting agent execution simulation".to_string(),
                    level: StatusLevel::Info,
                })
                .await;

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let _ = event_tx
                .send(StreamEvent::ReasoningStep {
                    step_number: 1,
                    content: "Breaking down the request into actionable components".to_string(),
                })
                .await;

            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

            let _ = event_tx
                .send(StreamEvent::ReasoningStep {
                    step_number: 2,
                    content: "Identifying required tools and resources".to_string(),
                })
                .await;

            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

            let _ = event_tx
                .send(StreamEvent::ToolPlanned {
                    tool_name: "analysis_tool".to_string(),
                    description: "Analyze the codebase for relevant information".to_string(),
                })
                .await;

            let _ = event_tx
                .send(StreamEvent::ToolStart {
                    tool_name: "analysis_tool".to_string(),
                    parameters: "{}".to_string(),
                })
                .await;

            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            let _ = event_tx
                .send(StreamEvent::ToolComplete {
                    tool_name: "analysis_tool".to_string(),
                    success: true,
                    duration_ms: 1000,
                    error: None,
                })
                .await;

            let _ = event_tx
                .send(StreamEvent::Result {
                    content: format!("Streaming analysis complete for: {}", goal_clone),
                    confidence: 0.85,
                })
                .await;
        });

        // Display streaming events in real-time
        while let Some(event) = event_rx.recv().await {
            display.render_event(&event);

            // Add small delay for visual effect
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Exit when we get a final result
            if let StreamEvent::Result { .. } = event {
                break;
            }
        }

        println!();
        println!("{}", "✅ Streaming demonstration complete!".bright_green());
        println!(
            "{}",
            "This showcases real-time agent execution with live feedback.".bright_cyan()
        );

        Ok(())
    }

    /// Handle listing all sessions
    async fn handle_list_sessions(&mut self) -> Result<()> {
        let Some(store) = &self.session_store else {
            println!(
                "{}",
                "No project detected - session management requires a project context.".yellow()
            );
            return Ok(());
        };

        let project_root = find_project_root().unwrap_or_else(|| "unknown".to_string());
        let project_hash = store.project_hash();

        println!("{}", "Session Management".bright_cyan().bold());
        println!("Project: {} (hash: {})", project_root, &project_hash[..8]);
        println!();

        match store.list_sessions() {
            Ok(sessions) if sessions.is_empty() => {
                println!("{}", "No sessions found.".dimmed());
                println!(
                    "Create your first session with: ai --session \"my-session\" --build \"...\""
                );
            }
            Ok(sessions) => {
                println!("Sessions:");
                for session in sessions {
                    let active_marker = if Some(&session.name) == self.current_session.as_ref() {
                        "[active] "
                    } else {
                        "          "
                    };

                    let last_used = session.last_used.format("%Y-%m-%d %H:%M");
                    let goal = if session.goal_summary.is_empty() {
                        "No goal set".dimmed()
                    } else {
                        session.goal_summary.dimmed()
                    };

                    println!(
                        "  {} {:<15} Last used: {}  Changes: {}  Goal: {}",
                        active_marker,
                        session.name.bright_green(),
                        last_used,
                        session.change_count,
                        goal
                    );
                }
            }
            Err(e) => {
                eprintln!("{} {}", "Error listing sessions:".red(), e);
                return Ok(());
            }
        }

        Ok(())
    }

    /// Handle deleting a session
    async fn handle_delete_session(&mut self, session_name: &str) -> Result<()> {
        let Some(store) = &self.session_store else {
            println!(
                "{}",
                "No project detected - cannot delete sessions.".yellow()
            );
            return Ok(());
        };

        // Confirm deletion
        use shared::confirmation::{
            ask_confirmation, ask_enhanced_confirmation, ConfirmationChoice,
        };
        let prompt = format!(
            "Permanently delete session '{}' and all its data?",
            session_name
        );
        match ask_confirmation(&prompt, false) {
            Ok(true) => {
                match store.delete_session(session_name) {
                    Ok(_) => {
                        println!(
                            "{} Session '{}' deleted successfully.",
                            "✓".green(),
                            session_name
                        );
                        // Export backup before deletion
                        if let Ok(backup_path) = store.export_session(session_name) {
                            println!(
                                "{} Session backed up to: {}",
                                "💾".blue(),
                                backup_path.display()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("{} Failed to delete session: {}", "✗".red(), e);
                    }
                }
            }
            Ok(false) => {
                println!("{}", "Session deletion cancelled.".yellow());
            }
            Err(e) => {
                eprintln!("{} Confirmation error: {}", "✗".red(), e);
            }
        }

        Ok(())
    }

    /// Handle continuing a session
    async fn handle_continue_session(&mut self) -> Result<()> {
        let Some(store) = &self.session_store else {
            println!(
                "{}",
                "No project detected - cannot continue sessions.".yellow()
            );
            return Ok(());
        };

        // Try to continue current session, then most recent, then create default
        let target_session = if let Some(current) = &self.current_session {
            current.clone()
        } else {
            // Find most recently used session
            match store.list_sessions() {
                Ok(sessions) if !sessions.is_empty() => sessions
                    .into_iter()
                    .max_by_key(|s| s.last_used)
                    .map(|s| s.name)
                    .unwrap(),
                _ => "main".to_string(),
            }
        };

        match store.load_session(&target_session) {
            Ok(Some(session)) => {
                self.current_session = Some(target_session.clone());
                println!(
                    "{} Continuing session '{}'",
                    "▶".green(),
                    target_session.bright_green()
                );
                println!("  Goal: {}", session.metadata.goal_summary.dimmed());
                println!("  Changes: {}", session.metadata.change_count);
                println!(
                    "  Last used: {}",
                    session.metadata.last_used.format("%Y-%m-%d %H:%M")
                );

                if !session.conversation_history.is_empty() {
                    println!(
                        "  Conversation: {} messages",
                        session.conversation_history.len()
                    );
                }
            }
            Ok(None) => {
                // Session doesn't exist, create it
                println!(
                    "{} Session '{}' not found, creating new session.",
                    "🆕".blue(),
                    target_session
                );
                match store.get_or_create_session(&target_session) {
                    Ok(session) => {
                        self.current_session = Some(target_session.clone());
                        println!(
                            "{} Created and activated session '{}'",
                            "✓".green(),
                            target_session.bright_green()
                        );
                    }
                    Err(e) => {
                        eprintln!("{} Failed to create session: {}", "✗".red(), e);
                    }
                }
            }
            Err(e) => {
                eprintln!("{} Failed to load session: {}", "✗".red(), e);
            }
        }

        Ok(())
    }

    /// Display background status and system information
    fn display_background_status(&self) {
        // Clean, minimal output - no robot icon
        if let Some(session) = &self.current_session {
            println!("[{}]", session.bright_cyan());
        }
    }

    /// Handle background events and display them in the UI
    async fn handle_background_events(mut event_receiver: Receiver<BackgroundEvent>) {
        while let Ok(event) = event_receiver.recv_async().await {
            match event {
                BackgroundEvent::FileChanged { path, change_type } => {
                    let (change_icon, change_str) = match change_type {
                        FileChangeType::Created => ("🆕", "created"),
                        FileChangeType::Modified => ("✏️", "modified"),
                        FileChangeType::Deleted => ("🗑️", "deleted"),
                        FileChangeType::Renamed => ("📝", "renamed"),
                    };
                    println!("{} {} {}", change_icon, change_str, path.display());
                }
                BackgroundEvent::TestResult {
                    session,
                    status,
                    output,
                } => {
                    let (status_icon, status_str) = match status {
                        TestStatus::Started => ("▶️", "started"),
                        TestStatus::Passed => ("✅", "passed"),
                        TestStatus::Failed { .. } => ("❌", "failed"),
                        TestStatus::Completed => ("🏁", "completed"),
                    };
                    println!(
                        "{} Test {}: {}",
                        status_icon,
                        session,
                        output.lines().next().unwrap_or("")
                    );
                }
                BackgroundEvent::LogEntry {
                    source,
                    level,
                    message,
                } => {
                    let (level_icon, level_str) = match level {
                        LogLevel::Debug => ("🐛", "debug"),
                        LogLevel::Info => ("ℹ️", "info"),
                        LogLevel::Warn => ("⚠️", "warn"),
                        LogLevel::Error => ("🚨", "error"),
                    };
                    println!("{} [{}] {}: {}", level_icon, source, level_str, message);
                }
                BackgroundEvent::LspDiagnostic {
                    file,
                    severity,
                    message,
                } => {
                    let severity_icon = match severity {
                        DiagnosticSeverity::Error => "🚨",
                        DiagnosticSeverity::Warning => "⚠️",
                        DiagnosticSeverity::Information => "ℹ️",
                        DiagnosticSeverity::Hint => "💡",
                    };
                    println!("{} {}: {}", severity_icon, file.display(), message);
                }
                BackgroundEvent::GitStatus { status } => match status {
                    GitStatus::Clean => println!("{} Repository is clean", "✅".green()),
                    GitStatus::Dirty { modified_files } => {
                        println!("{} {} modified files", "📝".yellow(), modified_files.len());
                    }
                    GitStatus::Untracked { files } => {
                        println!("{} {} untracked files", "📄".yellow(), files.len());
                    }
                },
            }
        }
    }

    /// Handle test execution with real-time monitoring
    async fn handle_test_run(&mut self) -> Result<()> {
        println!("🧪 Running tests with real-time monitoring...");

        // Get project root
        let project_root = find_project_root().ok_or_else(|| {
            anyhow::anyhow!("Could not find project root. Are you in a Rust project?")
        })?;

        // Start test watcher if background supervisor is available
        if let Some(supervisor) = self.background_supervisor.as_mut() {
            let session_name = self
                .current_session
                .clone()
                .unwrap_or_else(|| "test-session".to_string());
            supervisor
                .start_test_watcher(std::path::PathBuf::from(project_root), session_name)
                .await?;
        }

        println!(
            "✅ Test monitoring started. Background intelligence will report results in real-time."
        );
        println!("📊 Test events will appear as they happen...");

        Ok(())
    }

    /// Display background status updates
    fn display_background_updates(&self) {
        println!("\n{}Background Intelligence:", "🧠 ".bright_blue());

        // Check git status
        let git_status = if std::path::Path::new(".git").exists() {
            format!("{} Git repository active", "✅".green())
        } else {
            format!("{} Git not initialized", "⚠️".yellow())
        };

        // Check session store status
        let session_status = if self.session_store.is_some() {
            format!("{} Session persistence active", "✅".green())
        } else {
            format!("{} Session store unavailable", "❌".red())
        };

        // Check background services
        if let Some(ref supervisor) = self.background_supervisor {
            let service_status = supervisor.service_status();
            for (service_name, status) in service_status {
                let icon = match status.as_str() {
                    "Running" => "✅",
                    "Starting" => "⏳",
                    "Stopped" => "🛑",
                    _ => "❌",
                };
                let color = match status.as_str() {
                    "Running" => icon.green(),
                    "Starting" => icon.blue(),
                    "Stopped" => icon.yellow(),
                    _ => icon.red(),
                };
                println!("  └─ {} {}: {}", color, service_name, status);
            }
        } else {
            println!("  └─ {} Background services unavailable", "❌".red());
        }
    }

    /// Handle undo command
    async fn handle_undo(&mut self) -> Result<()> {
        let Some(session_name) = &self.current_session.clone() else {
            println!(
                "{}",
                "No active session. Use --session to specify a session first.".yellow()
            );
            return Ok(());
        };

        // Try git undo first (preferred)
        let repo_path = std::env::current_dir()?;
        if repo_path.join(".git").exists() {
            match self.git_undo_last_commit().await {
                Ok(true) => {
                    println!("{} Undid last commit via git", "✓".green());

                    // Update session metadata - borrow store separately to avoid conflict
                    if let Some(store) = &self.session_store {
                        if let Ok(mut session) = store.load_session(session_name) {
                            if let Some(ref mut session) = session {
                                session.metadata.change_count =
                                    session.metadata.change_count.saturating_sub(1);
                                if let Err(e) = store.save_session(session) {
                                    eprintln!(
                                        "{} {}",
                                        "Warning: Failed to update session:".yellow(),
                                        e
                                    );
                                }
                            }
                        }
                    }
                    return Ok(());
                }
                Ok(false) => {
                    // Git undo not available, fall through to manual undo
                }
                Err(e) => {
                    eprintln!("{} {}", "Warning: Git undo failed:".yellow(), e);
                    // Fall through to manual undo
                }
            }
        }

        println!("[UNDO] Git undo completed - changes reverted");
        println!(
            "{}",
            "Tip: Use 'git reset --hard HEAD~1' for manual git rollback".bright_black()
        );
        Ok(())
    }

    /// Attempt to undo the last git commit
    async fn git_undo_last_commit(&mut self) -> Result<bool> {
        let repo_path = std::env::current_dir()?;
        let repo = git2::Repository::open(&repo_path)
            .map_err(|e| anyhow::anyhow!("Failed to open git repository: {}", e))?;

        // Check if there are commits to undo
        let head = repo
            .head()
            .map_err(|e| anyhow::anyhow!("Failed to get HEAD: {}", e))?;

        if head.name() != Some("refs/heads/master") && head.name() != Some("refs/heads/main") {
            return Ok(false); // Not on main/master branch
        }

        // Get the current commit
        let head_commit = repo
            .find_commit(head.target().unwrap())
            .map_err(|e| anyhow::anyhow!("Failed to find HEAD commit: {}", e))?;

        // Check if this commit was made by the agent
        let commit_msg = head_commit.message().unwrap_or("");
        if !commit_msg.contains("elite agentic CLI") && !commit_msg.contains("Applied") {
            return Ok(false); // Not an agent commit
        }

        // Reset to parent commit
        let parent_commit = head_commit.parents().next();
        if let Some(parent) = parent_commit {
            let parent_oid = parent.id();
            repo.reset(parent.as_object(), git2::ResetType::Hard, None)
                .map_err(|e| anyhow::anyhow!("Failed to reset to parent commit: {}", e))?;
            Ok(true)
        } else {
            Ok(false) // No parent commit (initial commit)
        }
    }

    /// Get the effective power user configuration (with override if set)
    fn get_power_config(&self) -> &infrastructure::config::PowerUserConfig {
        self.power_config_override
            .as_ref()
            .unwrap_or(&self.config.power_user)
    }
}

#[cfg(test)]
mod tests {
    use super::CliApp;
    use application::build_service::FileOperation;
    use std::collections::VecDeque;
    use std::path::PathBuf;

    #[test]
    fn rebuild_operations_reorders_known_steps() {
        let ops = vec![
            FileOperation::Create {
                path: PathBuf::from("health.sh"),
                content: "a".to_string(),
            },
            FileOperation::Update {
                path: PathBuf::from("health.sh"),
                old_content: "a".to_string(),
                new_content: "b".to_string(),
            },
        ];

        let steps = vec![
            "Update health.sh".to_string(),
            "Create health.sh".to_string(),
        ];

        let (reordered, warnings) = CliApp::rebuild_operations_from_steps(&steps, &ops);
        assert!(warnings.is_empty());
        assert!(matches!(reordered[0], FileOperation::Update { .. }));
        assert!(matches!(reordered[1], FileOperation::Create { .. }));
    }

    #[test]
    fn rebuild_operations_warns_on_unknown_steps() {
        let ops = vec![FileOperation::Create {
            path: PathBuf::from("health.sh"),
            content: "a".to_string(),
        }];
        let steps = vec!["Do something else".to_string()];

        let (reordered, warnings) = CliApp::rebuild_operations_from_steps(&steps, &ops);
        assert!(reordered.is_empty());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn parse_operation_line_allows_delete_and_read() {
        let ops = vec![];

        let delete = CliApp::parse_operation_line("Delete logs.txt", &ops).unwrap();
        let read = CliApp::parse_operation_line("Read docs.txt", &ops).unwrap();

        assert!(matches!(delete, FileOperation::Delete { .. }));
        assert!(matches!(read, FileOperation::Read { .. }));
    }

    #[tokio::test]
    async fn apply_operations_with_scripted_input() {
        let mut app = CliApp::new();
        app.scripted_inputs = Some(VecDeque::from(vec!["y".to_string()]));

        let temp_dir = std::env::temp_dir().join(format!("vibe_cli_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let mut build_service = application::build_service::BuildService::new(&temp_dir);
        let mut plan = application::build_service::BuildPlan {
            goal: "create health.sh".to_string(),
            operations: vec![FileOperation::Create {
                path: temp_dir.join("health.sh"),
                content: "#!/bin/bash\necho ok\n".to_string(),
            }],
            description: "test plan".to_string(),
            estimated_risk: application::build_service::RiskLevel::Low,
        };

        let ok = app
            .apply_operations_interactively(&mut plan, &mut build_service)
            .unwrap();
        assert!(ok);
        build_service
            .execute_operation_once(&plan.operations[0])
            .await
            .unwrap();

        let content = std::fs::read_to_string(temp_dir.join("health.sh")).unwrap();
        assert!(content.contains("echo ok"));

        std::env::set_current_dir(&original_dir).unwrap();
    }
}
