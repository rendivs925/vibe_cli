use crate::types::Result;
use colored::Colorize;
use crossterm::event::{read, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use dialoguer::console::Term;
use std::io;

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

/// Selection prompt for choosing from a list of numbered options.
/// Returns the selected index (0-based) or None for quit.
pub fn ask_selection(
    prompt: &str,
    options: &[String],
    allow_generate_new: bool,
) -> Result<Option<usize>> {
    let term = Term::stdout();

    // Display options
    for (i, option) in options.iter().enumerate() {
        term.write_line(&format!("  [{}] {}", i + 1, option))?;
    }

    // Build hint based on available actions
    let mut hint_parts = vec!["Choose [1-{}]".to_string()];
    if allow_generate_new {
        hint_parts.push("(g)enerate new".to_string());
    }
    hint_parts.push("(q)uit".to_string());
    let hint = hint_parts.join(" ");

    term.write_str(&format!("{}: ", hint))?;
    term.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "q" {
        return Ok(None);
    } else if input == "g" && allow_generate_new {
        return Err(anyhow::anyhow!("generate_new"));
    } else if let Ok(choice) = input.parse::<usize>() {
        if choice >= 1 && choice <= options.len() {
            term.write_line(&format!("Selected: {}", options[choice - 1]))?;
            return Ok(Some(choice - 1));
        }
    }

    term.write_line("Invalid choice. Please try again.")?;
    // Retry by calling again (recursive call with same parameters)
    ask_selection(prompt, options, allow_generate_new)
}

/// Simple text input prompt for collecting user feedback.
pub fn ask_feedback(prompt: &str) -> Result<String> {
    let term = Term::stdout();
    term.write_str(prompt)?;
    term.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
