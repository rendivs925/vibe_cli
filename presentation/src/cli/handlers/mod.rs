mod agent;
mod cache;
mod chat;
mod domain_init;
mod domain_learning;
mod domain_manage;
mod domain_output;
mod explain;
mod neurosymbolic_filter;
mod neurosymbolic_flow;
mod neurosymbolic_utils;
mod rag;

#[cfg(test)]
mod tests;

use super::cache::CacheManager;
use super::command_extraction::query_keywords;
use super::utils::{detect_system_info, project_cache_suffix};
use application::services::neurosymbolic_service::IntegratedNeurosymbolicService;
use application::services::rag_service::RagService;
use infrastructure::config::Config;
use shared::types::Message;
use shared::types::Result;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

pub struct CliHandlers {
    cache_manager: CacheManager,
    system_info: String,
    config: Config,
    rag_service: Option<RagService>,
    integrated_service: Option<IntegratedNeurosymbolicService>,
}

impl CliHandlers {
    pub fn new(config: Config) -> Self {
        let cache_dir = Self::default_cache_dir();
        let system_info_path = Self::default_system_info_path();
        let system_info = Self::load_or_collect_system_info(&system_info_path);

        let integrated_service = IntegratedNeurosymbolicService::new().ok();

        Self {
            cache_manager: CacheManager::new(cache_dir.clone(), false),
            system_info,
            config,
            rag_service: None,
            integrated_service,
        }
    }

    pub fn has_neurosymbolic_domains(&self) -> bool {
        self.integrated_service
            .as_ref()
            .map(|service| service.has_enabled_domains())
            .unwrap_or(false)
    }

    fn default_cache_dir() -> PathBuf {
        let mut path = Self::home_dir();
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(suffix);
        path
    }

    fn default_system_info_path() -> PathBuf {
        let mut path = Self::home_dir();
        path.push(".config");
        path.push("vibe_cli");
        path.push("system_info.txt");
        path
    }

    fn home_dir() -> PathBuf {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
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

    fn ensure_integrated_service(&mut self) {
        if self.integrated_service.is_none() {
            self.integrated_service = IntegratedNeurosymbolicService::new().ok();
        }
    }

    fn direct_answer(&self, query: &str) -> Option<String> {
        self.integrated_service
            .as_ref()
            .and_then(|service| service.direct_answer(query))
    }

    async fn allowed_commands_from_rag(
        &mut self,
        query: &str,
    ) -> Result<Option<std::collections::HashSet<String>>> {
        if self.rag_service.is_none() {
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            self.rag_service = Some(
                RagService::new(".", &self.config.db_path, client, self.config.clone()).await?,
            );
            let keywords = query_keywords(query);
            self.rag_service
                .as_ref()
                .unwrap()
                .build_index_for_keywords(&keywords)
                .await?;
        }

        let Some(rag) = self.rag_service.as_ref() else {
            return Ok(None);
        };

        let chunks = rag.relevant_chunks(query, 30).await?;
        let mut allowed = std::collections::HashSet::new();
        for chunk in chunks {
            for cmd in super::command_extraction::extract_commands(&chunk, query) {
                allowed.insert(cmd.command);
            }
        }

        if allowed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(allowed))
        }
    }

    fn user_message(query: &str, critique_feedback: Option<&str>) -> Message {
        Message {
            role: "user".to_string(),
            content: critique_feedback.unwrap_or(query).to_string(),
        }
    }

    fn run_shell_command(&self, cmd: &str) -> Result<CommandOutput> {
        let output = Command::new("bash").arg("-c").arg(cmd).output()?;
        Ok(CommandOutput::from(output))
    }

    fn run_shell_command_streaming(&self, cmd: &str) -> Result<CommandOutput> {
        self.run_shell_command_streaming_with_sink(cmd, None)
    }

    fn run_shell_command_streaming_with_sink(
        &self,
        cmd: &str,
        sink: Option<OutputSink>,
    ) -> Result<CommandOutput> {
        let adjusted = Self::apply_streaming_fixes(cmd);
        let wrapped = Self::wrap_streaming_command(&adjusted);
        let mut child = Command::new("bash")
            .arg("-c")
            .arg(wrapped)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let out_buf = Arc::new(Mutex::new(String::new()));
        let err_buf = Arc::new(Mutex::new(String::new()));

        let (line_tx, line_rx) = mpsc::channel::<OutputLine>();

        let stdout_handle = {
            let line_tx = line_tx.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let _ = line_tx.send(OutputLine::Stdout(line));
                    }
                }
            })
        };

        let stderr_handle = {
            let line_tx = line_tx.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let _ = line_tx.send(OutputLine::Stderr(line));
                    }
                }
            })
        };

        drop(line_tx);

        let dispatcher = {
            let out_buf = Arc::clone(&out_buf);
            let err_buf = Arc::clone(&err_buf);
            let sink = sink;
            thread::spawn(move || {
                let mut chunk: Vec<OutputLine> = Vec::with_capacity(3);
                for line in line_rx {
                    chunk.push(line);
                    if chunk.len() >= 3 {
                        flush_chunk(&chunk, &out_buf, &err_buf, sink.as_ref());
                        chunk.clear();
                    }
                }
                if !chunk.is_empty() {
                    flush_chunk(&chunk, &out_buf, &err_buf, sink.as_ref());
                }
            })
        };

        let status = child.wait()?;
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();
        let _ = dispatcher.join();

        let stdout = out_buf.lock().map(|s| s.clone()).unwrap_or_default();
        let stderr = err_buf.lock().map(|s| s.clone()).unwrap_or_default();
        let full_output = if stderr.trim().is_empty() {
            stdout.clone()
        } else {
            format!("{}\nErrors:\n{}", stdout, stderr)
        };

        Ok(CommandOutput {
            stdout,
            stderr,
            full_output,
            status,
        })
    }

    fn wrap_streaming_command(cmd: &str) -> String {
        if Self::has_in_path("script") {
            let escaped = Self::escape_single_quotes(cmd);
            return format!("script -q /dev/null -c '{}'", escaped);
        }

        if Self::has_in_path("stdbuf") {
            return format!("stdbuf -oL -eL {}", cmd);
        }

        cmd.to_string()
    }

    fn apply_streaming_fixes(cmd: &str) -> String {
        let mut tokens: Vec<&str> = cmd.split_whitespace().collect();
        if tokens.is_empty() {
            return cmd.to_string();
        }

        let has_pipe = cmd.contains('|');
        let has_journalctl = tokens
            .iter()
            .any(|t| *t == "journalctl" || t.ends_with("/journalctl"));

        if has_journalctl && !tokens.iter().any(|t| *t == "--no-pager") {
            tokens.push("--no-pager");
        }

        if has_journalctl && !has_pipe {
            return format!("SYSTEMD_PAGER=cat {}", tokens.join(" "));
        }

        tokens.join(" ")
    }

    fn has_in_path(bin: &str) -> bool {
        let Ok(path) = std::env::var("PATH") else {
            return false;
        };
        for entry in path.split(':') {
            let candidate = PathBuf::from(entry).join(bin);
            if candidate.exists() {
                return true;
            }
        }
        false
    }

    fn escape_single_quotes(input: &str) -> String {
        input.replace('\'', "'\\''")
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
}

