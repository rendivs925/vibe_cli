use crate::types::Result;
use dialoguer::console::Term;

/// Standardized confirmation prompt used across binaries.
/// Returns immediately on single keypress: y/Y, n/N, or Enter for default.
pub fn ask_confirmation(prompt: &str, default_yes: bool) -> Result<bool> {
    let term = Term::stdout();
    let default_hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    term.write_str(&format!("{prompt} {default_hint} "))?;
    term.flush()?;

    loop {
        let ch = term.read_char()?;
        match ch {
            'y' | 'Y' => {
                term.write_line("")?;
                return Ok(true);
            }
            'n' | 'N' => {
                term.write_line("")?;
                return Ok(false);
            }
            '\n' | '\r' => {
                term.write_line("")?;
                return Ok(default_yes);
            }
            _ => continue,
        }
    }
}
