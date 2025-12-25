/// Real-time streaming capabilities for agent execution
///
/// Provides live output streaming, file change monitoring, and interactive controls:
/// - Streaming agent reasoning and tool execution
/// - Real-time file change notifications
/// - Interactive user controls (pause, resume, cancel)
/// - Multiple display modes (simple, rich, panels)

use crate::parallel_agent::{SubTask, SubTaskResult};
use shared::types::Result;
use std::path::PathBuf;
use std::io::Write;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

/// Streaming event types emitted during agent execution
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Agent is starting reasoning process
    ReasoningStart { task_description: String },
    /// Agent produces a reasoning step
    ReasoningStep { step_number: usize, content: String },
    /// Agent completes reasoning
    ReasoningComplete { total_steps: usize, duration_ms: u64 },

    /// Agent plans to execute a tool
    ToolPlanned { tool_name: String, description: String },
    /// Tool execution starts
    ToolStart { tool_name: String, parameters: String },
    /// Tool produces output (streaming)
    ToolOutput { tool_name: String, output_chunk: String, is_complete: bool },
    /// Tool execution completes
    ToolComplete { tool_name: String, success: bool, duration_ms: u64, error: Option<String> },

    /// File is being created/modified/deleted
    FileChange { path: PathBuf, change_type: FileChangeType, content_preview: Option<String> },

    /// Agent produces final result
    Result { content: String, confidence: f32 },

    /// Execution status updates
    Progress { completed_tasks: usize, total_tasks: usize, current_task: Option<String> },
    Status { message: String, level: StatusLevel },

    /// User interaction required
    UserPrompt { question: String, options: Vec<String> },
}

/// File change types
#[derive(Debug, Clone)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}

/// Status message levels
#[derive(Debug, Clone)]
pub enum StatusLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// User control commands during streaming
#[derive(Debug, Clone)]
pub enum UserControl {
    Pause,
    Resume,
    Cancel,
    ModifyGoal(String),
    AnswerPrompt(usize), // Index of selected option
}

/// Streaming display modes
#[derive(Debug, Clone)]
pub enum DisplayMode {
    /// Simple line-by-line output
    Simple,
    /// Rich TUI with panels and colors
    Rich,
    /// Minimal output for automation
    Minimal,
}

/// Streaming agent orchestrator
pub struct StreamingAgentOrchestrator {
    event_tx: mpsc::Sender<StreamEvent>,
    control_rx: mpsc::Receiver<UserControl>,
    display_mode: DisplayMode,
    is_paused: std::sync::Mutex<bool>,
}

impl StreamingAgentOrchestrator {
    /// Create new streaming orchestrator
    pub fn new(display_mode: DisplayMode) -> (Self, mpsc::Receiver<StreamEvent>, mpsc::Sender<UserControl>) {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (control_tx, control_rx) = mpsc::channel(10);

        let orchestrator = Self {
            event_tx,
            control_rx,
            display_mode,
            is_paused: std::sync::Mutex::new(false),
        };

        (orchestrator, event_rx, control_tx)
    }

    /// Get a clone of the event sender for external use
    pub fn event_sender(&self) -> mpsc::Sender<StreamEvent> {
        self.event_tx.clone()
    }

    /// Emit a streaming event
    pub async fn emit_event(&mut self, event: StreamEvent) -> Result<()> {
        // Check if execution is paused
        if *self.is_paused.lock().unwrap() {
            // Wait for resume signal
            while *self.is_paused.lock().unwrap() {
                tokio::time::sleep(Duration::from_millis(100)).await;
                // Check for control messages
                if let Ok(control) = self.control_rx.try_recv() {
                    match control {
                        UserControl::Resume => {
                            *self.is_paused.lock().unwrap() = false;
                            break;
                        }
                        UserControl::Cancel => {
                            return Err(anyhow::anyhow!("Execution cancelled by user"));
                        }
                        _ => {} // Handle other controls
                    }
                }
            }
        }

        self.event_tx.send(event).await?;
        Ok(())
    }

    /// Handle user control input
    pub async fn handle_control(&mut self, control: UserControl) -> Result<()> {
        match control {
            UserControl::Pause => {
                *self.is_paused.lock().unwrap() = true;
                self.emit_event(StreamEvent::Status {
                    message: "Execution paused".to_string(),
                    level: StatusLevel::Info,
                }).await?;
            }
            UserControl::Resume => {
                *self.is_paused.lock().unwrap() = false;
                self.emit_event(StreamEvent::Status {
                    message: "Execution resumed".to_string(),
                    level: StatusLevel::Info,
                }).await?;
            }
            UserControl::Cancel => {
                self.emit_event(StreamEvent::Status {
                    message: "Execution cancelled".to_string(),
                    level: StatusLevel::Warning,
                }).await?;
                return Err(anyhow::anyhow!("Execution cancelled by user"));
            }
            UserControl::ModifyGoal(new_goal) => {
                self.emit_event(StreamEvent::Status {
                    message: format!("Goal modified to: {}", new_goal),
                    level: StatusLevel::Info,
                }).await?;
            }
            UserControl::AnswerPrompt(_) => {
                // Handle prompt responses
            }
        }
        Ok(())
    }

