use super::CliHandlers;
use colored::Colorize;
use serde::Deserialize;
use shared::types::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

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
    pub(crate) async fn interpret_output(&self, query: &str, output: &str) -> Result<()> {
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            "You summarize command output.\n\
Return STRICT JSON only (no markdown, no code fences).\n\
Schema:\n\
{{\"key_points\":[\"...\"],\"warnings\":[\"...\"],\"errors\":[\"...\"]}}\n\
Rules:\n\
- Use short bullets (max ~12 words each)\n\
- Only include items supported by the output\n\
- If nothing new: key_points=[\"No new findings.\"]\n\
\n\
User asked: \"{}\"\n\n\
Command output:\n{}\n",
            query, output
        );

        let response = client.generate_response(&prompt).await?;
        let formatted = Self::format_ai_response(&response);
        if !formatted.is_empty() {
            println!("\n{}", "=== AI Interpretation ===".green().bold());
            println!("{}", formatted);
        }
        Ok(())
    }

    pub(crate) async fn interpret_output_final(
        &self,
        query: &str,
        output: &str,
        previous_summary: &str,
    ) -> Result<()> {
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            "Final wrap-up.\n\
Return STRICT JSON only (no markdown, no code fences).\n\
Schema:\n\
{{\"key_points\":[\"...\"],\"warnings\":[\"...\"],\"errors\":[\"...\"]}}\n\
Rules:\n\
- Up to 5 total bullets across all arrays\n\
- Highlight failures/unhealthy/exit-code in errors\n\
\n\
User asked: \"{}\"\n\
Previous incremental summary:\n{}\n\
\n\
Full output:\n{}\n",
            query,
            if previous_summary.trim().is_empty() {
                "<none>"
            } else {
                previous_summary
            },
            output
        );

        let response = client.generate_response(&prompt).await?;
        let formatted = Self::format_ai_response(&response);
        if !formatted.is_empty() {
            println!("\n{}", "=== AI Final Summary ===".green().bold());
            println!("{}", formatted);
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
            let mut last_printed = String::new();

            let flush = |chunk: &str, summary: &str| -> Option<String> {
                if chunk.trim().is_empty() {
                    return None;
                }
                let prompt = format!(
                    "You are providing incremental updates from a running command.\n\
Return STRICT JSON only (no markdown, no code fences).\n\
Schema:\n\
{{\"key_points\":[\"...\"],\"warnings\":[\"...\"],\"errors\":[\"...\"]}}\n\
Rules:\n\
- Use short bullets (max ~12 words each)\n\
- Only include items supported by the output\n\
- If nothing new: key_points=[\"No new findings.\"]\n\
\n\
User asked: \"{}\"\n\
Previous summary:\n{}\n\
\n\
New output chunk:\n{}\n",
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
                    super::OutputLine::ChunkEnd => {
                        if let Some(update) = flush(&buffer, &summary) {
                            let formatted = Self::format_ai_response(&update);
                            if !formatted.is_empty() && formatted != last_printed {
                                println!(
                                    "\n{}\n{}\n",
                                    "=== AI Chunk Summary ===".green().bold(),
                                    formatted
                                );
                                last_printed = formatted.clone();
                                summary = formatted;
                            }
                        }
                        buffer.clear();
                        thread::sleep(Duration::from_secs(2));
                        let _ = ack.send(());
                    }
                }
            }

            if !buffer.trim().is_empty() {
                if let Some(update) = flush(&buffer, &summary) {
                    let formatted = Self::format_ai_response(&update);
                    if !formatted.is_empty() && formatted != last_printed {
                        println!(
                            "\n{}\n{}\n",
                            "=== AI Chunk Summary ===".green().bold(),
                            formatted
                        );
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

    fn is_no_new_findings(update: &AiUpdate) -> bool {
        let all = update
            .key_points
            .iter()
            .chain(update.errors.iter())
            .chain(update.warnings.iter())
            .map(|s| s.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();

        (update.key_points.is_empty()
            && update.errors.is_empty()
            && update.warnings.is_empty())
            || all
                .iter()
                .all(|x| x == "no new findings." || x == "no new findings")
    }

    fn render_section(
        title: &str,
        items: &[String],
        color: fn(&str) -> colored::ColoredString,
    ) -> String {
        if items.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        out.push_str(&format!("{}\n", color(title).bold()));
        for it in items {
            let msg = it
                .trim()
                .trim_start_matches(['-', '•', '*'])
                .trim();
            if msg.is_empty() {
                continue;
            }
            out.push_str(&format!("  {} {}\n", "•".cyan(), msg));
        }
        out
    }

    fn format_ai_response(raw: &str) -> String {
        let cleaned = Self::strip_code_fences(raw);
        if let Ok(update) = serde_json::from_str::<AiUpdate>(&cleaned) {
            if Self::is_no_new_findings(&update) {
                return String::new();
            }
            let mut out = String::new();
            out.push_str(&Self::render_section("Errors", &update.errors, |t| t.red()));
            out.push_str(&Self::render_section("Warnings", &update.warnings, |t| t.yellow()));
            out.push_str(&Self::render_section("Key points", &update.key_points, |t| t.green()));
            return out.trim_end().to_string();
        }

        Self::format_ai_update(&cleaned)
    }

    fn format_ai_update(raw: &str) -> String {
        let mut lines: Vec<String> = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.eq_ignore_ascii_case("bullets:") {
                continue;
            }
            let normalized = if trimmed.starts_with('-') {
                trimmed.to_string()
            } else if trimmed.eq_ignore_ascii_case("no new findings.")
                || trimmed.eq_ignore_ascii_case("no new findings")
            {
                "No new findings.".to_string()
            } else {
                format!("- {}", trimmed)
            };
            lines.push(normalized);
        }
        lines.join("\n")
    }
}
