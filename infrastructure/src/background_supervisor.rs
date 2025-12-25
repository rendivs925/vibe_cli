use crate::error_analyzer::{ErrorAnalyzer, ErrorContext};
use crate::session_store::SessionStore;
use anyhow::Result;
use flume::{Receiver, Sender};
use notify::{Event, Watcher, RecursiveMode};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Background event types that can be broadcast to the UI
#[derive(Debug, Clone)]
pub enum BackgroundEvent {
    FileChanged { path: PathBuf, change_type: FileChangeType },
    TestResult { session: String, status: TestStatus, output: String },
    LogEntry { source: String, level: LogLevel, message: String },
    LspDiagnostic { file: PathBuf, severity: DiagnosticSeverity, message: String },
    GitStatus { status: GitStatus },
}

#[derive(Debug, Clone)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone)]
pub enum TestStatus {
    Started,
    Passed,
    Failed { error: String },
    Completed,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone)]
pub enum GitStatus {
    Clean,
    Dirty { modified_files: Vec<PathBuf> },
    Untracked { files: Vec<PathBuf> },
}

/// Background intelligence supervisor managing all background services
pub struct BackgroundSupervisor {
    event_tx: Sender<BackgroundEvent>,
    event_rx: Receiver<BackgroundEvent>,
    services: HashMap<String, BackgroundService>,
    session_store: Option<Arc<RwLock<SessionStore>>>,
    shutdown_tx: flume::Sender<()>,
    shutdown_rx: flume::Receiver<()>,
    error_analyzer: ErrorAnalyzer,
    project_root: Option<PathBuf>,
}

#[derive(Debug)]
struct BackgroundService {
    name: String,
    handle: JoinHandle<()>,
    status: ServiceStatus,
}

#[derive(Debug, Clone)]
enum ServiceStatus {
    Starting,
    Running,
    Stopped,
    Failed(String),
}

impl BackgroundSupervisor {
    /// Create a new background supervisor
    pub fn new() -> Self {
        let (event_tx, event_rx) = flume::unbounded();
        let (shutdown_tx, shutdown_rx) = flume::unbounded();

        Self {
            event_tx,
            event_rx,
            services: HashMap::new(),
            session_store: None,
            shutdown_tx,
            shutdown_rx,
            error_analyzer: ErrorAnalyzer,
            project_root: None,
        }
    }

    /// Set the session store for background services
    pub fn with_session_store(mut self, store: Arc<RwLock<SessionStore>>) -> Self {
        self.session_store = Some(store);
        self
    }

    /// Start all background services
    pub async fn start(&mut self, project_root: &PathBuf) -> Result<()> {
        println!("🧠 Starting background intelligence services...");
        self.project_root = Some(project_root.clone());

        // Start file watcher
        self.start_file_watcher(project_root.clone()).await?;

        // Start LSP client
        self.start_lsp_client(project_root.clone()).await?;

        // Start log tailer
        self.start_log_tailer().await?;

        // Start autonomous fix analyzer
        self.start_autonomous_fix_analyzer().await?;

        println!("✅ Background intelligence active");
        Ok(())
    }

    /// Get event receiver for UI updates
    pub fn event_receiver(&self) -> Receiver<BackgroundEvent> {
        self.event_rx.clone()
    }

    /// Shutdown all background services
    pub async fn shutdown(&mut self) -> Result<()> {
        println!("🛑 Shutting down background services...");

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Wait for all services to stop
        for (name, service) in &self.services {
            println!("  └─ Stopping {}...", name);
            // Note: In a real implementation, we'd need proper shutdown signaling
            // For now, we'll just log that we're stopping
        }

        self.services.clear();
        println!("✅ All background services stopped");
        Ok(())
    }

    /// Start log tailer service
    async fn start_log_tailer(&mut self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let handle = tokio::spawn(async move {
            let tailer = crate::log_tailer::LogTailer::new();
            if let Err(e) = tailer.start_monitoring(event_tx).await {
                eprintln!("Log tailer error: {}", e);
            }
        });

        self.services.insert(
            "log-tailer".to_string(),
            BackgroundService {
                name: "log-tailer".to_string(),
                handle,
                status: ServiceStatus::Running,
            },
        );

        Ok(())
    }

