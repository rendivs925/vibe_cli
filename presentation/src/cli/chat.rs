//! Chat interface helpers and utilities

use colored::Colorize;

/// Display agent execution options menu
pub fn display_execution_options() {
    println!();
    println!("EXECUTION OPTIONS:");
    println!("1. Execute complete plan (recommended)");
    println!("   - All steps run automatically");
    println!("   - Progress tracking enabled");
    println!("   - Automatic error recovery");
    println!();
    println!("2. Step-by-step execution");
    println!("   - Confirm each step individually");
    println!("   - Full control over execution");
    println!("   - Manual intervention possible");
    println!();
    println!("3. Dry run mode");
    println!("   - Show what would happen");
    println!("   - Validate commands without execution");
    println!("   - Test system compatibility");
    println!();
    println!("Choose execution mode (1-3) or 'cancel':");
}

/// Parse execution mode choice from user input
pub enum ExecutionMode {
    Complete,
    StepByStep,
    DryRun,
    Cancel,
}

pub fn parse_execution_choice(choice: &str) -> ExecutionMode {
    match choice {
        "1" => ExecutionMode::Complete,
        "2" => ExecutionMode::StepByStep,
        "3" => ExecutionMode::DryRun,
        _ => ExecutionMode::Cancel,
    }
}

/// Display command with formatting
pub fn display_command(command: &str) {
    println!("{}", format!("Command: {}", command).green());
}

/// Display command execution start
pub fn display_exec_start(command: &str) {
    println!("[EXEC] {}", command);
    println!("[RUN] Executing command...");
}

/// Display command completion
pub fn display_exec_done() {
    println!("[DONE] Command completed");
}

/// Display command failure
pub fn display_exec_error(error: &str) {
    println!("[DONE] Command failed: {}", error);
}

/// Display cancellation message
pub fn display_cancel_message() {
    println!("{}", "Command execution cancelled.".yellow());
}

/// Display alias expansion
pub fn display_alias_expansion(original: &str, expanded: &str) {
    println!("Using alias '{}' -> '{}'", original, expanded);
}

/// Display shortcut expansion
pub fn display_shortcut_expansion(original: &str, expanded: &str) {
    println!("Expanded '{}' to: {}", original, expanded);
}

/// Display blocked command warning
pub fn display_blocked_command() {
    println!("{}", "Command blocked by sandbox".red());
}

/// Display sandbox execution failure
pub fn display_sandbox_error(error: &str) {
    eprintln!("[ERROR] Sandbox execution failed: {}", error);
}

/// Display direct execution failure
pub fn display_direct_exec_error(error: &str) {
    eprintln!("[ERROR] Direct execution failed: {}", error);
}
