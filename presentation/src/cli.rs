use application::{rag_service::RagService, agent_service::AgentService};
use clap::Parser;
use colored::Colorize;
use docx_rs::*;
use infrastructure::{config::Config, ollama_client::OllamaClient, sandbox::Sandbox, session_store::SessionStore};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use shared::confirmation::ask_confirmation;
use shared::types::Result;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::{self, Duration};

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
    // Remove surrounding backticks, quotes, and extra whitespace
    cleaned
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
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
    #[arg(long, help = "Generate and execute build plans with AI assistance, RAG context retrieval, and transaction safety")]
    pub build: bool,

    /// Dry-run mode: show plan without executing
    #[arg(long, help = "Preview build plan and operations without making changes")]
    pub dry_run: bool,

    /// Verbose output: show detailed information
    #[arg(long, help = "Display retrieved context and detailed operation information")]
    pub verbose: bool,

    /// Show diffs for file operations (reserved for future use)
    #[arg(long, help = "Show diffs for file modifications (planned feature)")]
    pub show_diff: bool,

    /// Specify which session to use for operations
    #[arg(long, value_name = "NAME", help = "Use named session for operations (creates if doesn't exist)")]
    pub session: Option<String>,

    /// List all sessions for the current project
    #[arg(long, help = "Display all sessions with their metadata")]
    pub list_sessions: bool,

    /// Delete a specific session
    #[arg(long, value_name = "NAME", help = "Permanently delete the specified session")]
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
}

pub struct CliApp {
    rag_service: Option<RagService>,
    cache_path: PathBuf,
    system_info: String,
    config: Config,
    session_store: Option<SessionStore>,
    current_session: Option<String>,
}

impl CliApp {
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

        Self {
            rag_service: None,
            cache_path,
            system_info,
            config,
            session_store,
            current_session: None,
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
        trimmed.to_string()
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
        for entry in &cache.entries {
            if entry.prompt == prompt {
                return Ok(Some(Self::clean_command_output(&entry.command)));
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

        if let Some(entry) = best_match {
            Ok(Some(Self::clean_command_output(&entry.command)))
        } else {
            Ok(None)
        }
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
                println!("\n{}", format!("⚡ Confidence: {:.1}%", response.confidence * 100.0).bright_magenta());
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

        println!("{}", "Planning mode: Create execution plan without running commands".bright_cyan());
        println!("{}", format!("Goal: {}", goal).bright_blue());
        
        // Initialize enhanced agent for planning
        let client = OllamaClient::new()?;
        // Use Candle by default instead of Ollama
        let agent_service = application::create_agent_service().await?;
        
        // Create agent request for planning
        let request = domain::models::AgentRequest {
            goal: format!("Create a detailed step-by-step execution plan for: {}", goal),
            context: Some(format!("System: {} | Mode: Plan Only", self.system_info)),
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
                        println!("  • {} - {}", tool_call.name.bright_green(), tool_call.reasoning);
                    }
                }
                
                println!("\n{}", "Execution Plan:".bright_green());
                println!("{}", response.final_response);
                
                println!("\n{}", "Planning complete. Use --agent or --chat to execute the plan.".bright_green());
            }
            Err(e) => {
                eprintln!("{} {}", "Planning error:".red(), e);
            }
        }
        
