use anyhow::Result;
use flume::Sender;
use regex::Regex;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Test watcher that monitors cargo test output in real-time
pub struct TestWatcher;

impl TestWatcher {
    /// Start monitoring cargo test output
    pub async fn start_monitoring(
        project_root: PathBuf,
        event_tx: Sender<super::background_supervisor::BackgroundEvent>,
        session: String,
    ) -> Result<Self> {
        println!("  └─ 🧪 Starting test watcher...");

        // Start cargo test process
        let mut child = Command::new("cargo")
            .args(&["test", "--", "--nocapture"])
            .current_dir(&project_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start cargo test: {}", e))?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Monitor stdout
        let event_tx_clone = event_tx.clone();
        let session_clone = session.clone();
        tokio::spawn(async move {
            Self::monitor_output(stdout, event_tx_clone, session_clone, false).await;
        });

        // Monitor stderr
        let event_tx_clone = event_tx.clone();
        let session_clone = session.clone();
        tokio::spawn(async move {
            Self::monitor_output(stderr, event_tx_clone, session_clone, true).await;
        });

        // Monitor process completion
        let event_tx_clone = event_tx.clone();
        let session_clone = session.clone();
        tokio::spawn(async move {
            match child.wait().await {
                Ok(status) => {
                    let result = if status.success() {
                        super::background_supervisor::TestStatus::Completed
                    } else {
                        super::background_supervisor::TestStatus::Failed {
                            error: format!(
                                "Tests failed with exit code {}",
                                status.code().unwrap_or(-1)
                            ),
                        }
                    };

                    let event = super::background_supervisor::BackgroundEvent::TestResult {
                        session: session_clone,
                        status: result,
                        output: format!("Test run completed with status: {}", status),
                    };

                    let _ = event_tx_clone.send(event);
                }
                Err(e) => {
                    let event = super::background_supervisor::BackgroundEvent::TestResult {
                        session: session_clone,
                        status: super::background_supervisor::TestStatus::Failed {
                            error: format!("Test process error: {}", e),
                        },
                        output: "Test execution failed".to_string(),
                    };

                    let _ = event_tx_clone.send(event);
                }
            }
        });

        println!("  └─ ✅ Test watcher started");
        Ok(Self)
    }

    /// Monitor output stream and parse test results
    async fn monitor_output(
        stream: impl tokio::io::AsyncRead + Unpin,
        event_tx: Sender<super::background_supervisor::BackgroundEvent>,
        session: String,
        is_stderr: bool,
    ) {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();

        // Regex patterns for test output
        let test_start = Regex::new(r"testing (.+)").unwrap();
        let test_pass = Regex::new(r"test (.+) \.\.\. ok").unwrap();
        let test_fail = Regex::new(r"test (.+) \.\.\. FAILED").unwrap();
        let summary = Regex::new(r"test result: (.+)\. (\d+) passed; (\d+) failed;").unwrap();

        while let Ok(Some(line)) = lines.next_line().await {
            // Send started event for test functions
            if let Some(captures) = test_start.captures(&line) {
                if let Some(test_name) = captures.get(1) {
                    let event = super::background_supervisor::BackgroundEvent::TestResult {
                        session: session.clone(),
                        status: super::background_supervisor::TestStatus::Started,
                        output: format!("Running test: {}", test_name.as_str()),
                    };
                    let _ = event_tx.send(event);
                }
            }

            // Send pass/fail events
            if let Some(captures) = test_pass.captures(&line) {
                if let Some(test_name) = captures.get(1) {
                    let event = super::background_supervisor::BackgroundEvent::TestResult {
                        session: session.clone(),
                        status: super::background_supervisor::TestStatus::Passed,
                        output: format!("✅ {} passed", test_name.as_str()),
                    };
                    let _ = event_tx.send(event);
                }
            }

            if let Some(captures) = test_fail.captures(&line) {
                if let Some(test_name) = captures.get(1) {
                    let event = super::background_supervisor::BackgroundEvent::TestResult {
                        session: session.clone(),
                        status: super::background_supervisor::TestStatus::Failed {
                            error: format!("❌ {} failed", test_name.as_str()),
                        },
                        output: format!("Test failure: {}", test_name.as_str()),
                    };
                    let _ = event_tx.send(event);
                }
            }

            // Send summary information
            if let Some(captures) = summary.captures(&line) {
                if let (Some(result), Some(passed), Some(failed)) =
                    (captures.get(1), captures.get(2), captures.get(3))
                {
                    let output = format!(
                        "{}: {} passed, {} failed",
                        result.as_str(),
                        passed.as_str(),
                        failed.as_str()
                    );

                    let status = if failed.as_str() == "0" {
                        super::background_supervisor::TestStatus::Passed
                    } else {
                        super::background_supervisor::TestStatus::Failed {
                            error: "Some tests failed".to_string(),
                        }
                    };

                    let event = super::background_supervisor::BackgroundEvent::TestResult {
                        session: session.clone(),
                        status,
                        output,
                    };

                    let _ = event_tx.send(event);
                }
            }

            // For stderr or unrecognized output, send as general info
            if is_stderr || (!line.contains("running") && !line.contains("...") && line.len() > 10)
            {
                let event = super::background_supervisor::BackgroundEvent::TestResult {
                    session: session.clone(),
                    status: super::background_supervisor::TestStatus::Started,
                    output: line,
                };
                let _ = event_tx.send(event);
            }
        }
    }
}
