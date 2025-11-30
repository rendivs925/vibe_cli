use application::rag_service::RagService;
use clap::Parser;
use colored::Colorize;
use docx_rs::*;
use infrastructure::ollama_client::OllamaClient;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::collections::HashSet;
use std::path::PathBuf;

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
#[command(name = "qwen-cli")]
#[command(about = "Qwen CLI assistant with RAG capabilities")]
pub struct Cli {
    /// Enter interactive chat mode
    #[arg(long)]
    pub chat: bool,

    /// Use multi-step agent mode
    #[arg(long)]
    pub agent: bool,

    /// Explain a file
    #[arg(long)]
    pub explain: bool,

    /// Query with RAG context
    #[arg(long)]
    pub rag: bool,

    /// Load context from path
    #[arg(long)]
    pub context: bool,

    /// Enter Leptos documentation mode
    #[arg(long)]
    pub leptos_mode: bool,

    /// The query or file path to process
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub struct CliApp {
    rag_service: Option<RagService>,
    cache_path: PathBuf,
}

impl CliApp {
    pub fn new() -> Self {
        let cache_path = Self::default_cache_path();
        Self {
            rag_service: None,
            cache_path,
        }
    }

    fn default_cache_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("qwen_cli_assistant");
        path.push("cli_cache.json");
        path
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

    pub async fn run(&mut self, cli: Cli) -> Result<()> {
        let args_str = cli.args.join(" ");
        if cli.chat {
            if args_str.trim().is_empty() {
                self.handle_chat().await
            } else {
                // Perhaps chat with initial message, but for now, just enter chat
                self.handle_chat().await
            }
        } else if cli.agent {
            self.handle_agent(&args_str).await
        } else if cli.explain {
            self.handle_explain(&args_str).await
        } else if cli.rag {
            self.handle_rag(&args_str).await
        } else if cli.context {
            self.handle_context(&args_str).await
        } else if cli.leptos_mode {
            self.handle_leptos_mode().await
        } else {
            // Default: general query
            self.handle_query(&args_str).await
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
            let system_info = detect_system_info();
            let prompt = format!("You are on a system with: {}. Generate a bash command to: {}. Respond with only the exact command to run, without any formatting, backticks, quotes, or explanation.", system_info, input);
            let response = client.generate_response(&prompt).await?;
            let command = extract_command_from_response(&response);
            println!("{}", format!("Command: {}", command).green());
            if dialoguer::Confirm::new()
                .with_prompt("Run this command?")
                .default(false)
                .interact()?
            {
                let output = std::process::Command::new("bash")
                    .arg("-c")
                    .arg(&command)
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
            } else {
                println!("{}", "Command execution cancelled.".yellow());
            }
        }
        Ok(())
    }

    async fn handle_agent(&self, task: &str) -> Result<()> {
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!("Plan and execute this multi-step task: {}", task);
        let response = client.generate_response(&prompt).await?;
        println!("{}", response);
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

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!("Explain this content in detail:\n\n{}", content);
        let response = client.generate_response(&prompt).await?;
        println!("{}", response);
        Ok(())
    }

    async fn handle_rag(&mut self, question: &str) -> Result<()> {
        if self.rag_service.is_none() {
            let client = OllamaClient::new()?;
            self.rag_service = Some(RagService::new(".", "embeddings.db", client)?);
            self.rag_service.as_ref().unwrap().build_index().await?;
        }
        let response = self.rag_service.as_ref().unwrap().query(question).await?;
        println!("{}", response);
        Ok(())
    }

    async fn handle_context(&mut self, path: &str) -> Result<()> {
        let client = OllamaClient::new()?;
        self.rag_service = Some(RagService::new(path, "embeddings.db", client)?);
        self.rag_service.as_ref().unwrap().build_index().await?;
        println!("Context loaded from {}", path);
        self.handle_chat().await
    }

    async fn handle_leptos_mode(&mut self) -> Result<()> {
        self.handle_context(".").await
    }

    async fn handle_query(&mut self, query: &str) -> Result<()> {
        if let Ok(Some(cached_command)) = self.load_cached(query) {
            println!(
                "{}",
                format!("Found cached command: {}", cached_command).green()
            );
            if dialoguer::Confirm::new()
                .with_prompt("Use cached command?")
                .default(true)
                .interact()?
            {
                let output = std::process::Command::new("bash")
                    .arg("-c")
                    .arg(&cached_command)
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

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let system_info = detect_system_info();
        let prompt = format!("You are on a system with: {}. Generate a bash command to: {}. Respond with only the exact command to run, without any formatting, backticks, quotes, or explanation.", system_info, query);
        let response = client.generate_response(&prompt).await?;
        let command = extract_command_from_response(&response);
        println!("{}", format!("Command: {}", command).green());
        if dialoguer::Confirm::new()
            .with_prompt("Run this command?")
            .default(false)
            .interact()?
        {
            let output = std::process::Command::new("bash")
                .arg("-c")
                .arg(&command)
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
            } else {
                let _ = self.save_cached(query, &command);
            }
        } else {
            println!("{}", "Command execution cancelled.".yellow());
        }
        Ok(())
    }
}

