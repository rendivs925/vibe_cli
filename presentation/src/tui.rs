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
                // Dismiss overlay or clear status
                if self.show_overlay.is_some() {
                    self.show_overlay = None;
                    self.status_message = "Ready".to_string();
                }
            }
            _ => {}
        }
        Ok(false)
    }

    /// Navigate command history
    fn navigate_history(&mut self, previous: bool) {
        if self.command_history.is_empty() {
            return;
        }

        let history_len = self.command_history.len();

        if let Some(current_index) = self.history_index {
            if previous {
                if current_index > 0 {
                    self.history_index = Some(current_index - 1);
                }
            } else {
                if current_index < history_len - 1 {
                    self.history_index = Some(current_index + 1);
                } else {
                    // At end of history, clear to show empty buffer
                    self.history_index = None;
                    self.input_buffer.clear();
                    self.cursor_position = 0;
                    return;
                }
            }
        } else if previous {
            // Start from most recent command
            self.history_index = Some(history_len - 1);
        }

        if let Some(index) = self.history_index {
            if let Some(command) = self.command_history.get(index) {
                self.input_buffer = command.clone();
                self.cursor_position = command.len();
            }
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

        self.status_message = format!("Executing: {}", command);

        // Here we would integrate with the existing CLI logic
        // For now, just show a placeholder response
        self.status_message = format!("Executed: {} ✓", command);

        Ok(())
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
        let block = Block::default()
            .title("Sessions")
            .borders(Borders::ALL);

        let items: Vec<ListItem> = self.session_list
            .iter()
            .enumerate()
            .map(|(i, session)| {
                let style = if Some(session) == self.current_session.as_ref() {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}. {}", i + 1, session)).style(style)
            })
            .collect();

        let list = List::new(items).block(block);
        f.render_widget(list, area);
    }

    /// Draw tools overlay
    fn draw_tools_overlay(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("Tools")
            .borders(Borders::ALL);

        let tools = vec![
            "Plan Mode",
            "Build Mode",
            "Run Mode",
            "Chat Mode",
            "RAG Query",
            "Explain Code",
        ];

        let items: Vec<ListItem> = tools
            .iter()
            .enumerate()
            .map(|(i, tool)| ListItem::new(format!("{}. {}", i + 1, tool)))
            .collect();

        let list = List::new(items).block(block);
        f.render_widget(list, area);
    }

    /// Draw context overlay
    fn draw_context_overlay(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("Context")
            .borders(Borders::ALL);

        let context_items = vec![
            "Current Directory",
            "Open Files",
            "Git Status",
            "Recent Commands",
            "Project Structure",
        ];

        let items: Vec<ListItem> = context_items
            .iter()
            .enumerate()
            .map(|(i, item)| ListItem::new(format!("{}. {}", i + 1, item)))
            .collect();

        let list = List::new(items).block(block);
        f.render_widget(list, area);
    }

    /// Draw command palette overlay
    fn draw_palette_overlay(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("Command Palette")
            .borders(Borders::ALL);

        let commands = vec![
            ":quit - Exit application",
            ":help - Show help",
            ":session <name> - Switch session",
            ":clear - Clear output",
            ":history - Show command history",
        ];

        let items: Vec<ListItem> = commands
            .iter()
            .enumerate()
            .map(|(i, cmd)| ListItem::new(format!("{}. {}", i + 1, cmd)))
            .collect();

        let list = List::new(items).block(block);
        f.render_widget(list, area);
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