//! Session management functionality for CLI operations

use colored::Colorize;
use infrastructure::session_store::{SessionStore, SessionMetadata};
use shared::confirmation::ask_confirmation;
use shared::types::Result;
use crate::utils::find_project_root;

/// Display all sessions for the current project
pub fn display_sessions(
    store: &SessionStore,
    current_session: Option<&String>,
) -> Result<()> {
    let project_root = find_project_root().unwrap_or_else(|| "unknown".to_string());
    let project_hash = store.project_hash();

    println!("{}", "Session Management".bright_cyan().bold());
    println!("Project: {} (hash: {})", project_root, &project_hash[..8]);
    println!();

    match store.list_sessions() {
        Ok(sessions) if sessions.is_empty() => {
            println!("{}", "No sessions found.".dimmed());
            println!(
                "Create your first session with: ai --session \"my-session\" --build \"...\""
            );
        }
        Ok(sessions) => {
            println!("Sessions:");
            for session in sessions {
                let active_marker = if Some(&session.name) == current_session {
                    "[active] "
                } else {
                    "          "
                };

                let last_used = session.last_used.format("%Y-%m-%d %H:%M");
                let goal = if session.goal_summary.is_empty() {
                    "No goal set".dimmed()
                } else {
                    session.goal_summary.dimmed()
                };

                println!(
                    "  {} {:<15} Last used: {}  Changes: {}  Goal: {}",
                    active_marker,
                    session.name.bright_green(),
                    last_used,
                    session.change_count,
                    goal
                );
            }
        }
        Err(e) => {
            eprintln!("{} {}", "Error listing sessions:".red(), e);
        }
    }

    Ok(())
}

/// Delete a session with user confirmation
pub fn delete_session_with_confirmation(
    store: &SessionStore,
    session_name: &str,
) -> Result<()> {
    let prompt = format!(
        "Permanently delete session '{}' and all its data?",
        session_name
    );

    match ask_confirmation(&prompt, false) {
        Ok(true) => {
            match store.delete_session(session_name) {
                Ok(_) => {
                    println!(
                        "{} Session '{}' deleted successfully.",
                        "V".green(),
                        session_name
                    );
                    // Export backup before deletion
                    if let Ok(backup_path) = store.export_session(session_name) {
                        println!(
                            "{} Session backed up to: {}",
                            "Backup".blue(),
                            backup_path.display()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to delete session: {}", "X".red(), e);
                }
            }
        }
        Ok(false) => {
            println!("{}", "Session deletion cancelled.".yellow());
        }
        Err(e) => {
            eprintln!("{} Confirmation error: {}", "X".red(), e);
        }
    }

    Ok(())
}

/// Get the target session to continue (current, most recent, or default)
pub fn get_target_session_to_continue(
    store: &SessionStore,
    current_session: Option<&String>,
) -> String {
    if let Some(current) = current_session {
        current.clone()
    } else {
        // Find most recently used session
        match store.list_sessions() {
            Ok(sessions) if !sessions.is_empty() => sessions
                .into_iter()
                .max_by_key(|s| s.last_used)
                .map(|s| s.name)
                .unwrap(),
            _ => "main".to_string(),
        }
    }
}

/// Display session continuation information
pub fn display_session_info(session_name: &str, metadata: &SessionMetadata, message_count: usize) {
    println!(
        "{} Continuing session '{}'",
        ">".green(),
        session_name.bright_green()
    );
    println!("  Goal: {}", metadata.goal_summary.dimmed());
    println!("  Changes: {}", metadata.change_count);
    println!(
        "  Last used: {}",
        metadata.last_used.format("%Y-%m-%d %H:%M")
    );

    if message_count > 0 {
        println!("  Conversation: {} messages", message_count);
    }
}

/// Display session creation confirmation
pub fn display_session_created(session_name: &str) {
    println!(
        "{} Created and activated session '{}'",
        "V".green(),
        session_name.bright_green()
    );
}
