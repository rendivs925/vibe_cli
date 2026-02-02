use crate::cli::streaming::request_command_stream_then_confirm;

use super::cache::{CacheManager, ExplainCacheManager, RagCacheManager};
use super::command_extraction::{extract_command_from_response, parse_agent_plan};
use super::utils::{detect_system_info, project_cache_suffix};
use application::services::neurosymbolic_service::{NeurosymbolicConfig, NeurosymbolicService};
use application::services::rag_service::RagService;
use colored::Colorize;
use infrastructure::{config::Config, ollama_client::OllamaClient};
use shared::confirmation::{ask_confirmation, ask_feedback};
use shared::types::Message;
use shared::types::Result;

use std::path::PathBuf;
use std::process::Command;

pub struct CliHandlers {
    cache_manager: CacheManager,
    explain_cache_manager: ExplainCacheManager,
    rag_cache_manager: RagCacheManager,
    system_info: String,
    config: Config,
    rag_service: Option<RagService>,
    neurosymbolic_service: Option<NeurosymbolicService>,
}

impl CliHandlers {
    pub fn new(config: Config) -> Self {
        let cache_path = Self::default_cache_path();
        let explain_cache_path = Self::explain_cache_path();
        let rag_cache_path = Self::rag_cache_path();
        let system_info_path = Self::default_system_info_path();
        let system_info = Self::load_or_collect_system_info(&system_info_path);

        Self {
            cache_manager: CacheManager::new(cache_path),
            explain_cache_manager: ExplainCacheManager::new(explain_cache_path),
            rag_cache_manager: RagCacheManager::new(rag_cache_path),
            system_info,
            config,
            rag_service: None,
            neurosymbolic_service: None,
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

    fn default_system_info_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("vibe_cli");
        path.push("system_info.txt");
        path
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

    pub async fn handle_chat(&self) -> Result<()> {
        use dialoguer::{theme::ColorfulTheme, Input};
        println!("Command execution mode. Type 'exit' to quit.");
        loop {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Query")
                .interact_text()?;
            if input.to_lowercase() == "exit" {
                break;
            }
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            let prompt = format!("You are on a system with: {}. Generate a bash command to: {}. Respond with only the exact command to run, without any formatting, backticks, quotes, or explanation. Ensure the command is complete, syntactically correct, and uses standard Unix tools. For size comparisons, use appropriate units like -BG for gigabytes in df.", self.system_info, input);
            let response = client.generate_response(&prompt).await?;
            let command = extract_command_from_response(&response);
            println!("{}", format!("Command: {}", command).green());
            if ask_confirmation("Run this command?", false)? {
                let output = Command::new("bash").arg("-c").arg(&command).output()?;
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
            } else {
                println!("{}", "Command execution cancelled.".yellow());
            }
        }
        Ok(())
    }

    pub async fn handle_agent(&self, task: &str) -> Result<()> {
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
            let status = Command::new("bash").arg("-c").arg(cmd).status()?;
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
        Ok(())
    }

    pub async fn handle_explain(&self, file: &str) -> Result<()> {
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
                "docx" => match std::fs::read(file) {
                    Ok(bytes) => match docx_rs::read_docx(&bytes) {
                        Ok(docx) => {
                            let mut text = String::new();
                            for child in &docx.document.children {
                                match child {
                                    docx_rs::DocumentChild::Paragraph(p) => {
                                        text.push_str(&p.raw_text());
                                        text.push('\n');
                                    }
                                    docx_rs::DocumentChild::Table(_t) => {
                                        text.push_str("[Table content not extracted]\n");
                                    }
                                    _ => {}
                                }
                            }
                            text
                        }
                        Err(e) => {
                            println!("Error parsing DOCX '{}': {}", file, e);
                            return Ok(());
                        }
                    },
                    Err(e) => {
                        println!("Error reading DOCX file '{}': {}", file, e);
                        return Ok(());
                    }
                },
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

        if let Some(cached_response) = self.explain_cache_manager.load_cached(&prompt)? {
            println!("{}", cached_response);
            return Ok(());
        }

        eprintln!("Analyzing file content...");
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let response = client.generate_response(&prompt).await?;

        self.explain_cache_manager.save_cached(&prompt, &response)?;

        println!("{}", response);
        Ok(())
    }

    pub async fn handle_rag(&mut self, question: &str) -> Result<()> {
        if let Some(cached_response) = self.rag_cache_manager.load_cached(question)? {
            if ask_confirmation("Cached answer found. Use it?", true)? {
                println!("{}", cached_response);
                return Ok(());
            }
        }

        if self.rag_service.is_none() {
            eprintln!("Analyzing query and scanning codebase...");
            let client = OllamaClient::new()?;
            self.rag_service = Some(
                RagService::new(".", &self.config.db_path, client, self.config.clone()).await?,
            );
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

            println!("{}", response);

            if ask_confirmation("Satisfied with this response?", true)? {
                self.rag_cache_manager.save_cached(question, &response)?;
                break;
            } else {
                feedback.clear();
                feedback = ask_feedback("Provide feedback for improvement: ")?;
                eprintln!("Regenerating with feedback...");
            }
        }

        Ok(())
    }

    pub async fn handle_context(&mut self, path: &str) -> Result<()> {
        eprintln!("Loading context from {}...", path);
        let client = OllamaClient::new()?;
        self.rag_service =
            Some(RagService::new(path, &self.config.db_path, client, self.config.clone()).await?);
        self.rag_service.as_ref().unwrap().build_index().await?;
        eprintln!("Context loaded from {}", path);
        self.handle_chat().await
    }

    pub async fn handle_neurosymbolic(&mut self, query: &str) -> Result<()> {
        if self.neurosymbolic_service.is_none() {
            eprintln!("Initializing neurosymbolic service with domain configs...");
            let config = NeurosymbolicConfig::default();
            match NeurosymbolicService::new(config).await {
                Ok(service) => {
                    self.neurosymbolic_service = Some(service);
                    eprintln!("Neurosymbolic service initialized.");
                }
                Err(e) => {
                    eprintln!("Failed to initialize neurosymbolic service: {:?}", e);
                    return Ok(());
                }
            }
        }

        eprintln!("Processing query with neurosymbolic reasoning...");
        match self.neurosymbolic_service
            .as_mut()
            .unwrap()
            .process_query_with_domains(query)
            .await
        {
            Ok(response) => {
                println!("\n{}", "=== Neurosymbolic Response ===".green().bold());
                println!("Confidence: {:.1}%", response.confidence * 100.0);
                println!("\n{}", response.explanation);
                println!("\n{}", "=== Best Solution ===".green().bold());
                if let Some(solution) = response.ranked_solutions.first() {
                    println!("Command: {}", solution.solution.command_sequence.join("; "));
                    println!("Score: {:.2}", solution.combined_score);
                    println!("Risk: {:?}", solution.risk_assessment.risk_level);
                }
                println!("\n{}", "=== Reasoning Summary ===".green().bold());
                println!("{}", response.reasoning_trace.summary);

                if ask_confirmation("Execute this command?", false)? {
                    if let Some(solution) = response.ranked_solutions.first() {
                        for cmd in &solution.solution.command_sequence {
                            println!("\n{}", format!("Executing: {}", cmd).yellow());
                            let output = Command::new("bash").arg("-c").arg(cmd).output()?;
                            println!("{}", String::from_utf8_lossy(&output.stdout));
                            if !output.status.success() {
                                println!(
                                    "{}",
                                    format!("Command failed: {}", String::from_utf8_lossy(&output.stderr)).red()
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Neurosymbolic processing failed: {:?}", e);
                eprintln!("Falling back to standard query...");
                self.handle_query(query).await?;
            }
        }
        Ok(())
    }

    pub async fn handle_query(&mut self, query: &str) -> Result<()> {
        if let Ok(Some(cached_command)) = self.cache_manager.load_cached(query) {
            println!("{}", format!("Found cached commands").green());
            if ask_confirmation("Use cached command?", true)? {
                let output = Command::new("bash")
                    .arg("-c")
                    .arg(&cached_command[0].command)
                    .output()?;
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
                return Ok(());
            }
        }

        let messages = vec![Message {
            role: "user".to_string(),
            content: query.to_string(),
        }];

        let command = request_command_stream_then_confirm(&self.config, &messages).await?;
        if let Some(cmd) = command {
            let output = Command::new("bash").arg("-c").arg(&cmd).output()?;
            println!("{}", String::from_utf8_lossy(&output.stdout));
            if !output.status.success() {
                println!(
                    "{}",
                    format!("Command failed with exit code: {:?}", output.status.code()).red()
                );
                if !output.stderr.is_empty() {
                    println!("{}", String::from_utf8_lossy(&output.stderr).red());
                }
            }
        } else {
            // println!("{}", "No command generated or cancelled.".yellow());
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

    fn config_dir(&self) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/vibe_cli/domains")
    }

    pub async fn handle_neurosymbolic_init(&self) -> Result<()> {
        let config_dir = self.config_dir();
        
        println!("{}", "Initializing neurosymbolic domain configuration...".green().bold());
        
        if config_dir.exists() {
            println!("{}", "Domain config directory already exists.".yellow());
        } else {
            std::fs::create_dir_all(&config_dir)?;
            println!("{}", format!("Created: {}", config_dir.display()).green());
        }

        let linux_dir = config_dir.join("linux/entities");
        let shared_dir = config_dir.join("../shared_entities");
        
        std::fs::create_dir_all(&linux_dir)?;
        std::fs::create_dir_all(&shared_dir)?;
        
        println!("{}", "Created directory structure:".green());
        println!("  - {}", config_dir.display());
        println!("  - {}", linux_dir.display());
        println!("  - {}", shared_dir.display());

        let domain_json = r#"{
    "domain": "linux",
    "version": "1.0.0",
    "description": "Linux system administration domain",
    "depends_on": [],
    "priority": 10,
    "enabled": true
}"#;
        
        let domain_path = config_dir.join("linux/domain.json");
        if !domain_path.exists() {
            std::fs::write(&domain_path, domain_json)?;
            println!("{}", format!("Created: {}", domain_path.display()).green());
        }

        let ops_json = r#"[
    {
        "op_id": "list_processes",
        "name": "List processes",
        "description": "List running processes",
        "input_schema": {},
        "generators": [
            {
                "name": "ps_standard",
                "tool": "ps",
                "template": "ps aux",
                "when": []
            }
        ],
        "examples": []
    },
    {
        "op_id": "check_disk_usage",
        "name": "Check disk usage",
        "description": "Show disk space usage",
        "input_schema": {},
        "generators": [
            {
                "name": "df_standard",
                "tool": "df",
                "template": "df -h",
                "when": []
            }
        ],
        "examples": []
    }
]"#;
        
        let ops_path = config_dir.join("linux/operations.json");
        if !ops_path.exists() {
            std::fs::write(&ops_path, ops_json)?;
            println!("{}", format!("Created: {}", ops_path.display()).green());
        }

        println!("\n{}", "Domain configuration initialized successfully!".green().bold());
        println!("\n{}", "Usage:".green());
        println!("  vibe_cli --neurosymbolic \"list processes\"");
        println!("  vibe_cli --neurosymbolic \"check disk space\"");
        
        Ok(())
    }

    pub async fn handle_neurosymbolic_install(&self, package: &str) -> Result<()> {
        let config_dir = self.config_dir();
        
        println!("{}", format!("Installing domain package: {}", package).green().bold());
        
        if package.starts_with("http://") || package.starts_with("https://") {
            println!("{}", "Downloading from URL...".yellow());
            let client = reqwest::Client::new();
            let response = client.get(package).send().await?;
            
            if response.status().is_success() {
                let content = response.text().await?;
                let domain_name = package.split('/').last()
                    .unwrap_or(package)
                    .replace(".json", "");
                
                let target_dir = config_dir.join(&domain_name);
                std::fs::create_dir_all(&target_dir)?;
                std::fs::write(target_dir.join("domain.json"), content)?;
                
                println!("{}", format!("Installed domain: {}", domain_name).green());
            } else {
                eprintln!("{}", "Failed to download package".red());
            }
        } else {
            println!("{}", format!("Looking for local package: {}", package).yellow());
            let package_dir = std::path::Path::new(package);
            if package_dir.exists() && package_dir.is_dir() {
                let domain_name = package_dir.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                
                let target_dir = config_dir.join(&domain_name);
                std::fs::create_dir_all(&target_dir)?;
                
                for entry in std::fs::read_dir(package_dir)? {
                    let entry = entry?;
                    if entry.path().is_file() {
                        let file_name = entry.file_name();
                        std::fs::copy(entry.path(), target_dir.join(&file_name))?;
                    }
                }
                
                println!("{}", format!("Installed domain: {}", domain_name).green());
            } else {
                eprintln!("{}", format!("Package not found: {}", package).red());
            }
        }
        
        Ok(())
    }

    pub async fn handle_neurosymbolic_remove(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);
        
        println!("{}", format!("Removing domain: {}", domain).green().bold());
        
        if domain_dir.exists() {
            std::fs::remove_dir_all(&domain_dir)?;
            println!("{}", format!("Removed: {}", domain_dir.display()).green());
        } else {
            println!("{}", format!("Domain not found: {}", domain).yellow());
        }
        
        Ok(())
    }

    pub async fn handle_neurosymbolic_edit(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);
        
        println!("{}", format!("Editing domain: {}", domain).green().bold());
        
        if !domain_dir.exists() {
            println!("{}", format!("Domain not found: {}. Use --neurosymbolic-add to create it.", domain).yellow());
            return Ok(());
        }

        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        
        for entry in std::fs::read_dir(&domain_dir)? {
            let entry = entry?;
            if entry.path().is_file() {
                let file_name = entry.file_name();
                println!("{}", format!("Opening: {}", file_name.display()).yellow());
                
                let status = Command::new(&editor)
                    .arg(entry.path())
                    .status()?;
                
                if status.success() {
                    println!("{}", format!("Saved: {}", file_name.display()).green());
                }
            }
        }
        
        Ok(())
    }

    pub async fn handle_neurosymbolic_add(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);
        
        println!("{}", format!("Adding new domain: {}", domain).green().bold());
        
        std::fs::create_dir_all(&domain_dir.join("entities"))?;
        
        let domain_json = format!(r#"{{
    "domain": "{}",
    "version": "1.0.0",
    "description": "Custom domain: {}",
    "depends_on": [],
    "priority": 50,
    "enabled": true
}}"#, domain, domain);
        
        std::fs::write(domain_dir.join("domain.json"), &domain_json)?;
        println!("{}", format!("Created: {}/domain.json", domain).green());
        
        let ops_json = r#"[
    {
        "op_id": "custom_operation",
        "name": "Custom Operation",
        "description": "Description of your custom operation",
        "input_schema": {},
        "generators": [
            {
                "name": "custom_tool",
                "tool": "your-tool",
                "template": "your-tool --option value",
                "when": []
            }
        ],
        "examples": []
    }
]"#;
        
        std::fs::write(domain_dir.join("operations.json"), ops_json)?;
        println!("{}", format!("Created: {}/operations.json", domain).green());
        
        std::fs::write(domain_dir.join("relationships.json"), "[]")?;
        std::fs::write(domain_dir.join("inference_rules.json"), "[]")?;
        std::fs::write(domain_dir.join("troubleshooting.json"), "[]")?;
        
        println!("\n{}", "Domain template created!".green().bold());
        println!("{}", format!("Edit with: vibe_cli --neurosymbolic-edit {}", domain).yellow());
        
        Ok(())
    }

    pub async fn handle_neurosymbolic_list(&self) -> Result<()> {
        let config_dir = self.config_dir();
        
        println!("{}", "Installed Domains".green().bold());
        println!("{}", "==============".to_string());
        
        if !config_dir.exists() {
            println!("{}", "No domains installed. Run --neurosymbolic-init first.".yellow());
            return Ok(());
        }
        
        let mut domains: Vec<String> = Vec::new();
        
        for entry in std::fs::read_dir(&config_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let domain_name = entry.file_name().to_string_lossy().to_string();
                let domain_json = entry.path().join("domain.json");
                
                if domain_json.exists() {
                    if let Ok(content) = std::fs::read_to_string(&domain_json) {
                        if let Ok(domain) = serde_json::from_str::<serde_json::Value>(&content) {
                            let desc = domain.get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or("");
                            let version = domain.get("version")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let enabled = domain.get("enabled")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(true);
                            
                            let status = if enabled { "enabled" } else { "disabled" };
                            println!("  {} - {} (v{}) [{}]", 
                                domain_name.green().bold(), 
                                desc, 
                                version,
                                status
                            );
                            domains.push(domain_name);
                        }
                    }
                }
            }
        }
        
        if domains.is_empty() {
            println!("{}", "No domains found.".yellow());
        } else {
            println!("\n{}", format!("Total: {} domain(s)", domains.len()).cyan());
        }
        
        println!("\n{}", "Usage:".green());
        println!("  vibe_cli --neurosymbolic \"your query\"");
        println!("  vibe_cli --neurosymbolic-edit <domain>  # Edit a domain");
        println!("  vibe_cli --neurosymbolic-remove <domain>  # Remove a domain");
        
        Ok(())
    }
}