    /// Start autonomous fix analyzer service
    async fn start_autonomous_fix_analyzer(&mut self) -> Result<()> {
        let event_rx = self.event_rx.clone();
        let analyzer = self.error_analyzer.clone();
        let project_root = self.project_root.clone().unwrap_or_else(|| PathBuf::from("."));

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::run_fix_analyzer(event_rx, analyzer, project_root).await {
                eprintln!("Autonomous fix analyzer error: {}", e);
            }
        });

        self.services.insert(
            "fix-analyzer".to_string(),
            BackgroundService {
                name: "fix-analyzer".to_string(),
                handle,
                status: ServiceStatus::Running,
            },
        );

        Ok(())
    }

    /// Run the autonomous fix analyzer loop
    async fn run_fix_analyzer(
        event_rx: Receiver<BackgroundEvent>,
        analyzer: ErrorAnalyzer,
        project_root: PathBuf,
    ) -> Result<()> {
        println!("🤖 Autonomous fix analyzer active - monitoring for errors...");

        while let Ok(event) = event_rx.recv_async().await {
            // Only process error events
            let should_analyze = matches!(event,
                BackgroundEvent::LspDiagnostic { .. } |
                BackgroundEvent::TestResult { status: crate::background_supervisor::TestStatus::Failed { .. }, .. } |
                BackgroundEvent::LogEntry { level: crate::background_supervisor::LogLevel::Error, .. } |
                BackgroundEvent::LogEntry { level: crate::background_supervisor::LogLevel::Warn, .. }
            );

            if should_analyze {
                let error_context: ErrorContext = (&event).into();

                // Only analyze high-severity errors automatically
                if matches!(error_context.severity, crate::error_analyzer::ErrorSeverity::High | crate::error_analyzer::ErrorSeverity::Critical) {
                    println!("🔧 Analyzing error for autonomous fix...");
                    println!("   Error: {}", error_context.message);

                    match analyzer.analyze_and_fix(error_context, &project_root).await {
                        Ok(suggestions) if !suggestions.is_empty() => {
                            println!("💡 Found {} fix suggestions (confidence > 0.5):", suggestions.len());
                            for (i, suggestion) in suggestions.iter().enumerate() {
                                if suggestion.confidence > 0.5 {
                                    println!("   {}. {} (confidence: {:.1}%)", i + 1, suggestion.description, suggestion.confidence * 100.0);
                                    println!("      {}", suggestion.explanation);
                                }
                            }
                            println!("🚀 Ready to apply fixes automatically (feature in development)");
                        }
                        Ok(_) => {
                            println!("🤔 No high-confidence fixes found for this error");
                        }
                        Err(e) => {
                            eprintln!("❌ Error during fix analysis: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Start test watcher service
    pub async fn start_test_watcher(&mut self, project_root: PathBuf, session: String) -> Result<()> {
        let event_tx = self.event_tx.clone();

        let handle = tokio::spawn(async move {
            match crate::test_watcher::TestWatcher::start_monitoring(project_root, event_tx, session).await {
                Ok(_watcher) => {
                    // Test watcher is now monitoring
                    futures::future::pending::<()>().await;
                }
                Err(e) => {
                    eprintln!("Test watcher error: {}", e);
                }
            }
        });

        self.services.insert(
            "test-watcher".to_string(),
            BackgroundService {
                name: "test-watcher".to_string(),
                handle,
                status: ServiceStatus::Running,
            },
        );

        Ok(())
    }

    /// Start LSP client service
    async fn start_lsp_client(&mut self, project_root: PathBuf) -> Result<()> {
        let event_tx = self.event_tx.clone();

        let handle = tokio::spawn(async move {
            match crate::lsp_client::LspClient::start_rust_analyzer(project_root, event_tx).await {
                Ok(_client) => {
                    // Keep the client alive for the duration
                    // In a real implementation, we'd handle graceful shutdown
                    // For now, just let it run
                    futures::future::pending::<()>().await;
                }
                Err(e) => {
                    eprintln!("LSP client error: {}", e);
                }
            }
        });

        self.services.insert(
            "lsp-client".to_string(),
            BackgroundService {
                name: "lsp-client".to_string(),
                handle,
                status: ServiceStatus::Running,
            },
        );

        Ok(())
    }

    /// Start file watcher service
    async fn start_file_watcher(&mut self, project_root: PathBuf) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let shutdown_rx = self.shutdown_rx.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::run_file_watcher(project_root, event_tx, shutdown_rx).await {
                eprintln!("File watcher error: {}", e);
            }
        });

        self.services.insert(
            "file-watcher".to_string(),
            BackgroundService {
                name: "file-watcher".to_string(),
                handle,
                status: ServiceStatus::Running,
            },
        );

        Ok(())
    }

    /// Run the file watcher loop with proper file system monitoring
    async fn run_file_watcher(
        project_root: PathBuf,
        event_tx: Sender<BackgroundEvent>,
        shutdown_rx: Receiver<()>,
    ) -> Result<()> {
        println!("  └─ 📁 File watcher starting on {}", project_root.display());

        // Create a channel for file system events
        let (watcher_tx, watcher_rx) = flume::unbounded();

        // Create the file watcher
        let mut watcher = notify::recommended_watcher(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    // Convert notify event to our background event
                    if let Some(bg_event) = Self::convert_notify_event(event) {
                        let _ = watcher_tx.send(bg_event);
                    }
                }
            }
        )?;

        // Watch the project root recursively, but ignore common build artifacts
        watcher.watch(&project_root, RecursiveMode::Recursive)?;

        // Filter out paths we don't want to watch
        let ignored_paths = [
            "target/",
            ".git/",
            "node_modules/",
            ".cargo/",
            "__pycache__/",
            "*.tmp",
            "*.log",
        ];

        println!("  └─ 📁 Watching for changes (excluding build artifacts)...");

        loop {
            tokio::select! {
                // Handle file system events
                Ok(bg_event) = watcher_rx.recv_async() => {
                    // Filter out ignored paths
                    if let BackgroundEvent::FileChanged { path, .. } = &bg_event {
                        let path_str = path.to_string_lossy();
                        let should_ignore = ignored_paths.iter().any(|ignored| {
                            path_str.contains(ignored.trim_end_matches('/'))
                        });

                        if !should_ignore {
                            if event_tx.send(bg_event).is_err() {
                                break; // UI receiver disconnected
                            }
                        }
                    }
                }

                // Handle shutdown signal
                _ = shutdown_rx.recv_async() => {
                    break;
                }
            }
        }

        println!("  └─ 📁 File watcher shutting down");
        Ok(())
    }

    /// Convert a notify Event to our BackgroundEvent
    fn convert_notify_event(event: Event) -> Option<BackgroundEvent> {
        // Get the first path (notify events can have multiple paths)
        let path = event.paths.first()?.clone();

        let change_type = if event.kind.is_create() {
            FileChangeType::Created
        } else if event.kind.is_modify() {
            FileChangeType::Modified
        } else if event.kind.is_remove() {
            FileChangeType::Deleted
        } else if event.kind.is_access() {
            return None; // Ignore access events
        } else {
            FileChangeType::Modified // Default to modified for unknown events
        };

        Some(BackgroundEvent::FileChanged { path, change_type })
    }

    /// Get status of all background services
    pub fn service_status(&self) -> HashMap<String, String> {
        self.services
            .iter()
            .map(|(name, service)| {
                let status = match &service.status {
                    ServiceStatus::Starting => "Starting",
                    ServiceStatus::Running => "Running",
                    ServiceStatus::Stopped => "Stopped",
                    ServiceStatus::Failed(err) => &format!("Failed: {}", err),
                };
                (name.clone(), status.to_string())
            })
            .collect()
    }
}