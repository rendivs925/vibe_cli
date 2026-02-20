use super::CliHandlers;
use colored::Colorize;
use shared::theme;
use shared::types::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time::timeout;

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
            println!("\n{}", theme::accent("=== AI Final Summary ===").bold());
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
        self.spawn_incremental_interpreter_with_policy(query, rx, ack, ChunkSummaryPolicy::silent())
    }

    pub(crate) fn spawn_incremental_interpreter_with_policy(
        &self,
        query: &str,
        rx: mpsc::Receiver<super::OutputLine>,
        ack: mpsc::Sender<()>,
        policy: ChunkSummaryPolicy,
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
            let mut buffer_lines: usize = 0;

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
                            buffer_lines += 1;
                        }
                    }
                    super::OutputLine::Stderr(text) => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            buffer.push_str("STDERR: ");
                            buffer.push_str(trimmed);
                            buffer.push('\n');
                            buffer_lines += 1;
                        }
                    }
                    super::OutputLine::ChunkEnd => {
                        let should_summarize =
                            policy.emit && (buffer_lines >= policy.min_lines
                                || buffer.len() >= policy.min_chars);
                        if should_summarize {
                            if let Some(update) = flush(&buffer, &summary) {
                                let mut formatted = Self::format_ai_response(&update);
                                if formatted.trim().is_empty() {
                                    formatted = "No summary available.".to_string();
                                }
                                println!(
                                    "\n{}\n{}",
                                    theme::accent("=== AI Chunk Summary ===").bold(),
                                    formatted.trim()
                                );
                                summary = formatted;
                            }
                        }
                        buffer.clear();
                        buffer_lines = 0;
                        let _ = ack.send(());
                    }
                }
            }

            if !buffer.trim().is_empty() {
                let should_summarize =
                    policy.emit && (buffer_lines >= policy.min_lines || buffer.len() >= policy.min_chars);
                if should_summarize {
                    if let Some(update) = flush(&buffer, &summary) {
                        let mut formatted = Self::format_ai_response(&update);
                        if formatted.trim().is_empty() {
                            formatted = "No summary available.".to_string();
                        }
                        println!(
                            "\n{}\n{}",
                            theme::accent("=== AI Chunk Summary ===").bold(),
                            formatted.trim()
                        );
                        summary = formatted;
                    }
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

    fn format_ai_response(raw: &str) -> String {
        let cleaned = Self::strip_code_fences(raw);
        cleaned
            .lines()
            .map(|l| l.trim_start())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ChunkSummaryPolicy {
    pub min_lines: usize,
    pub min_chars: usize,
    pub emit: bool,
}

impl ChunkSummaryPolicy {
    pub(crate) fn silent() -> Self {
        Self {
            min_lines: usize::MAX,
            min_chars: usize::MAX,
            emit: false,
        }
    }

    pub(crate) fn long_output_default() -> Self {
        Self {
            min_lines: 20,
            min_chars: 1200,
            emit: true,
        }
    }
}