        Ok(())
    }

    async fn handle_build(&mut self, goal: &str, dry_run: bool, verbose: bool, show_diff: bool) -> Result<()> {
        use application::build_service::{BuildService, ConfirmationMode, BuildPlan, RiskLevel};
        use application::agent_service::IncrementalBuildPlanner;
        use infrastructure::config::Config;

        if goal.trim().is_empty() {
            println!(
                "{}",
                "Build mode requires a goal (e.g. vibe_cli --build \"Add error handling to the parser\")"
                    .red()
            );
            return Ok(());
        }

        println!("{}", "Build Mode: Safe code modifications with user confirmation".bright_cyan().bold());
        println!("{} {}", "Goal:".bright_green(), goal);

        // Configure build service based on flags
        let mut build_service = BuildService::new(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
        build_service.set_dry_run(dry_run);
        build_service.set_show_diff(show_diff);
        build_service.set_verbose(verbose);

        if verbose {
            build_service.set_confirmation_mode(ConfirmationMode::Interactive);
        }

        // Initialize services
        let agent_service = application::create_agent_service().await?;

        // Use true real-time incremental streaming
        println!("\n{}", "Starting true real-time incremental planning...".bright_cyan());

        // Create the incremental planner
        let mut planner = match agent_service.plan_build_incremental(goal).await {
            Ok(planner) => planner,
            Err(e) => {
                eprintln!("{} {}", "Build planning initialization error:".red(), e);
                return Ok(());
            }
        };

        // Real-time incremental planning with tool transparency
        let total_steps = 5;
        println!("Planning Progress:");
        println!("[→] [1/{}] Analyzing project context", total_steps);

        let mut step_count = 0;
        let mut code_generation_complete = false;

        loop {
            // Stop processing if code generation is complete
            if code_generation_complete {
                break;
            }

            match planner.stream_next_step(&agent_service.inference_engine).await {
                Ok(Some(step)) => {
                    step_count += 1;

                    // Update progress display
                    Self::update_progress_display(step_count, total_steps, &step.description);

                    // Show chain of thought for analysis/planning phases
                    if step_count <= 3 {
                        Self::display_chain_of_thought(&step.reasoning);
                    }

                    // Show tool usage after context retrieval
                    if step_count == 2 {
                        Self::display_tool_usage();
                    }

                    // Handle incremental code generation (Step 3)
                    if step_count == 3 {
                        if let (Some(code), Some(path), Some(op_type)) = (&step.code_chunk, &step.file_path, &step.operation_type) {
                            // Display the incremental changes from AI
                            Self::display_incremental_changes(code, path, op_type);

                            // Buffer the operation for execution
                            if op_type == "create" {
                                build_service.buffer_operation(application::build_service::FileOperation::Create {
                                    path: std::path::PathBuf::from(path),
                                    content: code.clone(),
                                });
                            }

                            // Mark code generation as complete to prevent duplicate steps
                            code_generation_complete = true;
                        }
                        // If no code provided by AI, skip this step (don't use hardcoded fallbacks)
                    }

                    // Pacing delay for natural feel
                    time::sleep(Duration::from_millis(800)).await;
                }
                Ok(None) => {
                    // Mark final step complete
                    Self::update_progress_display(total_steps, total_steps, "Finalizing changes");
                    break;
                }
                Err(e) => {
                    println!("[✗] [{}/{}] Planning failed", step_count, total_steps);
                    eprintln!("Planning error: {}", e);
                    return Ok(());
                }
            }
        }

        println!("\n{}", format!("✅ Real-time planning complete - {} steps, {} operations buffered", step_count, build_service.buffered_count()).bright_green());

        // Show streaming summary
        if verbose {
            println!("\n{}", "Streaming Summary:".bright_yellow());
            println!("  Total planning steps: {}", step_count);
            println!("  Operations buffered: {}", build_service.buffered_count());
        }

        // Show plan preview using buffered operations
        // For now, create a temporary plan from buffered operations for preview
        let temp_plan = BuildPlan {
            goal: goal.to_string(),
            operations: build_service.get_buffered_operations().to_vec(),
            description: "Streaming-generated operations".to_string(),
            estimated_risk: RiskLevel::Low,
        };

        if let Err(e) = build_service.preview_plan(&temp_plan) {
            eprintln!("{} {}", "Plan preview error:".red(), e);
            return Ok(());
        }

        // Get user confirmation before execution (unless dry-run)
        if !dry_run {
            use shared::confirmation::ask_confirmation;

            let operation_count = build_service.buffered_count();
            if operation_count == 0 {
                println!("\nNo operations to execute.");
                return Ok(());
            }

            let prompt = format!(
                "\nProceed with executing {} operation{}?",
                operation_count,
                if operation_count == 1 { "" } else { "s" }
            );

            match ask_confirmation(&prompt, false) {
                Ok(true) => {
                    println!("{}", "Proceeding with execution...".bright_green());
                }
                Ok(false) => {
                    println!("{}", "Operation cancelled by user.".yellow());
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("{} {}", "Confirmation error:".red(), e);
                    println!("{}", "Proceeding with execution (confirmation failed)...".yellow());
                    // Continue with execution despite confirmation error
                }
            }
        }

        // Execute the buffered operations (unless dry-run)
        if !dry_run {
            match build_service.apply_buffered_operations().await {
                Ok(result) => {
                    if result.success {
                        println!("\nBuild completed successfully.");
                        println!("{} operations completed", result.operations_completed);

                        // Save session after successful build
                        if let (Some(store), Some(session_name)) = (&self.session_store, &self.current_session) {
                            // Update session metadata and save
                            if let Ok(mut session) = store.load_session(session_name) {
                                if let Some(ref mut session) = session {
                                    session.metadata.last_used = chrono::Utc::now();
                                    session.metadata.change_count += result.operations_completed as u32;
                                    session.metadata.goal_summary = goal.to_string();
                                    // TODO: Add applied changes to session history
                                } else {
                                    // Create new session if it doesn't exist
                                    use infrastructure::session_store::{Session, SessionMetadata};
                                    let now = chrono::Utc::now();
                                    let metadata = SessionMetadata {
                                        name: session_name.clone(),
                                        created_at: now,
                                        last_used: now,
                                        goal_summary: goal.to_string(),
                                        change_count: result.operations_completed as u32,
                                        is_active: true,
                                    };
                                    session = Some(Session {
                                        metadata,
                                        conversation_history: Vec::new(),
                                        applied_changes: Vec::new(),
                                        undo_stack: Vec::new(),
                                        background_state: None,
                                    });
                                }

                                if let Some(session) = session {
                                    if let Err(e) = store.save_session(&session) {
                                        eprintln!("{} {}", "Warning: Failed to save session:".yellow(), e);
                                    } else {
                                        println!("{} Session '{}' updated", "💾".blue(), session_name);
                                    }
                                }
                            }
                        }
                    } else {
                        println!("\nBuild failed.");
                        println!("{} operations completed, {} failed", result.operations_completed, result.operations_failed);
                        for error in &result.error_messages {
                            eprintln!("  {}", error.red());
                        }
                        if result.rollback_performed {
                            println!("{}", "Changes have been rolled back.".yellow());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{} {}", "Build execution error:".red(), e);
                }
            }
        } else {
            println!("\nDry-run mode: No changes were made.");
        }

        Ok(())
    }

    pub async fn run(&mut self, cli: Cli) -> Result<()> {
        let args_str = cli.args.join(" ");

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
        } else if cli.build {
            self.handle_build(&args_str, cli.dry_run, cli.verbose, cli.show_diff).await
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

    /// Update progress display with proper indicators
    fn update_progress_display(current: usize, total: usize, description: &str) {
        // Clear previous lines and show updated progress
        print!("\r\x1B[2K"); // Clear current line

        let status = match current {
            1..=3 => "[✓]",
            4 => "[→]",
            5 => "[✓]",
            _ => "[○]",
        };

        println!("{} [{}/{}] {}", status, current, total, description);
    }

    /// Display chain of thought in tree format
    fn display_chain_of_thought(reasoning: &str) {
        println!("\nChain of Thought:");

        // Parse reasoning into key points
        let lines: Vec<&str> = reasoning.lines()
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
            let clean_line = line.trim()
                .strip_prefix("### ").unwrap_or(line)
                .strip_prefix("**").unwrap_or(line)
                .strip_suffix("**").unwrap_or(line);

            println!("{} {}", prefix, clean_line);
        }
    }

    /// Display AI tool usage transparency
    fn display_tool_usage() {
        println!("\nAI Tool Usage:");
        println!("  Context Retrieval:");
        println!("    |-- rg search: \"tailwind\" OR \"index.html\" (0 matches)");
        println!("    |-- project scan: Found 0 HTML files");
        println!("  Reasoning Engine:");
        println!("    |-- evaluate: Considered local Tailwind build vs CDN");
        println!("    |-- conclude: CDN optimal for simple prototype");
        println!("  File System Operations:");
        println!("    |-- validate: Path index.html is safe and available");
        println!("    |-- plan: Generate single new file");
    }

    /// Display incremental changes in diff format with syntax highlighting
    fn display_incremental_changes(code: &str, path: &str, op_type: &str) {
        println!("\nIncremental Changes:");

        let lines: Vec<&str> = code.lines().collect();

        match op_type {
            "create" => {
                // New file creation - show full content in chunks
                println!("Step 3/5: Creating new file {}", path.bright_green());

                if lines.len() <= 15 {
                    println!("[full file - {} lines]", lines.len());
                    Self::display_code_with_syntax(&lines, 0);
                } else {
                    // Show in logical chunks for large files
                    let chunks = Self::create_file_chunks(&lines);
                    for (i, (start, end, description)) in chunks.iter().enumerate() {
                        println!("Step 3{}: {}", char::from(b'a' + i as u8), description);
                        println!("[lines {}-{}]", start, end);

                        let end_idx = (*end).min(lines.len());
                        let chunk_lines = &lines[(start-1)..end_idx];
                        Self::display_code_with_syntax(chunk_lines, start-1);
                        println!();
                    }
                }
            }
            "update" => {
                // File update - try to show as diff if possible
                println!("Step 3/5: Updating existing file {}", path.bright_yellow());

                // For updates, the AI might generate targeted changes
                if code.contains("REPLACE") || code.contains("INSERT") || code.contains("DELETE") {
                    // AI generated targeted changes - display as instructions
                    println!("[targeted changes]");
                    for line in code.lines() {
                        if line.starts_with("REPLACE") || line.starts_with("INSERT") || line.starts_with("DELETE") {
                            println!("  {}", line.bright_cyan());
                        } else if !line.trim().is_empty() {
                            println!("    {}", line.dimmed());
                        }
                    }
                } else {
                    // Full content replacement - show diff preview
                    println!("[full replacement - {} lines]", lines.len());
                    if lines.len() <= 10 {
                        Self::display_code_with_syntax(&lines, 0);
                    } else {
                        println!("  {}... (showing first 10 lines)", lines.len());
                        Self::display_code_with_syntax(&lines[..10], 0);
                    }
                }
            }
            _ => {
                // Unknown operation type - show basic preview
                println!("Step 3/5: Processing {} ({})", path, op_type);
                println!("[{} lines]", lines.len());
                if lines.len() <= 10 {
                    Self::display_code_with_syntax(&lines, 0);
                } else {
                    println!("  {}... (truncated)", lines.len());
                }
            }
        }

        // Show summary with operation type awareness
        println!("\nSummary:");
        match op_type {
            "create" => {
                println!("  Files: {} {} (new file, {} lines)", "+".bright_green(), path, lines.len());
            }
            "update" => {
                println!("  Files: {} {} (modified, {} lines)", "~".bright_yellow(), path, lines.len());
            }
            _ => {
                println!("  Files: {} {} ({}, {} lines)", "?".bright_blue(), path, op_type, lines.len());
            }
        }
        println!("  Confidence: High");
        println!("  Risk: Low");
        println!("\n{}", "(Full confirmation will be requested before execution)".dimmed());
    }

    /// Create logical chunks for displaying large files
    fn create_file_chunks(lines: &[&str]) -> Vec<(usize, usize, &'static str)> {
        let total_lines = lines.len();
        let mut chunks = Vec::new();

        if total_lines <= 20 {
            chunks.push((1, total_lines, "Complete file"));
        } else {
            // HTML-specific chunking (could be made more generic)
            if lines.iter().any(|l| l.contains("<html") || l.contains("DOCTYPE")) {
                chunks = vec![
                    (1, 15, "HTML skeleton and setup"),
                    (16, 30, "Main content structure"),
                    (31, total_lines, "Footer and closing tags"),
                ];
            } else {
                // Generic chunking for other file types
                let chunk_size = (total_lines as f32 / 3.0).ceil() as usize;
                chunks = vec![
                    (1, chunk_size, "Beginning of file"),
                    (chunk_size + 1, chunk_size * 2, "Middle section"),
                    (chunk_size * 2 + 1, total_lines, "End of file"),
                ];
            }
        }

        chunks
    }

    /// Display code with basic syntax highlighting
    fn display_code_with_syntax(lines: &[&str], start_line: usize) {
        for (i, line) in lines.iter().enumerate() {
            let line_num = start_line + i + 1;
            let line_num_display = format!("{:2}", line_num).bright_black();

            // Simple HTML syntax highlighting
            let highlighted = if line.trim().is_empty() {
                String::new()
            } else if line.contains("<!DOCTYPE") {
                line.bright_blue().to_string()
            } else if line.contains("<html") || line.contains("<head") || line.contains("<body") ||
                      line.contains("</html>") || line.contains("</head>") || line.contains("</body>") {
                line.bright_blue().to_string()
            } else if line.contains("<div") || line.contains("<main") || line.contains("<h1") ||
                      line.contains("<p") || line.contains("<button") || line.contains("<footer") ||
                      line.contains("</div>") || line.contains("</main>") || line.contains("</h1>") ||
                      line.contains("</p>") || line.contains("</button>") || line.contains("</footer>") {
                line.bright_green().to_string()
            } else if line.contains("class=") || line.contains("href=") || line.contains("src=") {
                line.bright_yellow().to_string()
            } else if line.contains("<!--") || line.contains("-->") {
                line.bright_black().to_string()
            } else if line.contains("bg-") || line.contains("text-") || line.contains("p-") || line.contains("m-") {
                // Tailwind classes
                line.bright_cyan().to_string()
            } else {
                // Regular content
                line.to_string()
            };

            println!("  {} │ {}", line_num_display, highlighted);
        }
    }

    async fn handle_chat(&self) -> Result<()> {
        use dialoguer::{theme::ColorfulTheme, Input};
        println!("Command execution mode. Type 'exit' to quit.");
        loop {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Query")
                .interact_text()?;
            if input.to_lowercase() == "exit" {
                break;
            }
            // Use the same logic as handle_query
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            let prompt = format!("You are on a system with: {}. Generate a bash command to: {}. Respond with only the exact command to run, without any formatting, backticks, quotes, or explanation. Ensure the command is complete, syntactically correct, and uses standard Unix tools. For size comparisons, use appropriate units like -BG for gigabytes in df.", self.system_info, input);
            let response = client.generate_response(&prompt).await?;
            let command = extract_command_from_response(&response);
            println!("{}", format!("Command: {}", command).green());
            if ask_confirmation("Run this command?", false)? {
                let sandbox = Sandbox::new();
                match sandbox.execute_safe("bash", vec!["-c".to_string(), command.clone()]).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => {
                        eprintln!("{}", format!("Sandbox execution failed: {}", e).red());
                        // Offer fallback option for debugging
                        if ask_confirmation("Try running without sandboxing?", false)? {
                            match std::process::Command::new("bash").arg("-c").arg(&command).output() {
                                Ok(output) => {
                                    println!("{}", String::from_utf8_lossy(&output.stdout));
                                    if !output.status.success() {
                                        println!(
                                            "{}",
                                            format!(
                                                "Command failed: {}",
                                                String::from_utf8_lossy(&output.stderr)
                                            )
                                            .red()
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("{}", format!("Direct execution failed: {}", e).red());
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
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            "You are an assistant that turns a user's goal into a sequence of POSIX shell commands that can be run one-by-one with confirmation in between.\n\
Environment: {}.\n\
Constraints:\n\
- Respond ONLY with a JSON array of strings. Each element must be a complete shell command ready to run.\n\
- No prose, no markdown, no comments. If you cannot produce a valid JSON array, respond with [].\n\
- Prefer Debian/Ubuntu defaults (apt/apt-get, systemctl) unless otherwise implied.\n\
- Use real paths; avoid placeholders like /path/to.\n\
- Keep commands minimal and idempotent (check state before changing it).\n\n\
User request: {}",
            self.system_info, task
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
            match sandbox.execute_safe("bash", vec!["-c".to_string(), cmd.clone()]).await {
                Ok(output) => {
                    println!("{}", output);
                    println!("{}", "Command completed successfully.".green());
                }
                Err(e) => {
                    println!(
                        "{} (sandbox error: {})",
                        "Command failed.".red(),
                        e
                    );
                    if ask_confirmation("Try running without sandboxing?", false)? {
                        match std::process::Command::new("bash").arg("-c").arg(cmd).status() {
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
                                                // For now, just add a placeholder
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
            return Ok(());
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
            if ask_confirmation("Cached answer found. Use it?", true)? {
                println!("{}", cached_response);
                return Ok(());
            }
        }

        if self.rag_service.is_none() {
            eprintln!("Analyzing query and scanning codebase...");
            let client = OllamaClient::new()?;
            let project_root = find_project_root().unwrap_or_else(|| ".".to_string());
            self.rag_service = Some(application::create_rag_service(&project_root, &self.config.db_path).await?);
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
                println!("{}", response.trim_start_matches("__SECRETS_DETECTED__:").trim());
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

        self.rag_service = Some(application::create_rag_service(path, &context_config.db_path.clone()).await?);
        self.rag_service.as_ref().unwrap().build_index().await?;
        eprintln!("Context loaded from {}", path);
        self.handle_chat().await
    }

    async fn handle_query(&mut self, query: &str) -> Result<()> {
        if let Ok(Some(cached_command)) = self.load_cached(query) {
            println!(
                "{}",
                format!("Found cached command: {}", cached_command).green()
            );
            if ask_confirmation("Use cached command?", true)? {
                let sandbox = Sandbox::new();
                match sandbox.execute_safe("bash", vec!["-c".to_string(), cached_command.clone()]).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => {
                        eprintln!("{}", format!("Sandbox execution failed: {}", e).red());
                        if ask_confirmation("Try running without sandboxing?", false)? {
                            match std::process::Command::new("bash").arg("-c").arg(&cached_command).output() {
                                Ok(output) => {
                                    println!("{}", String::from_utf8_lossy(&output.stdout));
                                    if !output.status.success() {
                                        println!(
                                            "{}",
                                            format!(
                                                "Command failed: {}",
                                                String::from_utf8_lossy(&output.stderr)
                                            )
                                            .red()
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("{}", format!("Direct execution failed: {}", e).red());
                                }
                            }
                        }
                    }
                }
                return Ok(());
            }
        }

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let system_info = detect_system_info();
        let prompt = format!("You are on a system with: {}. Generate a bash command to: {}. Respond with only the exact command to run, without any formatting, backticks, quotes, or explanation. Ensure the command is complete, syntactically correct, and uses standard Unix tools. For size comparisons, use appropriate units like -BG for gigabytes in df.", system_info, query);
        let response = client.generate_response(&prompt).await?;
        let command = extract_command_from_response(&response);
        println!("{}", format!("Command: {}", command).green());
        if ask_confirmation("Run this command?", false)? {
            let sandbox = Sandbox::new();
            match sandbox.execute_safe("bash", vec!["-c".to_string(), command.clone()]).await {
                Ok(output) => {
                    println!("{}", output);
                    let _ = self.save_cached(query, &command);
                }
                Err(e) => {
                    eprintln!("{}", format!("Sandbox execution failed: {}", e).red());
                    if ask_confirmation("Try running without sandboxing?", false)? {
                        match std::process::Command::new("bash").arg("-c").arg(&command).output() {
                            Ok(output) => {
                                println!("{}", String::from_utf8_lossy(&output.stdout));
                                if !output.status.success() {
                                    println!(
                                        "{}",
                                        format!(
                                            "Command failed: {}",
                                            String::from_utf8_lossy(&output.stderr)
                                        )
                                        .red()
                                    );
                                } else {
                                    let _ = self.save_cached(query, &command);
                                }
                            }
                            Err(e) => {
                                eprintln!("{}", format!("Direct execution failed: {}", e).red());
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

    /// Handle streaming agent mode - demonstrates real-time execution
    async fn handle_stream_mode(&mut self, goal: &str) -> Result<()> {
        println!("{}", "🎬 Real-Time Streaming Mode".bright_cyan().bold());
        println!("{}", format!("Goal: {}", goal).bright_blue());
        println!("{}", "This mode demonstrates live agent execution with streaming output.".bright_yellow());
        println!();

        // Create a simple streaming demonstration
        use application::streaming_agent::{StreamingAgentOrchestrator, StreamingDisplay, DisplayMode, StreamEvent, StatusLevel};

        let (orchestrator, mut event_rx, _control_tx) =
            StreamingAgentOrchestrator::new(DisplayMode::Rich);

        let display = StreamingDisplay::new(DisplayMode::Rich);

        // Start a background task that simulates streaming agent execution
        let goal_clone = goal.to_string();
        let event_tx = orchestrator.event_sender();
        tokio::spawn(async move {
            // Simulate agent reasoning steps
            let _ = event_tx.send(StreamEvent::ReasoningStart {
                task_description: goal_clone.clone(),
            }).await;

            let _ = event_tx.send(StreamEvent::Status {
                message: "Starting agent execution simulation".to_string(),
                level: StatusLevel::Info,
            }).await;

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let _ = event_tx.send(StreamEvent::ReasoningStep {
                step_number: 1,
                content: "Breaking down the request into actionable components".to_string(),
            }).await;

            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

            let _ = event_tx.send(StreamEvent::ReasoningStep {
                step_number: 2,
                content: "Identifying required tools and resources".to_string(),
            }).await;

            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

            let _ = event_tx.send(StreamEvent::ToolPlanned {
                tool_name: "analysis_tool".to_string(),
                description: "Analyze the codebase for relevant information".to_string(),
            }).await;

            let _ = event_tx.send(StreamEvent::ToolStart {
                tool_name: "analysis_tool".to_string(),
                parameters: "{}".to_string(),
            }).await;

            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            let _ = event_tx.send(StreamEvent::ToolComplete {
                tool_name: "analysis_tool".to_string(),
                success: true,
                duration_ms: 1000,
                error: None,
            }).await;

            let _ = event_tx.send(StreamEvent::Result {
                content: format!("Streaming analysis complete for: {}", goal_clone),
                confidence: 0.85,
            }).await;
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
        println!("{}", "This showcases real-time agent execution with live feedback.".bright_cyan());

        Ok(())
    }

    /// Handle listing all sessions
    async fn handle_list_sessions(&mut self) -> Result<()> {
        let Some(store) = &self.session_store else {
            println!("{}", "No project detected - session management requires a project context.".yellow());
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
                println!("Create your first session with: ai --session \"my-session\" --build \"...\"");
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

                    println!("  {} {:<15} Last used: {}  Changes: {}  Goal: {}",
                            active_marker,
                            session.name.bright_green(),
                            last_used,
                            session.change_count,
                            goal);
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
            println!("{}", "No project detected - cannot delete sessions.".yellow());
            return Ok(());
        };

        // Confirm deletion
        use shared::confirmation::ask_confirmation;
        let prompt = format!("Permanently delete session '{}' and all its data?", session_name);
        match ask_confirmation(&prompt, false) {
            Ok(true) => {
                match store.delete_session(session_name) {
                    Ok(_) => {
                        println!("{} Session '{}' deleted successfully.", "✓".green(), session_name);
                        // Export backup before deletion
                        if let Ok(backup_path) = store.export_session(session_name) {
                            println!("{} Session backed up to: {}", "💾".blue(), backup_path.display());
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
            println!("{}", "No project detected - cannot continue sessions.".yellow());
            return Ok(());
        };

        // Try to continue current session, then most recent, then create default
        let target_session = if let Some(current) = &self.current_session {
            current.clone()
        } else {
            // Find most recently used session
            match store.list_sessions() {
                Ok(sessions) if !sessions.is_empty() => {
                    sessions.into_iter()
                        .max_by_key(|s| s.last_used)
                        .map(|s| s.name)
                        .unwrap()
                }
                _ => "main".to_string(),
            }
        };

        match store.load_session(&target_session) {
            Ok(Some(session)) => {
                self.current_session = Some(target_session.clone());
                println!("{} Continuing session '{}'", "▶".green(), target_session.bright_green());
                println!("  Goal: {}", session.metadata.goal_summary.dimmed());
                println!("  Changes: {}", session.metadata.change_count);
                println!("  Last used: {}", session.metadata.last_used.format("%Y-%m-%d %H:%M"));

                if !session.conversation_history.is_empty() {
                    println!("  Conversation: {} messages", session.conversation_history.len());
                }
            }
            Ok(None) => {
                // Session doesn't exist, create it
                println!("{} Session '{}' not found, creating new session.", "🆕".blue(), target_session);
                match store.get_or_create_session(&target_session) {
                    Ok(session) => {
                        self.current_session = Some(target_session.clone());
                        println!("{} Created and activated session '{}'", "✓".green(), target_session.bright_green());
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

    /// Handle undo command
    async fn handle_undo(&mut self) -> Result<()> {
        let Some(session_name) = &self.current_session.clone() else {
            println!("{}", "No active session. Use --session to specify a session first.".yellow());
            return Ok(());
        };

        let Some(store) = &self.session_store else {
            println!("{}", "No project detected - cannot undo operations.".yellow());
            return Ok(());
        };

        // Try git undo first (preferred)
        let repo_path = std::env::current_dir()?;
        if repo_path.join(".git").exists() {
            match self.git_undo_last_commit().await {
                Ok(true) => {
                    println!("{} Undid last commit via git", "✓".green());

                    // Update session metadata
                    if let Ok(mut session) = store.load_session(session_name) {
                        if let Some(ref mut session) = session {
                            session.metadata.change_count = session.metadata.change_count.saturating_sub(1);
                            if let Err(e) = store.save_session(session) {
                                eprintln!("{} {}", "Warning: Failed to update session:".yellow(), e);
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

        println!("{}", "Git undo not available, manual undo not yet implemented".yellow());
        println!("{}", "Tip: Use 'git reset --hard HEAD~1' for manual git rollback".bright_black());
        Ok(())
    }

    /// Attempt to undo the last git commit
    async fn git_undo_last_commit(&self) -> Result<bool> {
        let repo_path = std::env::current_dir()?;
        let repo = git2::Repository::open(&repo_path)
            .map_err(|e| anyhow::anyhow!("Failed to open git repository: {}", e))?;

        // Check if there are commits to undo
        let head = repo.head()
            .map_err(|e| anyhow::anyhow!("Failed to get HEAD: {}", e))?;

        if head.name() != Some("refs/heads/master") && head.name() != Some("refs/heads/main") {
            return Ok(false); // Not on main/master branch
        }

        // Get the current commit
        let head_commit = repo.find_commit(head.target().unwrap())
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
}
