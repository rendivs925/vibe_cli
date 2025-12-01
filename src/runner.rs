use crate::clipboard;
use crate::config::Config;
use crate::safety::{assess_command, print_assessment, require_additional_confirmation};
use anyhow::{anyhow, Result};
use colored::*;
use dialoguer::Confirm;
use std::process::Command;

/// Validate basic shell command syntax
fn validate_command_syntax(cmd: &str) -> Result<()> {
    let cmd = cmd.trim();

    // Skip validation for very simple commands
    if cmd.chars().filter(|&c| c == ' ' || c == '\t').count() < 2 && !cmd.contains('"') && !cmd.contains('\'') {
        return Ok(());
    }

    let mut single_quotes = 0;
    let mut double_quotes = 0;
    let mut parens = 0;
    let mut brackets = 0;
    let mut braces = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escape_next = false;

    let chars: Vec<char> = cmd.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' => {
                escape_next = true;
            }
            '\'' => {
                if !in_double_quote {
                    in_single_quote = !in_single_quote;
                    single_quotes += 1;
                }
            }
            '"' => {
                if !in_single_quote {
                    in_double_quote = !in_double_quote;
                    double_quotes += 1;
                }
            }
            '(' => {
                if !in_single_quote && !in_double_quote {
                    parens += 1;
                }
            }
            ')' => {
                if !in_single_quote && !in_double_quote {
                    parens -= 1;
                    if parens < 0 {
                        return Err(anyhow!("Unmatched closing parenthesis"));
                    }
                }
            }
            '[' => {
                if !in_single_quote && !in_double_quote {
                    brackets += 1;
                }
            }
            ']' => {
                if !in_single_quote && !in_double_quote {
                    brackets -= 1;
                    if brackets < 0 {
                        return Err(anyhow!("Unmatched closing bracket"));
                    }
                }
            }
            '{' => {
                if !in_single_quote && !in_double_quote {
                    braces += 1;
                }
            }
            '}' => {
                if !in_single_quote && !in_double_quote {
                    braces -= 1;
                    if braces < 0 {
                        return Err(anyhow!("Unmatched closing brace"));
                    }
                }
            }
            _ => {}
        }
    }

    // Check for unclosed quotes
    if in_single_quote {
        return Err(anyhow!("Unclosed single quote"));
    }
    if in_double_quote {
        return Err(anyhow!("Unclosed double quote"));
    }

    // Check for unmatched parentheses/brackets/braces
    if parens != 0 {
        return Err(anyhow!("Unmatched parentheses"));
    }
    if brackets != 0 {
        return Err(anyhow!("Unmatched brackets"));
    }
    if braces != 0 {
        return Err(anyhow!("Unmatched braces"));
    }

    // Check for incomplete expressions (common patterns)
    if cmd.ends_with("&&") || cmd.ends_with("||") || cmd.ends_with("|") || cmd.ends_with(";") {
        return Err(anyhow!("Command ends with incomplete expression"));
    }

    // Check for incomplete awk expressions
    if cmd.contains("awk") && (cmd.ends_with("$") || cmd.contains("$") && !cmd.contains("{print") && !cmd.contains("{print ")) {
        return Err(anyhow!("Potentially incomplete awk expression"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_command_syntax;

    #[test]
    fn test_valid_commands() {
        assert!(validate_command_syntax("ls -la").is_ok());
        assert!(validate_command_syntax("echo 'hello world'").is_ok());
        assert!(validate_command_syntax("du -h --max-depth=1 | sort -hr").is_ok());
        assert!(validate_command_syntax("find . -name '*.rs' -exec grep 'fn' {} \\;").is_ok());
    }

    #[test]
    fn test_invalid_commands() {
        assert!(validate_command_syntax("echo 'hello").is_err()); // unclosed quote
        assert!(validate_command_syntax("du -h | awk '$1 >").is_err()); // incomplete awk
        assert!(validate_command_syntax("ls &&").is_err()); // incomplete expression
        assert!(validate_command_syntax("echo (hello").is_err()); // unmatched paren
        assert!(validate_command_syntax("ls [ -f file").is_err()); // unmatched bracket
    }
}

pub fn confirm_and_run(cmd: &str, config: &Config) -> Result<()> {
    println!("{} {}", "Suggested command:".green().bold(), cmd.yellow());

    // Validate command syntax before proceeding
    if let Err(validation_error) = validate_command_syntax(cmd) {
        println!(
            "{} {}",
            "Command validation failed:".red().bold(),
            validation_error.to_string().red()
        );
        println!("{}", "This command appears to have syntax errors and will not be executed.".red());
        return Ok(());
    }

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
        println!("{}", "Command execution cancelled.".yellow());
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

pub fn confirm_and_run_multi_step(cmd: &str, config: &Config) -> Result<()> {
    println!("{} {}", "Suggested command:".green().bold(), cmd.yellow());

    let accept = Confirm::new()
        .with_prompt("Accept this command?")
        .default(true)
        .interact()?;

    if !accept {
        println!("{}", "Command rejected. Skipping this step.".yellow());
        return Ok(());
    }

    // Validate command syntax before proceeding
    if let Err(validation_error) = validate_command_syntax(cmd) {
        println!(
            "{} {}",
            "Command validation failed:".red().bold(),
            validation_error.to_string().red()
        );
        println!("{}", "This command appears to have syntax errors and will not be executed.".red());
        return Ok(());
    }

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
