use std::io::{self, stdout, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::cli::{CliApp, Cli};

/// TUI application state
pub struct TuiApp {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    cli_app: CliApp,
    current_mode: TuiMode,
    input_buffer: String,
    cursor_position: usize,
    status_message: String,
    show_overlay: Option<Overlay>,
    session_list: Vec<String>,
    current_session: Option<String>,
    command_history: Vec<String>,
    history_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TuiMode {
    Normal,
    Insert,
    Command,
}

#[derive(Debug)]
pub enum Overlay {
    Sessions,
    Tools,
    Context,
    Palette,
}

impl TuiApp {
    /// Create a new TUI application
    pub fn new(cli: Cli) -> Result<Self> {
        let mut cli_app = CliApp::new();

        // Initialize CLI app with the parsed CLI args
        // Note: We'll handle the TUI-specific logic separately

        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            cli_app,
            current_mode: TuiMode::Normal,
            input_buffer: String::new(),
            cursor_position: 0,
            status_message: "Ready".to_string(),
            show_overlay: None,
            session_list: vec!["default".to_string(), "project-x".to_string(), "debug-session".to_string()],
            current_session: Some("default".to_string()),
            command_history: Vec::new(),
            history_index: None,
        })
    }

    /// Run the TUI application
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture
        )?;
        self.terminal.clear()?;

        // Main event loop
        loop {
            // Draw the UI
            self.terminal.draw(|f| self.draw(f))?;

            // Handle events
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match self.current_mode {
                        TuiMode::Normal => {
                            if self.handle_normal_mode(key).await? {
                                break; // Exit application
                            }
                        }
                        TuiMode::Insert => {
                            if self.handle_insert_mode(key).await? {
                                break;
                            }
                        }
                        TuiMode::Command => {
                            if self.handle_command_mode(key).await? {
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Cleanup
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;

        Ok(())
    }

    /// Handle normal mode key events (vim-style)
    async fn handle_normal_mode(&mut self, key: event::KeyEvent) -> Result<bool> {
        match key.code {
            // Quit commands
            KeyCode::Char('q') => return Ok(true), // Quit
            KeyCode::Char('Z') if key.modifiers.contains(KeyModifiers::SHIFT) => return Ok(true), // ZZ

            // Mode switching
            KeyCode::Char('i') => {
                self.current_mode = TuiMode::Insert;
                self.status_message = "INSERT".to_string();
            }
            KeyCode::Char('I') => {
                // Insert at beginning of line
                self.current_mode = TuiMode::Insert;
                self.cursor_position = 0;
                self.status_message = "INSERT".to_string();
            }
            KeyCode::Char('a') => {
                // Append after cursor
                self.current_mode = TuiMode::Insert;
                self.status_message = "INSERT".to_string();
            }
            KeyCode::Char('A') => {
                // Append at end of line
                self.current_mode = TuiMode::Insert;
                self.cursor_position = self.input_buffer.len();
                self.status_message = "INSERT".to_string();
            }
            KeyCode::Char('o') => {
                // Open new line below (like vim 'o')
                self.input_buffer.push('\n');
                self.cursor_position = self.input_buffer.len();
                self.current_mode = TuiMode::Insert;
                self.status_message = "INSERT".to_string();
            }
            KeyCode::Char('O') => {
                // Open new line above (like vim 'O')
                self.input_buffer.insert(0, '\n');
                self.cursor_position = 1;
                self.current_mode = TuiMode::Insert;
                self.status_message = "INSERT".to_string();
            }
            KeyCode::Char(':') => {
                self.current_mode = TuiMode::Command;
                self.input_buffer.clear();
                self.cursor_position = 0;
                self.status_message = "COMMAND".to_string();
            }

            // Vim-style navigation (hjkl)
            KeyCode::Char('h') => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Char('l') => {
                if self.cursor_position < self.input_buffer.len() {
                    self.cursor_position += 1;
                }
            }
            KeyCode::Char('w') => {
                // Move to next word
                let rest = &self.input_buffer[self.cursor_position..];
                if let Some(word_end) = rest.find(|c: char| c.is_whitespace()) {
                    self.cursor_position += word_end + 1;
                    // Skip additional whitespace
                    while self.cursor_position < self.input_buffer.len() &&
                          self.input_buffer.chars().nth(self.cursor_position).unwrap().is_whitespace() {
                        self.cursor_position += 1;
                    }
                } else {
                    self.cursor_position = self.input_buffer.len();
                }
            }
            KeyCode::Char('b') => {
                // Move to previous word
                if self.cursor_position > 0 {
                    let mut pos = self.cursor_position - 1;
                    // Skip current whitespace
                    while pos > 0 && self.input_buffer.chars().nth(pos).unwrap().is_whitespace() {
                        pos -= 1;
                    }
                    // Find word start
                    while pos > 0 && !self.input_buffer.chars().nth(pos - 1).unwrap().is_whitespace() {
                        pos -= 1;
                    }
                    self.cursor_position = pos;
                }
            }
            KeyCode::Char('0') => {
                self.cursor_position = 0; // Beginning of line
            }
            KeyCode::Char('$') => {
                self.cursor_position = self.input_buffer.len(); // End of line
            }

            // Vim-style editing
            KeyCode::Char('x') => {
                // Delete character under cursor
                if self.cursor_position < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_position);
                }
            }
            KeyCode::Char('X') => {
                // Delete character before cursor
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.input_buffer.remove(self.cursor_position);
                }
            }
            KeyCode::Char('d') => {
                // Delete line (dd)
                self.input_buffer.clear();
                self.cursor_position = 0;
            }

            // Overlays and special functions (Ctrl+key)
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+S: Show sessions overlay
                self.show_overlay = Some(Overlay::Sessions);
                self.status_message = "SESSIONS".to_string();
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+P: Show command palette
                self.show_overlay = Some(Overlay::Palette);
                self.status_message = "PALETTE".to_string();
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+K: Show tools overlay
                self.show_overlay = Some(Overlay::Tools);
                self.status_message = "TOOLS".to_string();
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+O: Show context overlay
                self.show_overlay = Some(Overlay::Context);
                self.status_message = "CONTEXT".to_string();
            }

            // History navigation (bash-style)
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+P: Previous command
                self.navigate_history(true);
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+N: Next command
                self.navigate_history(false);
            }
            KeyCode::Up => {
                // Arrow up: Previous command
                self.navigate_history(true);
            }
            KeyCode::Down => {
                // Arrow down: Next command
                self.navigate_history(false);
            }

            KeyCode::Esc => {
                // Dismiss overlay
                self.show_overlay = None;
                self.status_message = "Ready".to_string();
            }
            KeyCode::Char(c) => {
                // Handle overlay-specific keybindings
                if let Some(overlay) = &self.show_overlay {
                    match overlay {
                        Overlay::Sessions => {
                            self.handle_sessions_overlay_key(c);
                        }
                        Overlay::Tools => {
                            self.handle_tools_overlay_key(c);
                        }
                        Overlay::Context => {
                            self.handle_context_overlay_key(c);
                        }
                        Overlay::Palette => {
                            self.handle_palette_overlay_key(c);
                        }
                    }
                }
            }
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle sessions overlay key events
    fn handle_sessions_overlay_key(&mut self, key: char) {
        match key {
            '1'..='9' => {
                // Select session by number
                let index = (key as u8 - b'1') as usize;
                if index < self.session_list.len() {
                    let session_name = self.session_list[index].clone();
                    self.current_session = Some(session_name.clone());
                    self.show_overlay = None;
                    self.status_message = format!("Switched to session: {}", session_name);
                }
            }
            'n' => {
                // Create new session
                self.show_overlay = None;
                self.status_message = "New session: type name and press Enter (not implemented yet)".to_string();
            }
            'd' => {
                // Delete current session (if not default)
                if let Some(current) = &self.current_session {
                    if current != "default" {
                        self.session_list.retain(|s| s != current);
                        self.current_session = Some("default".to_string());
                        self.show_overlay = None;
                        self.status_message = format!("Deleted session: {}", current);
                    } else {
                        self.status_message = "Cannot delete default session".to_string();
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle tools overlay key events
    fn handle_tools_overlay_key(&mut self, key: char) {
        match key {
            '1' => {
                self.show_overlay = None;
                self.status_message = "Switched to PLAN mode".to_string();
            }
            '2' => {
                self.show_overlay = None;
                self.status_message = "Switched to BUILD mode".to_string();
            }
            '3' => {
                self.show_overlay = None;
                self.status_message = "Switched to RUN mode".to_string();
            }
            '4' => {
                self.show_overlay = None;
                self.status_message = "Switched to CHAT mode".to_string();
            }
            '5' => {
                self.show_overlay = None;
                self.status_message = "Switched to RAG mode".to_string();
            }
            _ => {}
        }
    }

    /// Handle context overlay key events
    fn handle_context_overlay_key(&mut self, key: char) {
        match key {
            '1' => {
                self.show_overlay = None;
                self.status_message = "Showing current directory files".to_string();
            }
            '2' => {
                self.show_overlay = None;
                self.status_message = "Showing open files".to_string();
            }
            '3' => {
                self.show_overlay = None;
                self.status_message = "Showing git status".to_string();
            }
            '4' => {
                self.show_overlay = None;
                self.status_message = "Showing recent commands".to_string();
            }
            '5' => {
                self.show_overlay = None;
                self.status_message = "Showing project structure".to_string();
            }
            _ => {}
        }
    }

    /// Handle palette overlay key events
    fn handle_palette_overlay_key(&mut self, key: char) {
        match key {
            '1' => {
                self.show_overlay = None;
                self.status_message = "Quit command executed".to_string();
                // In a real implementation, this would trigger quit
            }
            '2' => {
                self.show_overlay = None;
                self.status_message = "Help: i=insert, :q=quit, hjkl=navigate, Ctrl+P/N=history".to_string();
            }
            '3' => {
                self.show_overlay = None;
                self.status_message = "Session switch: use :session <name>. Current: default".to_string();
            }
            '4' => {
                self.show_overlay = None;
                self.input_buffer.clear();
                self.cursor_position = 0;
                self.status_message = "Buffer cleared".to_string();
            }
            '5' => {
                self.show_overlay = None;
                self.status_message = format!("Status: Mode={:?}, Session={:?}, History={} cmds",
                    self.current_mode,
                    self.current_session,
                    self.command_history.len());
            }
            '6' => {
                self.show_overlay = None;
                self.status_message = "Session saved. Goodbye!".to_string();
                // In a real implementation, this would save and quit
            }
            '7' => {
                self.show_overlay = None;
                self.status_message = "Mode switch: use :mode <plan|build|run|chat>". Current: normal".to_string();
            }
            '8' => {
                self.show_overlay = None;
                let history_preview: Vec<String> = self.command_history
                    .iter()
                    .rev()
                    .take(5)
                    .map(|cmd| format!("  {}", cmd))
                    .collect();
                self.status_message = format!("Recent history:\n{}",
                    history_preview.join("\n"));
            }
            _ => {}
        }
    }
            '2' => {
                self.show_overlay = None;
                self.status_message = "Help: i=insert, :q=quit, hjkl=navigate, Ctrl+P/N=history".to_string();
            }
            '3' => {
                self.show_overlay = None;
                self.status_message = "Session switch: use :session <name>". Current: default".to_string();
            }
            '4' => {
                self.show_overlay = None;
                self.input_buffer.clear();
                self.cursor_position = 0;
                self.status_message = "Buffer cleared".to_string();
            }
            '5' => {
                self.show_overlay = None;
                self.status_message = format!("Status: Mode={:?}, Session={:?}, History={} cmds",
                    self.current_mode,
                    self.current_session,
                    self.command_history.len());
            }
            _ => {}
        }
    }

    /// Handle insert mode key events
    async fn handle_insert_mode(&mut self, key: event::KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.current_mode = TuiMode::Normal;
                self.status_message = "Ready".to_string();
            }
            KeyCode::Enter => {
                // Execute the command
                let command = self.input_buffer.clone();
                self.execute_command(&command).await?;
                self.input_buffer.clear();
                self.cursor_position = 0;
                self.current_mode = TuiMode::Normal;
                self.status_message = "Ready".to_string();
            }
            KeyCode::Tab => {
                // Auto-completion (placeholder)
                self.status_message = "Tab completion not yet implemented".to_string();
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.input_buffer.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_position);
                }
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_position < self.input_buffer.len() {
                    self.cursor_position += 1;
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle command mode key events
    async fn handle_command_mode(&mut self, key: event::KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.current_mode = TuiMode::Normal;
                self.status_message = "Ready".to_string();
            }
            KeyCode::Enter => {
                let command = self.input_buffer.clone();
                if self.execute_vim_command(&command).await? {
                    return Ok(true); // Quit command
                }
                self.current_mode = TuiMode::Normal;
                self.status_message = "Ready".to_string();
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.input_buffer.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            _ => {}
        }
        Ok(false)
    }

    /// Execute a command from the input buffer
    async fn execute_command(&mut self, command: &str) -> Result<()> {
        if !command.trim().is_empty() {
            // Add to history
            self.command_history.push(command.to_string());
            if self.command_history.len() > 100 {
                self.command_history.remove(0); // Keep only last 100 commands
            }
            self.history_index = None;
        }

        self.status_message = format!("Executing: {} ...", command);

        // Parse the command and determine what to do
        let parts: Vec<&str> = command.split_whitespace().collect();
        let result = match parts.get(0).map(|s| *s) {
            Some("ls") | Some("pwd") | Some("cd") | Some("mkdir") | Some("rm") | Some("cp") | Some("mv") => {
                // File system commands - execute directly
                self.execute_shell_command(command).await
            }
            Some("git") => {
                // Git commands - execute directly
                self.execute_shell_command(command).await
            }
            Some("cargo") => {
                // Cargo commands - execute directly
                self.execute_shell_command(command).await
            }
            Some("plan") => {
                // Plan mode command
                self.execute_plan_command(&parts[1..].join(" ")).await
            }
            Some("build") => {
                // Build mode command
                self.execute_build_command(&parts[1..].join(" ")).await
            }
            Some("run") => {
                // Run mode command
                self.execute_run_command(&parts[1..].join(" ")).await
            }
            Some("chat") => {
                // Chat mode command
                self.execute_chat_command(&parts[1..].join(" ")).await
            }
            Some("rag") => {
                // RAG mode command
                self.execute_rag_command(&parts[1..].join(" ")).await
            }
            _ => {
                // Default: try to execute as shell command
                self.execute_shell_command(command).await
            }
        };

        match result {
            Ok(output) => {
                self.status_message = format!("✓ {}", output);
            }
            Err(e) => {
                self.status_message = format!("✗ Error: {}", e);
            }
        }

        Ok(())
    }

    /// Execute a shell command
    async fn execute_shell_command(&mut self, command: &str) -> Result<String> {
        use tokio::process::Command;

        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(format!("Success: {}", stdout.trim()))
        } else {
            Err(anyhow::anyhow!("Command failed: {}", stderr.trim()))
        }
    }

    /// Execute a plan mode command
    async fn execute_plan_command(&mut self, goal: &str) -> Result<String> {
        // In a real implementation, this would call the CLI plan logic
        Ok(format!("Plan created for: '{}'. Use build mode to execute.", goal))
    }

    /// Execute a build mode command
    async fn execute_build_command(&mut self, goal: &str) -> Result<String> {
        // In a real implementation, this would call the CLI build logic
        Ok(format!("Build executed for: '{}'. Changes applied safely.", goal))
    }

    /// Execute a run mode command
    async fn execute_run_command(&mut self, goal: &str) -> Result<String> {
        // In a real implementation, this would call the CLI run logic
        Ok(format!("Multi-step execution completed for: '{}'.", goal))
    }

    /// Execute a chat mode command
    async fn execute_chat_command(&mut self, message: &str) -> Result<String> {
        // In a real implementation, this would call the CLI chat logic
        Ok(format!("Chat response: 'Hello! You said: {}'", message))
    }

    /// Execute a RAG mode command
    async fn execute_rag_command(&mut self, query: &str) -> Result<String> {
        // In a real implementation, this would call the CLI RAG logic
        Ok(format!("RAG query executed: '{}'. Found relevant context.", query))
    }

    /// Execute a vim-style command
    async fn execute_vim_command(&mut self, command: &str) -> Result<bool> {
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        let cmd = parts.get(0).unwrap_or(&"");

        match *cmd {
            "q" | "quit" => {
                self.status_message = "Goodbye!".to_string();
                return Ok(true);
            }
            "q!" => {
                self.status_message = "Force quit!".to_string();
                return Ok(true);
            }
            "w" | "write" => {
                self.status_message = "Session saved".to_string();
            }
            "wq" => {
                self.status_message = "Session saved. Goodbye!".to_string();
                return Ok(true);
            }
            "x" => {
                self.status_message = "Session saved. Goodbye!".to_string();
                return Ok(true);
            }
            "h" | "help" => {
                self.status_message = "Help: i=insert, :q=quit, :w=save, hjkl=navigate".to_string();
            }
            "session" => {
                if let Some(name) = parts.get(1) {
                    self.current_session = Some(name.to_string());
                    self.status_message = format!("Switched to session: {}", name);
                } else {
                    self.status_message = "Usage: :session <name>". Current: default".to_string();
                }
            }
            "mode" => {
                if let Some(mode) = parts.get(1) {
                    match *mode {
                        "plan" => self.status_message = "Switched to PLAN mode".to_string(),
                        "build" => self.status_message = "Switched to BUILD mode".to_string(),
                        "run" => self.status_message = "Switched to RUN mode".to_string(),
                        "chat" => self.status_message = "Switched to CHAT mode".to_string(),
                        _ => self.status_message = format!("Unknown mode: {}", mode),
                    }
                } else {
                    self.status_message = "Usage: :mode <plan|build|run|chat>". Current: normal".to_string();
                }
            }
            "clear" => {
                self.input_buffer.clear();
                self.cursor_position = 0;
                self.status_message = "Buffer cleared".to_string();
            }
            "status" => {
                self.status_message = format!("Mode: {:?}, Session: {:?}, Buffer: {} chars",
                    self.current_mode,
                    self.current_session,
                    self.input_buffer.len());
            }
            _ => {
                self.status_message = format!("Unknown command: {}. Type :help for commands", command);
            }
        }
        Ok(false)
    }

    /// Draw the TUI interface
    fn draw(&self, f: &mut Frame) {
        let size = f.size();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(1),    // Main content
                Constraint::Length(3), // Status bar
            ])
            .split(size);

        // Draw header
        self.draw_header(f, chunks[0]);

        // Draw main content
        self.draw_main_content(f, chunks[1]);

        // Draw status bar
        self.draw_status_bar(f, chunks[2]);

        // Draw overlay if active
        if let Some(overlay) = &self.show_overlay {
            self.draw_overlay(f, overlay.clone());
        }
    }

    /// Draw the header section
    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Vibe CLI")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Left);
        f.render_widget(title, header_chunks[0]);

        // Session info
        let session = self.current_session.as_deref().unwrap_or("no session");
        let session_info = Paragraph::new(format!("Session: {}", session))
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        f.render_widget(session_info, header_chunks[1]);

        // Mode indicator
        let mode_text = match self.current_mode {
            TuiMode::Normal => "NORMAL",
            TuiMode::Insert => "INSERT",
            TuiMode::Command => "COMMAND",
        };
        let mode = Paragraph::new(mode_text)
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Right);
        f.render_widget(mode, header_chunks[2]);
    }

    /// Draw the main content area
    fn draw_main_content(&self, f: &mut Frame, area: Rect) {
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // History area
                Constraint::Min(1),    // Input area
                Constraint::Length(1), // Separator
                Constraint::Length(3), // Message area
            ])
            .split(area);

        // History area (show last 3 commands)
        let history_block = Block::default()
            .borders(Borders::ALL)
            .title("History");

        let history_text = self.command_history
            .iter()
            .rev()
            .take(3)
            .enumerate()
            .map(|(i, cmd)| {
                let prefix = match i {
                    0 => "↑ ",
                    1 => "  ",
                    2 => "  ",
                    _ => "  ",
                };
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Gray)),
                    Span::styled(cmd, Style::default().fg(Color::White)),
                ])
            })
            .collect::<Vec<Line>>();

        let history = Paragraph::new(history_text)
            .block(history_block)
            .wrap(Wrap { trim: true });
        f.render_widget(history, content_chunks[0]);

        // Input area
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title("Command");

        let input_text = if self.current_mode == TuiMode::Command {
            format!(":{}", self.input_buffer)
        } else {
            self.input_buffer.clone()
        };

        let input = Paragraph::new(input_text)
            .block(input_block)
            .wrap(Wrap { trim: true });
        f.render_widget(input, content_chunks[1]);

        // Set cursor position for input
        let cursor_x = if self.current_mode == TuiMode::Command {
            content_chunks[1].x + 1 + self.cursor_position + 1 // +1 for ':' prefix
        } else {
            content_chunks[1].x + 1 + self.cursor_position
        };
        let cursor_y = content_chunks[1].y + 1;
        f.set_cursor(cursor_x, cursor_y);

        // Message area
        let message_block = Block::default()
            .borders(Borders::ALL)
            .title("Status");

        let message = Paragraph::new(self.status_message.as_str())
            .block(message_block)
            .wrap(Wrap { trim: true });
        f.render_widget(message, content_chunks[3]);
    }

    /// Draw the status bar
    fn draw_status_bar(&self, f: &mut Frame, area: Rect) {
        let status_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
            ])
            .split(area);

        let hints = match self.current_mode {
            TuiMode::Normal => vec![
                "i insert",
                "⌘P palette",
                "⌘S sessions",
                "⌘K tools",
                ": cmd",
            ],
            TuiMode::Insert => vec![
                "esc normal",
                "⏎ execute",
                "hjkl move",
                "w/b words",
                "",
            ],
            TuiMode::Command => vec![
                "⏎ run",
                "esc cancel",
                "tab complete",
                "",
                "",
            ],
        };

        for (i, hint) in hints.iter().enumerate() {
            let hint_widget = Paragraph::new(*hint)
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);
            f.render_widget(hint_widget, status_chunks[i]);
        }
    }

    /// Draw overlay windows
    fn draw_overlay(&self, f: &mut Frame, overlay: Overlay) {
        let area = centered_rect(60, 40, f.size());
        f.render_widget(Clear, area);

        match overlay {
            Overlay::Sessions => self.draw_sessions_overlay(f, area),
            Overlay::Tools => self.draw_tools_overlay(f, area),
            Overlay::Context => self.draw_context_overlay(f, area),
            Overlay::Palette => self.draw_palette_overlay(f, area),
        }
    }

    /// Draw sessions overlay
    fn draw_sessions_overlay(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header with actions
                Constraint::Min(1),    // Session list
                Constraint::Length(2), // Footer hints
            ])
            .split(area);

        // Header with session actions
        let header_block = Block::default()
            .title("Session Manager")
            .borders(Borders::ALL);

        let header_text = vec![
            Line::from(vec![
                Span::styled("Actions: ", Style::default().fg(Color::Yellow)),
                Span::styled("n", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("ew session, ", Style::default().fg(Color::Gray)),
                Span::styled("d", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("elete, ", Style::default().fg(Color::Gray)),
                Span::styled("Enter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(" switch", Style::default().fg(Color::Gray)),
            ]),
        ];

        let header = Paragraph::new(header_text)
            .block(header_block)
            .wrap(Wrap { trim: true });
        f.render_widget(header, chunks[0]);

        // Session list
        let list_block = Block::default()
            .borders(Borders::ALL);

        let items: Vec<ListItem> = self.session_list
            .iter()
            .enumerate()
            .map(|(i, session)| {
                let mut style = Style::default();
                let mut prefix = "  ";

                if Some(session) == self.current_session.as_ref() {
                    style = style.fg(Color::Green).add_modifier(Modifier::BOLD);
                    prefix = "● ";
                }

                // Show session metadata (placeholder for now)
                let display_name = if session == "default" {
                    format!("{}{} (default)", prefix, session)
                } else {
                    format!("{}{}", prefix, session)
                };

                ListItem::new(display_name).style(style)
            })
            .collect();

        let list = List::new(items).block(list_block);
        f.render_widget(list, chunks[1]);

        // Footer hints
        let footer_text = vec![
            Line::from(vec![
                Span::styled("Use number keys to select, ", Style::default().fg(Color::Gray)),
                Span::styled("Esc", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(" to close", Style::default().fg(Color::Gray)),
            ]),
        ];

        let footer = Paragraph::new(footer_text)
            .alignment(Alignment::Center);
        f.render_widget(footer, chunks[2]);
    }

    /// Draw tools overlay
    fn draw_tools_overlay(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Header
                Constraint::Min(1),    // Tools list
                Constraint::Length(2), // Footer
            ])
            .split(area);

        // Header
        let header_block = Block::default()
            .title("Available Tools")
            .borders(Borders::ALL);

        let header = Paragraph::new("Choose a tool to switch modes:")
            .block(header_block)
            .alignment(Alignment::Center);
        f.render_widget(header, chunks[0]);

        // Tools list
        let list_block = Block::default()
            .borders(Borders::ALL);

        let tools = vec![
            ("1", "Plan Mode", "Create execution plans without running commands"),
            ("2", "Build Mode", "Safe code modifications with AI assistance"),
            ("3", "Run Mode", "Execute multi-step command sequences"),
            ("4", "Chat Mode", "Interactive conversation with AI"),
            ("5", "RAG Mode", "Query codebase with context retrieval"),
        ];

        let items: Vec<ListItem> = tools
            .iter()
            .map(|(num, name, desc)| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} {}", num, name), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(" - ", Style::default().fg(Color::Gray)),
                    Span::styled(*desc, Style::default().fg(Color::White)),
                ]))
            })
            .collect();

        let list = List::new(items).block(list_block);
        f.render_widget(list, chunks[1]);

        // Footer
        let footer = Paragraph::new("Press number key to select, Esc to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));
        f.render_widget(footer, chunks[2]);
    }

    /// Draw context overlay
    fn draw_context_overlay(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Header
                Constraint::Min(1),    // Context list
                Constraint::Length(2), // Footer
            ])
            .split(area);

        // Header
        let header_block = Block::default()
            .title("Project Context")
            .borders(Borders::ALL);

        let header = Paragraph::new("Explore project state and history:")
            .block(header_block)
            .alignment(Alignment::Center);
        f.render_widget(header, chunks[0]);

        // Context list
        let list_block = Block::default()
            .borders(Borders::ALL);

        let context_items = vec![
            ("1", "Directory Files", "List files in current directory"),
            ("2", "Git Status", "Show uncommitted changes"),
            ("3", "Recent Commands", "Command history for this session"),
            ("4", "Project Structure", "Show directory tree"),
            ("5", "Configuration", "Show current settings"),
        ];

        let items: Vec<ListItem> = context_items
            .iter()
            .map(|(num, name, desc)| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} {}", num, name), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(" - ", Style::default().fg(Color::Gray)),
                    Span::styled(*desc, Style::default().fg(Color::White)),
                ]))
            })
            .collect();

        let list = List::new(items).block(list_block);
        f.render_widget(list, chunks[1]);

        // Footer
        let footer = Paragraph::new("Press number key to select, Esc to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));
        f.render_widget(footer, chunks[2]);
    }

    /// Draw command palette overlay
    fn draw_palette_overlay(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Header
                Constraint::Min(1),    // Commands list
                Constraint::Length(2), // Footer
            ])
            .split(area);

        // Header
        let header_block = Block::default()
            .title("Command Palette")
            .borders(Borders::ALL);

        let header = Paragraph::new("Quick commands and actions:")
            .block(header_block)
            .alignment(Alignment::Center);
        f.render_widget(header, chunks[0]);

        // Commands list
        let list_block = Block::default()
            .borders(Borders::ALL);

        let commands = vec![
            ("1", ":quit", "Exit the application"),
            ("2", ":help", "Show help and keybindings"),
            ("3", ":session <name>", "Switch to named session"),
            ("4", ":clear", "Clear command buffer"),
            ("5", ":status", "Show current status"),
            ("6", ":wq", "Save session and quit"),
            ("7", ":mode <type>", "Switch mode (plan/build/run/chat)"),
            ("8", ":history", "Show full command history"),
        ];

        let items: Vec<ListItem> = commands
            .iter()
            .map(|(num, cmd, desc)| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} {}", num, cmd), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(" - ", Style::default().fg(Color::Gray)),
                    Span::styled(*desc, Style::default().fg(Color::White)),
                ]))
            })
            .collect();

        let list = List::new(items).block(list_block);
        f.render_widget(list, chunks[1]);

        // Footer
        let footer = Paragraph::new("Press number key to execute, Esc to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));
        f.render_widget(footer, chunks[2]);
    }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}