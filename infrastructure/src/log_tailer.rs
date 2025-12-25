use anyhow::Result;
use flume::Sender;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio::time::{self, Duration};

/// Log tailer that monitors multiple log files for errors and events
pub struct LogTailer {
    watched_files: HashMap<String, PathBuf>,
}

impl LogTailer {
    /// Create a new log tailer
    pub fn new() -> Self {
        Self {
            watched_files: HashMap::new(),
        }
    }

    /// Add a log file to monitor
    pub fn add_log_file(&mut self, name: String, path: PathBuf) {
        self.watched_files.insert(name, path);
    }

    /// Start monitoring all configured log files
    pub async fn start_monitoring(
        mut self,
        event_tx: Sender<super::background_supervisor::BackgroundEvent>,
    ) -> Result<()> {
        println!("  └─ 📜 Starting log tailer...");

        // Add common log file locations
        self.add_default_log_files();

        if self.watched_files.is_empty() {
            println!("  └─ ⚠️ No log files found to monitor");
            return Ok(());
        }

        // Start monitoring each log file
        let mut handles = Vec::new();

        let watched_files = std::mem::take(&mut self.watched_files);
        for (name, path) in watched_files {
            let event_tx_clone = event_tx.clone();
            let name_for_monitoring = name.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = Self::monitor_log_file(name_for_monitoring, path, event_tx_clone).await {
                    eprintln!("Log monitoring error for {}: {}", name, e);
                }
            });
            handles.push(handle);
        }

        println!("  └─ ✅ Log tailer started (monitoring {} files)", handles.len());

        // Keep the service alive
        futures::future::pending::<()>().await;
        Ok(())
    }

    /// Add default log file locations
    fn add_default_log_files(&mut self) {
        let default_logs = vec![
            ("system", PathBuf::from("/var/log/syslog")),
            ("auth", PathBuf::from("/var/log/auth.log")),
            ("kern", PathBuf::from("/var/log/kern.log")),
            ("journal", PathBuf::from("/var/log/journal")),
            ("app", PathBuf::from("./app.log")),
            ("error", PathBuf::from("./error.log")),
            ("debug", PathBuf::from("./debug.log")),
        ];

        for (name, path) in default_logs {
            if path.exists() {
                self.add_log_file(name.to_string(), path);
            }
        }
    }

    /// Monitor a single log file
    async fn monitor_log_file(
        name: String,
        path: PathBuf,
        event_tx: Sender<super::background_supervisor::BackgroundEvent>,
    ) -> Result<()> {
        println!("    └─ Monitoring {}: {}", name, path.display());

        loop {
            match File::open(&path).await {
                Ok(file) => {
                    let mut reader = BufReader::new(file);
                    let mut buffer = String::new();

                    // Seek to end of file initially
                    if let Ok(metadata) = tokio::fs::metadata(&path).await {
                        let _ = reader.seek(SeekFrom::Start(metadata.len())).await;
                    }

                    // Read new lines as they're written
                    loop {
                        buffer.clear();
                        match reader.read_line(&mut buffer).await {
                            Ok(0) => {
                                // EOF reached, wait a bit before checking again
                                time::sleep(Duration::from_millis(500)).await;
                            }
                            Ok(_) => {
                                let line = buffer.trim();
                                if !line.is_empty() {
                                    Self::process_log_line(&name, line, &event_tx);
                                }
                            }
                            Err(e) => {
                                eprintln!("Error reading log file {}: {}", name, e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    // File might not exist yet, wait and retry
                    eprintln!("Cannot open log file {}: {}. Will retry...", name, e);
                    time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Process a single log line and extract events
    fn process_log_line(source: &str, line: &str, event_tx: &Sender<super::background_supervisor::BackgroundEvent>) {
        // Define regex patterns for different log levels and error types
        let patterns = vec![
            // Panic/unrecoverable errors
            (r"panic|PANIC", super::background_supervisor::LogLevel::Error, "Application panic detected"),
            // Fatal errors
            (r"fatal|FATAL|critical|CRITICAL", super::background_supervisor::LogLevel::Error, "Critical error detected"),
            // Standard errors
            (r"error|ERROR|err|ERR", super::background_supervisor::LogLevel::Error, "Error detected"),
            // Warnings
            (r"warn|WARN|warning|WARNING", super::background_supervisor::LogLevel::Warn, "Warning detected"),
            // Info messages
            (r"info|INFO", super::background_supervisor::LogLevel::Info, "Info message"),
            // Debug messages
            (r"debug|DEBUG", super::background_supervisor::LogLevel::Debug, "Debug message"),
            // Stack traces
            (r"at\s+.*\.rs:\d+", super::background_supervisor::LogLevel::Error, "Stack trace detected"),
            // Network errors
            (r"connection.*failed|timeout|TIMEOUT", super::background_supervisor::LogLevel::Error, "Network error detected"),
            // Database errors
            (r"sql|SQL.*error|database|DATABASE.*error", super::background_supervisor::LogLevel::Error, "Database error detected"),
            // Authentication failures
            (r"auth.*failed|login.*failed|unauthorized|UNAUTHORIZED", super::background_supervisor::LogLevel::Warn, "Authentication issue detected"),
        ];

        for (pattern, level, description) in patterns {
            if let Ok(regex) = Regex::new(pattern) {
                if regex.is_match(line) {
                    // Extract a meaningful message from the line
                    let message = Self::extract_message(line, description);

                    let event = super::background_supervisor::BackgroundEvent::LogEntry {
                        source: source.to_string(),
                        level,
                        message,
                    };

                    let _ = event_tx.send(event);
                    break; // Only send one event per line
                }
            }
        }

        // Special handling for Rust-specific errors
        if line.contains("thread") && (line.contains("panicked") || line.contains("panic")) {
            let event = super::background_supervisor::BackgroundEvent::LogEntry {
                source: source.to_string(),
                level: super::background_supervisor::LogLevel::Error,
                message: format!("Thread panic: {}", Self::truncate_message(line, 200)),
            };
            let _ = event_tx.send(event);
        }
    }

    /// Extract a meaningful message from a log line
    fn extract_message(line: &str, description: &str) -> String {
        // Try to extract error details from common patterns
        if let Some(start) = line.find("error") {
            let error_part = &line[start..];
            if error_part.len() < 100 {
                return format!("{}: {}", description, error_part);
            }
        }

        // Fallback to truncated message
        Self::truncate_message(line, 150)
    }

    /// Truncate a message to a maximum length
    fn truncate_message(message: &str, max_len: usize) -> String {
        if message.len() <= max_len {
            message.to_string()
        } else {
            format!("{}...", &message[..max_len.saturating_sub(3)])
        }
    }
}