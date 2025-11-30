use clap::{Parser, Subcommand};
use application::rag_service::RagService;
use infrastructure::ollama_client::OllamaClient;
use shared::types::Result;

#[derive(Parser)]
#[command(name = "qwen-cli")]
#[command(about = "Qwen CLI assistant with RAG capabilities")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Chat,
    Agent { task: String },
    Explain { file: String },
    Rag { question: String },
    Context { path: String },
    LeptosMode,
}

pub struct CliApp {
    rag_service: Option<RagService>,
}

impl CliApp {
    pub fn new() -> Self {
        Self { rag_service: None }
    }

    pub async fn run(&mut self, cli: Cli) -> Result<()> {
        match cli.command {
            Commands::Chat => self.handle_chat().await,
            Commands::Agent { task } => self.handle_agent(&task).await,
            Commands::Explain { file } => self.handle_explain(&file).await,
            Commands::Rag { question } => self.handle_rag(&question).await,
            Commands::Context { path } => self.handle_context(&path).await,
            Commands::LeptosMode => self.handle_leptos_mode().await,
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
                    println!("DOCX support not yet implemented. Please convert to PDF or text format.");
                    return Ok(());
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
}