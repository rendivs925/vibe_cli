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