struct CommandOutput {
    stdout: String,
    stderr: String,
    full_output: String,
    status: ExitStatus,
}

#[derive(Clone, Debug)]
pub(crate) enum OutputLine {
    Stdout(String),
    Stderr(String),
    ChunkEnd,
}

#[derive(Debug)]
pub(crate) struct OutputSink {
    pub tx: mpsc::Sender<OutputLine>,
    pub ack: mpsc::Receiver<()>,
}

fn flush_chunk(
    chunk: &[OutputLine],
    out_buf: &Arc<Mutex<String>>,
    err_buf: &Arc<Mutex<String>>,
    sink: Option<&OutputSink>,
) {
    println!("\n--- Output Chunk ---");
    for entry in chunk {
        match entry {
            OutputLine::Stdout(line) => {
                print_wrapped(line, false);
                if let Ok(mut buf) = out_buf.lock() {
                    buf.push_str(line);
                    buf.push('\n');
                }
                if let Some(sink) = sink {
                    let _ = sink.tx.send(OutputLine::Stdout(line.clone()));
                }
            }
            OutputLine::Stderr(line) => {
                print_wrapped(line, true);
                if let Ok(mut buf) = err_buf.lock() {
                    buf.push_str(line);
                    buf.push('\n');
                }
                if let Some(sink) = sink {
                    let _ = sink.tx.send(OutputLine::Stderr(line.clone()));
                }
            }
            OutputLine::ChunkEnd => {}
        }
    }

    if let Some(sink) = sink {
        let _ = sink.tx.send(OutputLine::ChunkEnd);
        let _ = sink.ack.recv();
    }
}

fn print_wrapped(line: &str, is_err: bool) {
    let prefix = "  ";
    let wrap_width = 80usize;
    let available = wrap_width.saturating_sub(prefix.len()).max(40);
    let clean = line.trim_end();
    let mut chars: Vec<char> = clean.chars().collect();

    if chars.is_empty() {
        if is_err {
            eprintln!("{}", prefix);
        } else {
            println!("{}", prefix);
        }
        return;
    }

    while !chars.is_empty() {
        let take = available.min(chars.len());
        let segment: String = chars.drain(..take).collect();
        if is_err {
            eprintln!("{}{}", prefix, segment);
        } else {
            println!("{}{}", prefix, segment);
        }
    }
}

impl From<std::process::Output> for CommandOutput {
    fn from(output: std::process::Output) -> Self {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let full_output = if stderr.is_empty() {
            stdout.clone()
        } else {
            format!("{}\nErrors:\n{}", stdout, stderr)
        };
        Self {
            stdout,
            stderr,
            full_output,
            status: output.status,
        }
    }
}
