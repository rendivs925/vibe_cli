use colored::Colorize;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Represents a backup of a file before modification
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileBackup {
    path: PathBuf,
    content: Option<Vec<u8>>, // None if file didn't exist
    existed: bool,
}

/// Transaction state for file operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    Pending,
    InProgress,
    Committed,
    RolledBack,
    Failed,
}

/// A transaction manages a set of file operations atomically
pub struct Transaction {
    id: String,
    state: TransactionState,
    backups: HashMap<PathBuf, FileBackup>,
    operations_log: Vec<String>,
}

impl Transaction {
    /// Create a new transaction
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            state: TransactionState::Pending,
            backups: HashMap::new(),
            operations_log: Vec::new(),
        }
    }

    /// Get the transaction ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the current state
    pub fn state(&self) -> TransactionState {
        self.state
    }

    /// Begin the transaction
    pub fn begin(&mut self) -> Result<()> {
        if self.state != TransactionState::Pending {
            return Err(anyhow::anyhow!("Transaction already started"));
        }
        self.state = TransactionState::InProgress;
        println!(
            "{}",
            format!("Transaction {} started", self.id).bright_cyan()
        );
        Ok(())
    }

    /// Create a backup of a file before modifying it
    pub fn backup_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref().to_path_buf();

        // Don't backup the same file twice
        if self.backups.contains_key(&path) {
            return Ok(());
        }

        let backup = if path.exists() {
            let content = std::fs::read(&path)?;
            FileBackup {
                path: path.clone(),
                content: Some(content),
                existed: true,
            }
        } else {
            FileBackup {
                path: path.clone(),
                content: None,
                existed: false,
            }
        };

        self.backups.insert(path.clone(), backup);
        self.log_operation(format!("Backed up: {}", path.display()));
        Ok(())
    }

    /// Execute a file write operation with backup
    pub fn write_file<P: AsRef<Path>>(&mut self, path: P, content: &[u8]) -> Result<()> {
        if self.state != TransactionState::InProgress {
            return Err(anyhow::anyhow!("Transaction not in progress"));
        }

        let path = path.as_ref();

        // Create backup before modifying
        self.backup_file(path)?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write the file
        std::fs::write(path, content)?;
        self.log_operation(format!("Wrote: {}", path.display()));

        Ok(())
    }

    /// Execute a file delete operation with backup
    pub fn delete_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        if self.state != TransactionState::InProgress {
            return Err(anyhow::anyhow!("Transaction not in progress"));
        }

        let path = path.as_ref();

        if !path.exists() {
            return Err(anyhow::anyhow!("File does not exist: {}", path.display()));
        }

        // Create backup before deleting
        self.backup_file(path)?;

        // Delete the file
        std::fs::remove_file(path)?;
        self.log_operation(format!("Deleted: {}", path.display()));

        Ok(())
    }

    /// Log an operation
    fn log_operation(&mut self, message: String) {
        self.operations_log.push(message);
    }

    /// Commit the transaction (make changes permanent)
    pub fn commit(mut self) -> Result<()> {
        if self.state != TransactionState::InProgress {
            return Err(anyhow::anyhow!("Transaction not in progress"));
        }

        self.state = TransactionState::Committed;
        println!(
            "{}",
            format!(
                "Transaction {} committed successfully ({} operations)",
                self.id,
                self.operations_log.len()
            )
            .bright_green()
        );

        // Clear backups as they're no longer needed
        self.backups.clear();

        Ok(())
    }

    /// Rollback the transaction (restore all files to original state)
    pub fn rollback(&mut self) -> Result<()> {
        if self.state == TransactionState::Committed {
            return Err(anyhow::anyhow!("Cannot rollback committed transaction"));
        }

        if self.state == TransactionState::RolledBack {
            return Err(anyhow::anyhow!("Transaction already rolled back"));
        }

        println!(
            "{}",
            format!("Rolling back transaction {}...", self.id).bright_yellow()
        );

        let mut errors = Vec::new();

        // Restore all backed up files
        for (path, backup) in &self.backups {
            match self.restore_backup(backup) {
                Ok(_) => {
                    println!("{}", format!("  Restored: {}", path.display()).yellow());
                }
                Err(e) => {
                    errors.push(format!("Failed to restore {}: {}", path.display(), e));
                    eprintln!("{}", format!("  Failed: {}", path.display()).red());
                }
            }
        }

        self.state = TransactionState::RolledBack;

        if errors.is_empty() {
            println!(
                "{}",
                format!("Transaction {} rolled back successfully", self.id).bright_green()
            );
            Ok(())
        } else {
            self.state = TransactionState::Failed;
            Err(anyhow::anyhow!(
                "Rollback completed with {} errors: {}",
                errors.len(),
                errors.join(", ")
            ))
        }
    }

    /// Restore a file from backup
    fn restore_backup(&self, backup: &FileBackup) -> Result<()> {
        if backup.existed {
            // File existed before, restore original content
            if let Some(content) = &backup.content {
                std::fs::write(&backup.path, content)?;
            }
        } else {
            // File didn't exist before, delete it if it exists now
            if backup.path.exists() {
                std::fs::remove_file(&backup.path)?;
            }
        }
        Ok(())
    }

    /// Get the operations log
    pub fn operations(&self) -> &[String] {
        &self.operations_log
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // Auto-rollback if transaction wasn't committed and is still in progress
        if self.state == TransactionState::InProgress {
            eprintln!(
                "{}",
                format!(
                    "WARNING: Transaction {} dropped without commit, performing auto-rollback",
                    self.id
                )
                .bright_red()
            );
            let _ = self.rollback();
        }
    }
}

