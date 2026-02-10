use super::CliHandlers;
use colored::Colorize;
use shared::types::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

impl CliHandlers {
    pub(crate) async fn interpret_output(&self, query: &str, output: &str) -> Result<()> {
        println!("\n{}", "=== AI Interpretation ===".green().bold());

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            "The user asked: '{}'\n\n\
            Command output:\n{}\n\n\
            Please provide a clear, concise summary of what this output means. \
            Focus on the key information and present it in a well-organized format. \
            Use sections and bullet points where appropriate.",
            query, output
        );

        let response = client.generate_response(&prompt).await?;
        println!("{}", response);
        Ok(())
    }

    pub(crate) fn spawn_incremental_interpreter(
        &self,
        query: &str,
        rx: mpsc::Receiver<super::OutputLine>,
    ) -> thread::JoinHandle<()> {
        let query = query.to_string();
        thread::spawn(move || {
            let client = match infrastructure::ollama_client::OllamaClient::new() {
                Ok(client) => client,
                Err(_) => return,
            };
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return,
            };

            let mut buffer = String::new();
            let mut line_count = 0usize;
            let mut summary = String::new();

            let mut flush = |chunk: &str, summary: &str| -> Option<String> {
                if chunk.trim().is_empty() {
                    return None;
                }
                let prompt = format!(
                    "You are providing incremental updates from a running command.\n\
User asked: \"{}\"\n\
Previous summary:\n{}\n\
\n\
New output chunk:\n{}\n\
\n\
Return a concise update in 1-3 short bullets. If no new findings, say \"No new findings.\"",
                    query,
                    if summary.trim().is_empty() {
                        "<none>"
                    } else {
                        summary
                    },
                    chunk
                );
                rt.block_on(client.generate_response(&prompt)).ok()
            };

            for line in rx {
                match line {
                    super::OutputLine::Stdout(text) => {
                        buffer.push_str(&text);
                        buffer.push('\n');
                    }
                    super::OutputLine::Stderr(text) => {
                        buffer.push_str("STDERR: ");
                        buffer.push_str(&text);
                        buffer.push('\n');
                    }
                }
                line_count += 1;

                if line_count >= 20 || buffer.len() >= 2000 {
                    if let Some(update) = flush(&buffer, &summary) {
                        let trimmed = update.trim();
                        if !trimmed.is_empty() {
                            println!("\n=== AI Update ===\n{}\n", trimmed);
                            summary = trimmed.to_string();
                        }
                    }
                    buffer.clear();
                    line_count = 0;
                }
            }

            if !buffer.trim().is_empty() {
                if let Some(update) = flush(&buffer, &summary) {
                    let trimmed = update.trim();
                    if !trimmed.is_empty() {
                        println!("\n=== AI Update ===\n{}\n", trimmed);
                    }
                }
            }

            thread::sleep(Duration::from_millis(10));
        })
    }
}
