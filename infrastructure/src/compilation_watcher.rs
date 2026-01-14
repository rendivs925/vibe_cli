use anyhow::Result;
use flume::Sender;
use regex::Regex;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{self, Duration};

/// Compilation watcher that monitors cargo check output for real errors
pub struct CompilationWatcher;

impl CompilationWatcher {
    /// Start monitoring compilation errors
    pub async fn start_monitoring(
        project_root: PathBuf,
        event_tx: Sender<super::background_supervisor::BackgroundEvent>,
    ) -> Result<Self> {
        println!("  └─ 🔨 Compilation watcher disabled by default");
        
        // Compilation watcher disabled by default - no automatic monitoring
        // Only start if explicitly requested
        Ok(Self { project_root, event_tx })

        // Start monitoring in a separate task
        tokio::spawn(async move {
            if let Err(e) = Self::run_compilation_monitor(project_root, event_tx).await {
                eprintln!("Compilation watcher error: {}", e);
            }
        });

        println!("  └─ ✅ Compilation watcher started");
        Ok(Self)
    }

    /// Run the compilation monitoring loop
    async fn run_compilation_monitor(
        project_root: PathBuf,
        event_tx: Sender<super::background_supervisor::BackgroundEvent>,
    ) -> Result<()> {
        let mut last_check_time = std::time::Instant::now();

        loop {
            // Check compilation every 3 seconds
            time::sleep(Duration::from_secs(3)).await;

            // Only check if files have been modified recently
            if last_check_time.elapsed() > Duration::from_secs(2) {
                if let Err(e) = Self::check_compilation(&project_root, &event_tx).await {
                    eprintln!("Compilation check error: {}", e);
                }
                last_check_time = std::time::Instant::now();
            }
        }
    }

    /// Run cargo check and parse errors
    async fn check_compilation(
        project_root: &PathBuf,
        event_tx: &Sender<super::background_supervisor::BackgroundEvent>,
    ) -> Result<()> {
        let mut child = Command::new("cargo")
            .args(&["check", "--quiet", "--message-format=short"])
            .current_dir(project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Read stdout and stderr
        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut errors_found = Vec::new();

        // Process stdout
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            if let Some(error) = Self::parse_cargo_error(&line) {
                errors_found.push(error);
            }
        }

        // Process stderr
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            if let Some(error) = Self::parse_cargo_error(&line) {
                errors_found.push(error);
            }
        }

        // Wait for process to complete
        let _ = child.wait().await;

        // Send events for new errors
        for error in errors_found {
            let bg_event = super::background_supervisor::BackgroundEvent::LspDiagnostic {
                file: error.file,
                severity: super::background_supervisor::DiagnosticSeverity::Error,
                message: error.message,
            };

            let _ = event_tx.send(bg_event);
        }

        Ok(())
    }

    /// Parse a cargo error line
    fn parse_cargo_error(line: &str) -> Option<CargoError> {
        // Match patterns like:
        // error[E0425]: cannot find value `undefined_var` in this scope
        //   --> src/main.rs:10:5
        //   |
        // 10 |     undefined_var;
        //   |     ^^^^^^^^^^^^^ not found in this scope

        let error_pattern = Regex::new(r"error\[([^\]]+)\]: (.+)").ok()?;
        let file_pattern = Regex::new(r"--> ([^:]+):(\d+):(\d+)").ok()?;

        if let Some(error_caps) = error_pattern.captures(line) {
            let error_code = error_caps.get(1)?.as_str().to_string();
            let message = error_caps.get(2)?.as_str().to_string();

            // Try to find the file location in subsequent lines
            // For now, we'll use a generic location
            let file_path = PathBuf::from("src/main.rs"); // Default

            return Some(CargoError {
                code: error_code,
                message,
                file: file_path,
                line: None,
                column: None,
            });
        }

        None
    }
}

#[derive(Debug)]
struct CargoError {
    code: String,
    message: String,
    file: PathBuf,
    line: Option<u32>,
    column: Option<u32>,
}