/// Scoped transaction guard that auto-rollbacks on drop
pub struct TransactionGuard {
    transaction: Option<Transaction>,
    auto_commit: bool,
}

impl TransactionGuard {
    /// Create a new transaction guard
    pub fn new() -> Result<Self> {
        let mut transaction = Transaction::new();
        transaction.begin()?;

        Ok(Self {
            transaction: Some(transaction),
            auto_commit: false,
        })
    }

    /// Enable auto-commit on successful completion
    pub fn auto_commit(mut self) -> Self {
        self.auto_commit = true;
        self
    }

    /// Get mutable reference to the transaction
    pub fn transaction(&mut self) -> &mut Transaction {
        self.transaction.as_mut().unwrap()
    }

    /// Manually commit the transaction
    pub fn commit(mut self) -> Result<()> {
        if let Some(transaction) = self.transaction.take() {
            transaction.commit()?;
        }
        Ok(())
    }
}

impl Drop for TransactionGuard {
    fn drop(&mut self) {
        if let Some(mut transaction) = self.transaction.take() {
            if self.auto_commit && transaction.state() == TransactionState::InProgress {
                let _ = transaction.commit();
            } else if transaction.state() == TransactionState::InProgress {
                let _ = transaction.rollback();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_transaction_commit() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_transaction_commit.txt");

        // Clean up if exists
        let _ = fs::remove_file(&test_file);

        let mut transaction = Transaction::new();
        transaction.begin()?;
        transaction.write_file(&test_file, b"test content")?;
        transaction.commit()?;

        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file)?, "test content");

        // Cleanup
        let _ = fs::remove_file(&test_file);

        Ok(())
    }

    #[test]
    fn test_transaction_rollback() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_transaction_rollback.txt");

        // Create initial file
        fs::write(&test_file, b"original")?;

        let mut transaction = Transaction::new();
        transaction.begin()?;
        transaction.write_file(&test_file, b"modified")?;
        transaction.rollback()?;

        // Should be restored to original
        assert_eq!(fs::read_to_string(&test_file)?, "original");

        // Cleanup
        let _ = fs::remove_file(&test_file);

        Ok(())
    }

    #[test]
    fn test_transaction_auto_rollback() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_transaction_auto_rollback.txt");

        // Create initial file
        fs::write(&test_file, b"original")?;

        {
            let mut transaction = Transaction::new();
            transaction.begin()?;
            transaction.write_file(&test_file, b"modified")?;
            // Drop without commit - should auto-rollback
        }

        // Should be restored to original
        assert_eq!(fs::read_to_string(&test_file)?, "original");

        // Cleanup
        let _ = fs::remove_file(&test_file);

        Ok(())
    }

    #[test]
    fn test_transaction_guard() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_transaction_guard.txt");

        // Clean up if exists
        let _ = fs::remove_file(&test_file);

        {
            let mut guard = TransactionGuard::new()?;
            guard.transaction().write_file(&test_file, b"guard test")?;
            guard.commit()?;
        }

        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file)?, "guard test");

        // Cleanup
        let _ = fs::remove_file(&test_file);

        Ok(())
    }
}
