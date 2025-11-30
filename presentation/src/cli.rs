use clap::Parser;
use std::collections::HashMap;
use application::rag_service::RagService;
use infrastructure::ollama_client::OllamaClient;
use shared::types::Result;
use docx_rs::*;

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



fn extract_command_from_response(response: &str) -> String {
    let response = response.trim();
    if response.starts_with("```bash") && response.ends_with("```") {
        let start = response.find('\n').unwrap_or(0) + 1;
        let end = response.len() - 3;
        response[start..end].trim().to_string()
    } else if response.starts_with("```") && response.ends_with("```") {
        let start = response.find('\n').unwrap_or(0) + 1;
        let end = response.len() - 3;
        response[start..end].trim().to_string()
    } else {
        response.to_string()
    }
}

pub struct CliApp {
    rag_service: Option<RagService>,
    cache: HashMap<String, String>,
    cache_file: String,
}

impl CliApp {
    pub fn new() -> Self {
        let cache_file = "cli_cache.txt".to_string();
        let cache = Self::load_cache(&cache_file);
        Self { rag_service: None, cache, cache_file }
    }

    fn load_cache(cache_file: &str) -> HashMap<String, String> {
        if let Ok(content) = std::fs::read_to_string(cache_file) {
            content.lines().filter_map(|line| {
                let mut parts = line.split('\t');
                Some((parts.next()?.to_string(), parts.next()?.to_string()))
            }).collect()
        } else {
            HashMap::new()
        }
    }

    fn save_cache(&self) {
        let content = self.cache.iter().map(|(k, v)| format!("{}\t{}", k, v)).collect::<Vec<_>>().join("\n");
        let _ = std::fs::write(&self.cache_file, content);
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
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        println!("Chat mode. Type 'exit' to quit.");
        loop {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("You")
                .interact_text()?;
            if input.to_lowercase() == "exit" {
                break;
            }
            let response = client.generate_response(&input).await?;
            println!("AI: {}", response);
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
                "pdf" => {
                    match pdf_extract::extract_text(file) {
                        Ok(text) => text,
                        Err(e) => {
                            println!("Error extracting text from PDF '{}': {}", file, e);
                            return Ok(());
                        }
                    }
                }
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

                _ => {
                    match std::fs::read_to_string(file) {
                        Ok(text) => text,
                        Err(_) => {
                            println!("Error: Cannot read file '{}' as text. Supported formats: text files, PDF, DOCX.", file);
                            return Ok(());
                        }
                    }
                }
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
        if let Some(cached_command) = self.cache.get(query) {
            println!("Cached command: {}", cached_command);
            if dialoguer::Confirm::new()
                .with_prompt("Use cached command?")
                .default(true)
                .interact()?
            {
                let output = std::process::Command::new("bash")
                    .arg("-c")
                    .arg(cached_command)
                    .output()?;
                if output.status.success() {
                    println!("{}", String::from_utf8_lossy(&output.stdout));
                } else {
                    println!("Command failed: {}", String::from_utf8_lossy(&output.stderr));
                }
                return Ok(());
            }
        }

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!("Generate a bash command to: {}. Respond with only the command, no explanation.", query);
        let response = client.generate_response(&prompt).await?;
        let command = extract_command_from_response(&response);
        println!("Suggested command: {}", command);
        if dialoguer::Confirm::new()
            .with_prompt("Execute this command?")
            .default(false)
            .interact()?
        {
            let output = std::process::Command::new("bash")
                .arg("-c")
                .arg(&command)
                .output()?;
            if output.status.success() {
                println!("{}", String::from_utf8_lossy(&output.stdout));
                self.cache.insert(query.to_string(), command.clone());
                self.save_cache();
            } else {
                println!("Command failed: {}", String::from_utf8_lossy(&output.stderr));
            }
        } else {
            println!("Command execution cancelled.");
        }
        Ok(())
    }
}