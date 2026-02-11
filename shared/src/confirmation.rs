use crate::types::Result;
use colored::Colorize;
use crossterm::event::{read, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use dialoguer::console::Term;
use std::io;

fn write_prompt(term: &Term, prompt: &str, hint: Option<&str>) -> Result<()> {
    let prompt_text = prompt.cyan().bold();
    if let Some(hint) = hint {
        let hint_text = hint.dimmed();
        term.write_str(&format!("{prompt_text} {hint_text} "))?;
    } else {
        term.write_str(&format!("{prompt_text} "))?;
    }
    term.flush()?;
    Ok(())
}

/// Standardized confirmation prompt used across binaries.
/// Returns immediately on single keypress: y/Y, n/N, or Enter for default.
pub fn ask_confirmation(prompt: &str, default_yes: bool) -> Result<bool> {
    let term = Term::stdout();
    let default_hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    write_prompt(&term, prompt, Some(default_hint))?;

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
    write_prompt(&term, prompt, Some(hint))?;

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
pub fn ask_selection(options: &[String], allow_generate_new: bool) -> Result<Option<usize>> {
    let term = Term::stdout();

    // Build prompt based on available actions
    let hint = if allow_generate_new {
        format!("[1-{}] (g)enerate new (q)uit:", options.len())
    } else {
        format!("[1-{}] (q)uit:", options.len())
    };
    write_prompt(&term, "Choose", Some(&hint))?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "q" {
        return Ok(None);
    } else if input == "g" && allow_generate_new {
        return Err(anyhow::anyhow!("generate_new"));
    } else if let Ok(choice) = input.parse::<usize>() {
        if choice >= 1 && choice <= options.len() {
            let label = "Selected:".dimmed();
            let choice_text = options[choice - 1].green();
            term.write_line(&format!("{label} {choice_text}"))?;
            return Ok(Some(choice - 1));
        }
    }

    term.write_line(&"Invalid choice. Please try again.".red().to_string())?;
    // Retry by calling again (recursive call with same parameters)
    ask_selection(options, allow_generate_new)
}

/// Simple text input prompt for collecting user feedback.
pub fn ask_feedback(prompt: &str) -> Result<String> {
    let term = Term::stdout();
    write_prompt(&term, prompt, None)?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