    /// Execute tasks with streaming
    pub async fn execute_with_streaming<F, Fut>(
        &mut self,
        tasks: Vec<SubTask>,
        executor: F,
    ) -> Result<Vec<SubTaskResult>>
    where
        F: Fn(SubTask) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<SubTaskResult>> + Send + 'static,
    {
        let total_tasks = tasks.len();
        self.emit_event(StreamEvent::Progress {
            completed_tasks: 0,
            total_tasks,
            current_task: None,
        }).await?;

        let mut results = Vec::new();
        let mut completed_count = 0;

        for task in tasks {
            // Check for user controls
            if let Ok(control) = self.control_rx.try_recv() {
                self.handle_control(control).await?;
            }

            self.emit_event(StreamEvent::Progress {
                completed_tasks: completed_count,
                total_tasks,
                current_task: Some(task.description.clone()),
            }).await?;

            // Execute tool with streaming
            self.emit_event(StreamEvent::ToolStart {
                tool_name: "task_executor".to_string(),
                parameters: format!("task: {}", task.id),
            }).await?;

            let start_time = Instant::now();
            let result = executor(task.clone()).await?;
            let duration = start_time.elapsed().as_millis() as u64;

            self.emit_event(StreamEvent::ToolComplete {
                tool_name: "task_executor".to_string(),
                success: result.success,
                duration_ms: duration,
                error: result.error.clone(),
            }).await?;

            results.push(result);
            completed_count += 1;
        }

        self.emit_event(StreamEvent::Progress {
            completed_tasks: completed_count,
            total_tasks,
            current_task: None,
        }).await?;

        Ok(results)
    }
}

/// File change watcher for streaming file modifications
pub struct FileChangeWatcher {
    event_tx: mpsc::Sender<StreamEvent>,
}

impl FileChangeWatcher {
    pub fn new(event_tx: mpsc::Sender<StreamEvent>) -> Self {
        Self { event_tx }
    }

    /// Watch a directory for changes (simplified implementation)
    pub async fn watch_directory(&self, _path: &std::path::Path) -> Result<()> {
        // In a real implementation, this would use notify crate
        // For now, this is a placeholder
        Ok(())
    }

    /// Report a file change
    pub async fn report_change(&self, path: PathBuf, change_type: FileChangeType) -> Result<()> {
        let content_preview = match &change_type {
            FileChangeType::Modified => {
                // Try to read first few lines for preview
                std::fs::read_to_string(&path)
                    .ok()
                    .map(|content| {
                        content.lines().take(3).collect::<Vec<&str>>().join("\n")
                    })
            }
            _ => None,
        };

        self.event_tx.send(StreamEvent::FileChange {
            path,
            change_type,
            content_preview,
        }).await?;

        Ok(())
    }
}

/// Streaming display renderer
pub struct StreamingDisplay {
    mode: DisplayMode,
}

impl StreamingDisplay {
    pub fn new(mode: DisplayMode) -> Self {
        Self { mode }
    }

    /// Render a streaming event
    pub fn render_event(&self, event: &StreamEvent) {
        match self.mode {
            DisplayMode::Simple => self.render_simple(event),
            DisplayMode::Rich => self.render_rich(event),
            DisplayMode::Minimal => self.render_minimal(event),
        }
    }

    fn render_simple(&self, event: &StreamEvent) {
        match event {
            StreamEvent::ReasoningStep { step_number, content } => {
                println!("🤔 Step {}: {}", step_number, content);
            }
            StreamEvent::ToolStart { tool_name, parameters } => {
                println!("🔧 Starting: {} ({})", tool_name, parameters);
            }
            StreamEvent::ToolOutput { tool_name, output_chunk, is_complete } => {
                print!("{}", output_chunk);
                if *is_complete {
                    println!(" ✓ {} completed", tool_name);
                }
            }
            StreamEvent::FileChange { path, change_type, .. } => {
                let icon = match change_type {
                    FileChangeType::Created => "📄",
                    FileChangeType::Modified => "📝",
                    FileChangeType::Deleted => "🗑️",
                    FileChangeType::Renamed { .. } => "📋",
                };
                println!("{} {}: {}", icon, change_type_to_string(change_type), path.display());
            }
            StreamEvent::Progress { completed_tasks, total_tasks, current_task } => {
                if let Some(task) = current_task {
                    println!("📊 Progress: {}/{} - {}", completed_tasks, total_tasks, task);
                }
            }
            StreamEvent::Status { message, level } => {
                let icon = match level {
                    StatusLevel::Info => "ℹ️",
                    StatusLevel::Warning => "⚠️",
                    StatusLevel::Error => "❌",
                    StatusLevel::Success => "✅",
                };
                println!("{} {}", icon, message);
            }
            _ => {} // Other events can be handled as needed
        }
    }

    fn render_rich(&self, event: &StreamEvent) {
        // Rich rendering with colors and formatting would go here
        // For now, delegate to simple
        self.render_simple(event);
    }

    fn render_minimal(&self, event: &StreamEvent) {
        // Minimal output for automation
        match event {
            StreamEvent::ToolComplete { tool_name, success, .. } => {
                if *success {
                    print!(".");
                } else {
                    print!("!");
                }
                std::io::stdout().flush();
            }
            StreamEvent::Progress { completed_tasks, total_tasks, .. } => {
                print!("\r{}/{}", completed_tasks, total_tasks);
                std::io::stdout().flush();
            }
            _ => {} // Silent for other events
        }
    }
}

fn change_type_to_string(change_type: &FileChangeType) -> &'static str {
    match change_type {
        FileChangeType::Created => "created",
        FileChangeType::Modified => "modified",
        FileChangeType::Deleted => "deleted",
        FileChangeType::Renamed { .. } => "renamed",
    }
}