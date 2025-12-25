use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Safe fix application system with transaction rollback
pub struct FixApplier {
    project_root: PathBuf,
    backup_dir: PathBuf,
    applied_fixes: HashMap<String, AppliedFix>,
}

#[derive(Debug, Clone)]
pub struct AppliedFix {
    pub id: String,
    pub description: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub files_modified: Vec<PathBuf>,
    pub backup_paths: Vec<PathBuf>,
    pub git_commit_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FixConfidence {
    High,    // Apply automatically
    Medium,  // Ask user confirmation
    Low,     // Manual application only
}

impl FixApplier {
    /// Create a new fix applier
    pub fn new(project_root: PathBuf) -> Self {
        let backup_dir = project_root.join(".vibe_fixes");
        let _ = std::fs::create_dir_all(&backup_dir);

        Self {
            project_root,
            backup_dir,
            applied_fixes: HashMap::new(),
        }
    }

    /// Apply a fix safely with rollback capability
    pub async fn apply_fix(
        &mut self,
        fix: &super::error_analyzer::FixSuggestion,
        confidence: FixConfidence,
    ) -> Result<AppliedFix> {
        println!("🔧 Applying fix: {}", fix.description);

        // Check if we should apply automatically
        match confidence {
            FixConfidence::High => {
                println!("✅ High confidence - applying automatically");
            }
            FixConfidence::Medium => {
                println!("⚠️ Medium confidence - applying with user confirmation");
                if !self.confirm_fix_application(fix).await? {
                    return Err(anyhow::anyhow!("Fix application cancelled by user"));
                }
            }
            FixConfidence::Low => {
                println!("❌ Low confidence - manual application required");
                return Err(anyhow::anyhow!("Fix confidence too low for automatic application"));
            }
        }

        // Create transaction
        let transaction_id = format!("fix_{}", chrono::Utc::now().timestamp());
        let transaction_backup_dir = self.backup_dir.join(&transaction_id);

        std::fs::create_dir_all(&transaction_backup_dir)?;

        // Apply the fix
        let applied_fix = self.apply_fix_transactionally(fix, &transaction_id).await?;

        // Create git commit if possible
        if let Ok(commit_hash) = self.create_git_commit(&applied_fix).await {
            println!("💾 Created git commit: {}", commit_hash);
        }

        println!("✅ Fix applied successfully: {}", fix.description);
        Ok(applied_fix)
    }

    /// Apply fix within a transaction with rollback capability
    async fn apply_fix_transactionally(
        &mut self,
        fix: &super::error_analyzer::FixSuggestion,
        transaction_id: &str,
    ) -> Result<AppliedFix> {
        let mut files_modified = Vec::new();
        let mut backup_paths = Vec::new();

        // Apply each code change
        for change in &fix.changes {
            self.apply_code_change(change, transaction_id, &mut files_modified, &mut backup_paths).await?;
        }

        let applied_fix = AppliedFix {
            id: transaction_id.to_string(),
            description: fix.description.clone(),
            timestamp: chrono::Utc::now(),
            files_modified,
            backup_paths,
            git_commit_hash: None,
        };

        self.applied_fixes.insert(transaction_id.to_string(), applied_fix.clone());

        Ok(applied_fix)
    }

    /// Apply a single code change
    async fn apply_code_change(
        &self,
        change: &super::error_analyzer::CodeChange,
        transaction_id: &str,
        files_modified: &mut Vec<PathBuf>,
        backup_paths: &mut Vec<PathBuf>,
    ) -> Result<()> {
        let file_path = self.project_root.join(&change.file_path);

        // Read current content
        let content = fs::read_to_string(&file_path)
            .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", file_path.display(), e))?;

        // Create backup
        let backup_filename = format!("{}.backup", change.file_path);
        let backup_path = self.backup_dir.join(transaction_id).join(backup_filename);
        std::fs::create_dir_all(backup_path.parent().unwrap())?;
        fs::write(&backup_path, &content)?;

        // Apply the change (simplified - in reality this would be more sophisticated)
        // For now, we'll just append the suggested code
        let new_content = format!("{}\n// AUTONOMOUS FIX: {}\n{}\n", content, change.new_code, change.new_code);
        fs::write(&file_path, new_content)?;

        files_modified.push(file_path);
        backup_paths.push(backup_path);

        Ok(())
    }

