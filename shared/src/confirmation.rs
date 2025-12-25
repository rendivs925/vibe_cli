use crate::types::Result;
use colored::Colorize;
use crossterm::event::{read, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use dialoguer::console::Term;

/// Standardized confirmation prompt used across binaries.
/// Returns immediately on single keypress: y/Y, n/N, or Enter for default.
pub fn ask_confirmation(prompt: &str, default_yes: bool) -> Result<bool> {
    let term = Term::stdout();
    let default_hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    term.write_str(&format!("{prompt} {default_hint} "))?;
    term.flush()?;

    enable_raw_mode()?;
    let result = loop {
        match read()? {
            Event::Key(key) => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => break true,
                KeyCode::Char('n') | KeyCode::Char('N') => break false,
                KeyCode::Enter => break default_yes,
                _ => continue,
            },
            _ => continue,
        }
    };
    disable_raw_mode()?;

    // Echo selection with color for clarity.
    let selection = if result { "y".green() } else { "n".red() };
    term.write_line(&selection.to_string())?;

    Ok(result)
}

/// Enhanced confirmation with multiple options for advanced workflows
#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmationChoice {
    Yes,
    No,
    Edit,
    Revise,
    Suggest,
}

impl ConfirmationChoice {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Edit => "edit",
            Self::Revise => "revise",
            Self::Suggest => "suggest",
        }
    }
}

/// Advanced confirmation prompt with multiple choice options
pub fn ask_enhanced_confirmation(prompt: &str) -> Result<ConfirmationChoice> {
    let term = Term::stdout();
    term.write_str(&format!("{prompt} [y/n/edit/revise/suggest] "))?;
    term.flush()?;

    enable_raw_mode()?;
    let result = loop {
        match read()? {
            Event::Key(key) => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => break ConfirmationChoice::Yes,
                KeyCode::Char('n') | KeyCode::Char('N') => break ConfirmationChoice::No,
                KeyCode::Char('e') | KeyCode::Char('E') => break ConfirmationChoice::Edit,
                KeyCode::Char('r') | KeyCode::Char('R') => break ConfirmationChoice::Revise,
                KeyCode::Char('s') | KeyCode::Char('S') => break ConfirmationChoice::Suggest,
                KeyCode::Enter => break ConfirmationChoice::No, // Default to No
                _ => continue,
            },
            _ => continue,
        }
    };
    disable_raw_mode()?;

    // Echo selection with color for clarity
    let (selection, color) = match result {
        ConfirmationChoice::Yes => ("yes".green(), true),
        ConfirmationChoice::No => ("no".red(), true),
        ConfirmationChoice::Edit => ("edit".bright_blue(), true),
        ConfirmationChoice::Revise => ("revise".bright_yellow(), true),
        ConfirmationChoice::Suggest => ("suggest".bright_cyan(), true),
    };

    term.write_line(&selection.to_string())?;

    Ok(result)
}
