use super::CliHandlers;
use colored::Colorize;
use serde::Deserialize;
use shared::types::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time::timeout;

#[derive(Debug, Deserialize, Default)]
struct AiUpdate {
    #[serde(default)]
    key_points: Vec<String>,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    warnings: Vec<String>,
}

impl CliHandlers {
    pub(crate) async fn interpret_output_final(
        &self,
        query: &str,
        output: &str,
        previous_summary: &str,
    ) -> Result<()> {
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            "Provide a brief summary of this command output:\n\n\
User asked: \"{}\"\n\
Previous summary: \"{}\"\n\n\
Full output:\n{}\n",
            query,
            if previous_summary.trim().is_empty() {
                "<none>"
            } else {
                previous_summary
            },
            output
        );

        let response = timeout(Duration::from_secs(8), client.generate_response(&prompt))
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();
        let formatted = Self::format_ai_response(&response);
        if !formatted.is_empty() {
            println!("\n{}", "=== AI Final Summary ===".green().bold());
            println!("{}", formatted.trim());
        }
        Ok(())
    }

    pub(crate) fn spawn_incremental_interpreter(
        &self,
        query: &str,
        rx: mpsc::Receiver<super::OutputLine>,
        ack: mpsc::Sender<()>,
    ) -> thread::JoinHandle<String> {
        let query = query.to_string();
        thread::spawn(move || {
            let client = match infrastructure::ollama_client::OllamaClient::new() {
                Ok(client) => client,
                Err(_) => return String::new(),
            };
            let rt = match Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return String::new(),
            };

            let mut buffer = String::new();
            let mut summary = String::new();

            let flush = |chunk: &str, summary: &str| -> Option<String> {
                if chunk.trim().is_empty() {
                    return None;
                }
                let prompt = format!(
                    "Briefly summarize new output from running command:\n\n\
User asked: \"{}\"\n\
Previous summary: \"{}\"\n\n\
New output:\n{}\n",
                    query,
                    if summary.trim().is_empty() {
                        "<none>"
                    } else {
                        summary
                    },
                    chunk
                );
                rt.block_on(async {
                    timeout(Duration::from_secs(6), client.generate_response(&prompt))
                        .await
                        .ok()
                        .and_then(Result::ok)
                })
            };

            for line in rx {
                match line {
                    super::OutputLine::Stdout(text) => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            buffer.push_str(trimmed);
                            buffer.push('\n');
                        }
                    }
                    super::OutputLine::Stderr(text) => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            buffer.push_str("STDERR: ");
                            buffer.push_str(trimmed);
                            buffer.push('\n');
                        }
                    }
                    super::OutputLine::ChunkEnd => {
                        if let Some(update) = flush(&buffer, &summary) {
                            let mut formatted = Self::format_ai_response(&update);
                            if formatted.trim().is_empty() {
                                formatted = "No summary available.".to_string();
                            }
                            println!(
                                "\n{}\n{}",
                                "=== AI Chunk Summary ===".green().bold(),
                                formatted.trim()
                            );
                            summary = formatted;
                        }
                        buffer.clear();
                        let _ = ack.send(());
                    }
                }
            }

            if !buffer.trim().is_empty() {
                if let Some(update) = flush(&buffer, &summary) {
                    let mut formatted = Self::format_ai_response(&update);
                    if formatted.trim().is_empty() {
                        formatted = "No summary available.".to_string();
                    }
                    println!(
                        "\n{}\n{}",
                        "=== AI Chunk Summary ===".green().bold(),
                        formatted.trim()
                    );
                }
            }

            thread::sleep(Duration::from_millis(10));
            summary
        })
    }

    fn strip_code_fences(s: &str) -> String {
        let t = s.trim();
        if t.starts_with("```") {
            let mut lines = t.lines();
            let _ = lines.next();
            let mut body: Vec<&str> = lines.collect();
            if body
                .last()
                .map(|l| l.trim().starts_with("```"))
                .unwrap_or(false)
            {
                body.pop();
            }
            return body.join("\n").trim().to_string();
        }
        t.to_string()
    }

    fn is_no_new_findings(update: &AiUpdate) -> bool {
        let all = update
            .key_points
            .iter()
            .chain(update.errors.iter())
            .chain(update.warnings.iter())
            .map(|s| s.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();

        (update.key_points.is_empty() && update.errors.is_empty() && update.warnings.is_empty())
            || all
                .iter()
                .all(|x| x == "no new findings." || x == "no new findings")
    }

    fn format_ai_response(raw: &str) -> String {
        let cleaned = Self::strip_code_fences(raw);
        if let Ok(update) = serde_json::from_str::<AiUpdate>(&cleaned) {
            if Self::is_no_new_findings(&update) {
                return "No new findings.".to_string();
            }
            let mut lines = Vec::new();
            if !update.errors.is_empty() {
                lines.push("Errors:".to_string());
                for err in &update.errors {
                    lines.push(format!("- {}", err.trim()));
                }
            }
            if !update.warnings.is_empty() {
                lines.push("Warnings:".to_string());
                for warn in &update.warnings {
                    lines.push(format!("- {}", warn.trim()));
                }
            }
            if !update.key_points.is_empty() {
                lines.push("Key points:".to_string());
                for point in &update.key_points {
                    lines.push(format!("- {}", point.trim()));
                }
            }
            return lines.join("\n");
        }

        cleaned
            .lines()
            .map(|l| l.trim_start())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
