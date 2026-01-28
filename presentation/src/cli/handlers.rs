use crate::cli::streaming::request_command_stream_then_confirm;

use super::cache::{CacheManager, ExplainCacheManager, RagCacheManager};
use super::command_extraction::{extract_command_from_response, parse_agent_plan};
use super::utils::{detect_system_info, project_cache_suffix};
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
}
