use crate::clipboard;
use crate::config::Config;
use crate::safety::{assess_command, print_assessment, require_additional_confirmation};
use anyhow::Result;
use colored::*;
use dialoguer::Confirm;
use std::process::Command;

pub fn offer_cached(cached: &str) -> Result<bool> {
    println!(
        "{} {}",
        "Cached command found for this prompt:".green().bold(),
        cached.yellow()
    );
    let use_cached = Confirm::new()
        .with_prompt("Use this cached command?")
        .default(true)
        .interact()?;
    Ok(use_cached)
}

pub fn confirm_and_run(cmd: &str, config: &Config) -> Result<()> {
    println!("{} {}", "Suggested command:".green().bold(), cmd.yellow());

    if config.copy_to_clipboard {
        if let Err(err) = clipboard::copy_to_clipboard(cmd) {
            eprintln!("{} {}", "Clipboard copy failed:".red(), err);
        } else {
            println!("{}", "Copied to clipboard.".green());
        }
    }

    let assessment = assess_command(cmd, config.safe_mode);

    if assessment.blocked {
        print_assessment(&assessment);
        println!(
            "\n{}",
            "Command has been blocked in ultra-safe mode. It will not be executed.".red()
        );
        return Ok(());
    }

    print_assessment(&assessment);

    // If there are warnings, require an extra typed confirmation.
    if !assessment.warnings.is_empty() {
        let proceed = require_additional_confirmation(&assessment)?;
        if !proceed {
            return Ok(());
        }
    }

    let proceed = Confirm::new()
        .with_prompt("Run this command?")
        .default(false)
        .interact()?;

    if !proceed {
        println!("{}", "Cancelled by user.".red());
        return Ok(());
    }

    println!("{}", "Running command...\n".cyan());

    let status = Command::new("sh").arg("-c").arg(cmd).status()?;

    if status.success() {
        println!("{}", "Command completed successfully.".green());
    } else {
        println!(
            "{} (exit status: {:?})",
            "Command failed.".red(),
            status.code()
        );
    }

    Ok(())
}
