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

/// Confirmation prompt with optional "generate new" choice for cached commands.
/// Returns Some(true) for yes, Some(false) for no, None for generate new.
pub fn ask_command_confirmation(prompt: &str, allow_generate: bool) -> Result<Option<bool>> {
    let term = Term::stdout();
    let hint = if allow_generate { "[y/N/g]" } else { "[y/N]" };
    term.write_str(&format!("{prompt} {hint} "))?;
    term.flush()?;

    enable_raw_mode()?;
    let result = loop {
        match read()? {
            Event::Key(key) => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => break Some(true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter => break Some(false),
                KeyCode::Char('g') | KeyCode::Char('G') if allow_generate => break None,
                _ => continue,
            },
            _ => continue,
        }
    };
    disable_raw_mode()?;

    // Echo selection with color for clarity.
    let selection = match result {
        Some(true) => "y".green().to_string(),
        Some(false) => "n".red().to_string(),
        None => "g".yellow().to_string(),
    };
    term.write_line(&selection)?;

    Ok(result)
}
