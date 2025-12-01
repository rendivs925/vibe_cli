use dialoguer::Confirm;
use crate::types::Result;

/// Standardized confirmation prompt used across binaries.
pub fn ask_confirmation(prompt: &str, default_yes: bool) -> Result<bool> {
    let choice = Confirm::new()
        .with_prompt(prompt)
        .default(default_yes)
        .show_default(true)
        .interact()?;
    Ok(choice)
}