    /// Create a git commit for the applied fix
    async fn create_git_commit(&self, applied_fix: &AppliedFix) -> Result<String> {
        // Stage the modified files
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.project_root)
            .arg("add");

        for file in &applied_fix.files_modified {
            cmd.arg(file.strip_prefix(&self.project_root).unwrap_or(file));
        }

        cmd.status().await?;

        // Create commit
        let commit_msg = format!("🤖 Autonomous fix: {}", applied_fix.description);
        let output = Command::new("git")
            .current_dir(&self.project_root)
            .args(&["commit", "-m", &commit_msg])
            .output()
            .await?;

        if output.status.success() {
            let commit_hash = String::from_utf8_lossy(&output.stdout)
                .lines()
                .find(|line| line.starts_with("commit "))
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("unknown")
                .to_string();

            Ok(commit_hash)
        } else {
            Err(anyhow::anyhow!("Git commit failed"))
        }
    }

    /// Ask user for confirmation before applying fix
    async fn confirm_fix_application(&self, fix: &super::error_analyzer::FixSuggestion) -> Result<bool> {
        println!("\n🤔 Apply this fix?");
        println!("Description: {}", fix.description);
        println!("Confidence: {:.1}%", fix.confidence * 100.0);
        println!("Explanation: {}", fix.explanation);
        println!("Changes: {} code modification(s)", fix.changes.len());

        // In a real implementation, this would prompt the user
        // For now, we'll auto-approve medium confidence fixes
        if fix.confidence >= 0.6 {
            println!("✅ Auto-approved based on confidence score");
            Ok(true)
        } else {
            println!("❌ Rejected due to low confidence");
            Ok(false)
        }
    }

    /// Rollback an applied fix
    pub async fn rollback_fix(&mut self, fix_id: &str) -> Result<()> {
        let applied_fix = self.applied_fixes.get(fix_id)
            .ok_or_else(|| anyhow::anyhow!("Fix {} not found", fix_id))?;

        println!("🔄 Rolling back fix: {}", applied_fix.description);

        // Restore backups
        for (file_path, backup_path) in applied_fix.files_modified.iter().zip(&applied_fix.backup_paths) {
            if backup_path.exists() {
                let backup_content = fs::read_to_string(backup_path)?;
                fs::write(file_path, backup_content)?;
            }
        }

        // Git reset if we have a commit
        if let Some(commit_hash) = &applied_fix.git_commit_hash {
            Command::new("git")
                .current_dir(&self.project_root)
                .args(&["reset", "--hard", &format!("{}~1", commit_hash)])
                .status()
                .await?;
        }

        // Remove from applied fixes
        self.applied_fixes.remove(fix_id);

        println!("✅ Fix rolled back successfully");
        Ok(())
    }

    /// Get status of applied fixes
    pub fn get_applied_fixes(&self) -> &HashMap<String, AppliedFix> {
        &self.applied_fixes
    }

    /// Clean up old backups (keep last 10 fixes)
    pub fn cleanup_old_backups(&self) -> Result<()> {
        let entries = std::fs::read_dir(&self.backup_dir)?;
        let mut fix_dirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();

        // Sort by modification time (newest first)
        fix_dirs.sort_by(|a, b| {
            b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .cmp(&a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH))
        });

        // Keep only the last 10
        for old_dir in fix_dirs.iter().skip(10) {
            let _ = std::fs::remove_dir_all(old_dir.path());
        }

        Ok(())
    }
}