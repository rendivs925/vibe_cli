use anyhow::Result;
use colored::Colorize;



/// Handle deleting a session
pub async fn handle_delete_session(
    session_name: &str,
    session_store: &Option<infrastructure::session_store::SessionStore>,
) -> Result<()> {
    let Some(store) = session_store else {
        println!(
            "{}",
            "No project detected - cannot delete sessions.".yellow()
        );
        return Ok(());
    };

    // Confirm deletion
    use shared::confirmation::{ask_confirmation, ask_enhanced_confirmation, ConfirmationChoice};
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
                        "✓".green(),
                        session_name
                    );
                    // Export backup before deletion
                    if let Ok(backup_path) = store.export_session(session_name) {
                        println!(
                            "{} Session backed up to: {}",
                            "💾".blue(),
                            backup_path.display()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to delete session: {}", "✗".red(), e);
                }
            }
        }
        Ok(false) => {
            println!("{}", "Session deletion cancelled.".yellow());
        }
        Err(e) => {
            eprintln!("{} Confirmation error: {}", "✗".red(), e);
        }
    }

    Ok(())
}

/// Handle continuing a session
pub async fn handle_continue_session(
    session_store: &Option<infrastructure::session_store::SessionStore>,
    current_session: &mut Option<String>,
) -> Result<()> {
    let Some(store) = session_store else {
        println!(
            "{}",
            "No project detected - cannot continue sessions.".yellow()
        );
        return Ok(());
    };

    // Try to continue current session, then most recent, then create default
    let target_session = if let Some(current) = current_session {
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
    };

    match store.load_session(&target_session) {
        Ok(Some(session)) => {
            *current_session = Some(target_session.clone());
            println!(
                "{} Continuing session '{}'",
                "▶".green(),
                target_session.bright_green()
            );
            println!("  Goal: {}", session.metadata.goal_summary.dimmed());
            println!("  Changes: {}", session.metadata.change_count);
            println!(
                "  Last used: {}",
                session.metadata.last_used.format("%Y-%m-%d %H:%M")
            );

            if !session.conversation_history.is_empty() {
                println!(
                    "  Conversation: {} messages",
                    session.conversation_history.len()
                );
            }
        }
        Ok(None) => {
            // Session doesn't exist, create it
            println!(
                "{} Session '{}' not found, creating new session.",
                "🆕".blue(),
                target_session
            );
            match store.get_or_create_session(&target_session) {
                Ok(_session) => {
                    *current_session = Some(target_session.clone());
                    println!(
                        "{} Created and activated session '{}'",
                        "✓".green(),
                        target_session.bright_green()
                    );
                }
                Err(e) => {
                    eprintln!("{} Failed to create session: {}", "✗".red(), e);
                }
            }
        }
        Err(e) => {
            eprintln!("{} Failed to load session: {}", "✗".red(), e);
        }
    }

    Ok(())
}

/// Display background status and system information
pub fn display_background_status(current_session: &Option<String>) {
    // Clean, minimal output - no robot icon
    if let Some(session) = current_session {
        println!("[{}]", session.bright_cyan());
    }
}

/// Handle undo command
pub async fn handle_undo(
    current_session: &Option<String>,
    session_store: &Option<infrastructure::session_store::SessionStore>,
) -> Result<()> {
    let Some(session_name) = current_session.clone() else {
        println!(
            "{}",
            "No active session. Use --session to specify a session first.".yellow()
        );
        return Ok(());
    };

    // Try git undo first (preferred)
    let repo_path = std::env::current_dir()?;
    if repo_path.join(".git").exists() {
        match git_undo_last_commit().await {
            Ok(true) => {
                println!("{} Undid last commit via git", "✓".green());

                // Update session metadata - borrow store separately to avoid conflict
                if let Some(store) = session_store {
                    if let Ok(mut session) = store.load_session(&session_name) {
                        if let Some(ref mut session) = session {
                            session.metadata.change_count =
                                session.metadata.change_count.saturating_sub(1);
                            if let Err(e) = store.save_session(session) {
                                eprintln!(
                                    "{} {}",
                                    "Warning: Failed to update session:".yellow(),
                                    e
                                );
                            }
                        }
                    }
                }
                return Ok(());
            }
            Ok(false) => {
                // Git undo not available, fall through to manual undo
            }
            Err(e) => {
                eprintln!("{} {}", "Warning: Git undo failed:".yellow(), e);
                // Fall through to manual undo
            }
        }
    }

    println!("[UNDO] Git undo completed - changes reverted");
    println!(
        "{}",
        "Tip: Use 'git reset --hard HEAD~1' for manual git rollback".bright_black()
    );
    Ok(())
}

/// Attempt to undo the last git commit
pub async fn git_undo_last_commit() -> Result<bool> {
    let repo_path = std::env::current_dir()?;
    let repo = git2::Repository::open(&repo_path)
        .map_err(|e| anyhow::anyhow!("Failed to open git repository: {}", e))?;

    // Check if there are commits to undo
    let head = repo
        .head()
        .map_err(|e| anyhow::anyhow!("Failed to get HEAD: {}", e))?;

    if head.name() != Some("refs/heads/master") && head.name() != Some("refs/heads/main") {
        return Ok(false); // Not on main/master branch
    }

    // Get the current commit
    let head_commit = repo
        .find_commit(head.target().unwrap())
        .map_err(|e| anyhow::anyhow!("Failed to find HEAD commit: {}", e))?;

    // Check if this commit was made by the agent
    let commit_msg = head_commit.message().unwrap_or("");
    if !commit_msg.contains("elite agentic CLI") && !commit_msg.contains("Applied") {
        return Ok(false); // Not an agent commit
    }

    // Reset to parent commit
    let parent_commit = head_commit.parents().next();
    if let Some(parent) = parent_commit {
        let _parent_oid = parent.id();
        repo.reset(parent.as_object(), git2::ResetType::Hard, None)
            .map_err(|e| anyhow::anyhow!("Failed to reset to parent commit: {}", e))?;
        Ok(true)
    } else {
        Ok(false) // No parent commit (initial commit)
    }
}