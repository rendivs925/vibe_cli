use shared::types::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use colored::Colorize;

/// Represents a file operation in the build process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOperation {
    Create { path: PathBuf, content: String },
    Read { path: PathBuf },
    Update { path: PathBuf, old_content: String, new_content: String },
    Delete { path: PathBuf },
}

/// Risk level for file operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,      // Creating new files, reading files
    Medium,   // Updating existing files
    High,     // Deleting files, modifying critical files
    Critical, // System files, configuration files
}

/// Build plan containing all planned operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildPlan {
    pub goal: String,
    pub operations: Vec<FileOperation>,
    pub description: String,
    pub estimated_risk: RiskLevel,
}

/// Result of executing a build plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub success: bool,
    pub operations_completed: usize,
    pub operations_failed: usize,
    pub error_messages: Vec<String>,
    pub rollback_performed: bool,
}

/// Service for managing build mode operations
pub struct BuildService {
    /// Workspace root directory
    workspace_root: PathBuf,
    /// Enable dry-run mode (preview only)
    dry_run: bool,
}

impl BuildService {
    /// Create a new BuildService
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            dry_run: false,
        }
    }

    /// Enable or disable dry-run mode
    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.dry_run = dry_run;
    }

    /// Assess risk level of a file operation
    pub fn assess_risk(&self, operation: &FileOperation) -> RiskLevel {
        match operation {
            FileOperation::Read { .. } => RiskLevel::Low,
            FileOperation::Create { path, .. } => {
                // Check if creating in sensitive directories
                if self.is_critical_path(path) {
                    RiskLevel::High
                } else {
                    RiskLevel::Low
                }
            }
            FileOperation::Update { path, .. } => {
                if self.is_critical_path(path) {
                    RiskLevel::Critical
                } else {
                    RiskLevel::Medium
                }
            }
            FileOperation::Delete { path } => {
                if self.is_critical_path(path) {
                    RiskLevel::Critical
                } else {
                    RiskLevel::High
                }
            }
        }
    }

    /// Check if a path is critical (system files, config files, etc.)
    fn is_critical_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Critical patterns
        let critical_patterns = [
            "/etc/",
            "/sys/",
            "/proc/",
            "/dev/",
            "cargo.toml",
            "package.json",
            ".git/",
            ".gitignore",
            "dockerfile",
            "makefile",
        ];

        critical_patterns.iter().any(|pattern| path_str.contains(pattern))
    }

    /// Display a file operation with color coding
    pub fn display_operation(&self, operation: &FileOperation, risk: RiskLevel) {
        let risk_color = match risk {
            RiskLevel::Low => "green",
            RiskLevel::Medium => "yellow",
            RiskLevel::High => "bright red",
            RiskLevel::Critical => "red",
        };

        let risk_label = format!("[{:?}]", risk).color(risk_color);

        match operation {
            FileOperation::Create { path, .. } => {
                println!("  {} CREATE: {}", risk_label, path.display().to_string().bright_blue());
            }
            FileOperation::Read { path } => {
                println!("  {} READ: {}", risk_label, path.display().to_string().bright_cyan());
            }
            FileOperation::Update { path, .. } => {
                println!("  {} UPDATE: {}", risk_label, path.display().to_string().bright_yellow());
            }
            FileOperation::Delete { path } => {
                println!("  {} DELETE: {}", risk_label, path.display().to_string().bright_red());
            }
        }
    }

    /// Preview a build plan with color-coded operations
    pub fn preview_plan(&self, plan: &BuildPlan) -> Result<()> {
        println!("\n{}", "=== Build Plan Preview ===".bright_cyan().bold());
        println!("{}: {}", "Goal".bright_green(), plan.goal);
        println!("{}: {}", "Description".bright_green(), plan.description);
        println!("{}: {:?}", "Estimated Risk".bright_green(), plan.estimated_risk);
        println!("\n{}", "Planned Operations:".bright_yellow());

        for operation in &plan.operations {
            let risk = self.assess_risk(operation);
            self.display_operation(operation, risk);
        }

        println!("\n{}", "=== End of Preview ===".bright_cyan().bold());
        Ok(())
    }

    /// Execute a single file operation
    async fn execute_operation(&self, operation: &FileOperation) -> Result<()> {
        if self.dry_run {
            println!("{}", format!("DRY RUN: Would execute {:?}", operation).yellow());
            return Ok(());
        }

        match operation {
            FileOperation::Create { path, content } => {
                if path.exists() {
                    return Err(anyhow::anyhow!("File already exists: {}", path.display()));
                }

                // Create parent directories if needed
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                std::fs::write(path, content)?;
                println!("{}", format!("Created: {}", path.display()).green());
                Ok(())
            }
            FileOperation::Read { path } => {
                let _content = std::fs::read_to_string(path)?;
                println!("{}", format!("Read: {}", path.display()).cyan());
                Ok(())
            }
            FileOperation::Update { path, old_content, new_content } => {
                if !path.exists() {
                    return Err(anyhow::anyhow!("File does not exist: {}", path.display()));
                }

                let current_content = std::fs::read_to_string(path)?;

                // Verify old content matches (safety check)
                if current_content != *old_content {
                    return Err(anyhow::anyhow!(
                        "File content has changed since plan creation: {}",
                        path.display()
                    ));
                }

                std::fs::write(path, new_content)?;
                println!("{}", format!("Updated: {}", path.display()).yellow());
                Ok(())
            }
            FileOperation::Delete { path } => {
                if !path.exists() {
                    return Err(anyhow::anyhow!("File does not exist: {}", path.display()));
                }

                std::fs::remove_file(path)?;
                println!("{}", format!("Deleted: {}", path.display()).red());
                Ok(())
            }
        }
    }

    /// Execute a build plan
    pub async fn execute_plan(&mut self, plan: &BuildPlan) -> Result<BuildResult> {
        let mut result = BuildResult {
            success: true,
            operations_completed: 0,
            operations_failed: 0,
            error_messages: Vec::new(),
            rollback_performed: false,
        };

        // TODO: Implement transaction framework (Phase 1.3)
        // For now, execute operations sequentially without transaction support

        for operation in &plan.operations {
            match self.execute_operation(operation).await {
                Ok(_) => {
                    result.operations_completed += 1;
                }
                Err(e) => {
                    result.operations_failed += 1;
                    result.success = false;
                    result.error_messages.push(format!("{:?}: {}", operation, e));
                    eprintln!("{}", format!("Operation failed: {}", e).red());

                    // TODO: Implement rollback (Phase 1.3)
                    println!("{}", "Transaction rollback not yet implemented (Phase 1.3)".yellow());
                    break;
                }
            }
        }

        Ok(result)
    }

    /// Create a build plan from a goal description
    /// This is a placeholder - in practice, this would use AI to generate the plan
    pub fn create_plan_from_goal(&self, goal: &str) -> Result<BuildPlan> {
        // This is a simplified version - the actual implementation would:
        // 1. Use AI agent to analyze the goal
        // 2. Identify files that need to be modified
        // 3. Generate specific file operations
        // 4. Assess overall risk

        Ok(BuildPlan {
            goal: goal.to_string(),
            operations: Vec::new(),
            description: format!("Build plan for: {}", goal),
            estimated_risk: RiskLevel::Low,
        })
    }

    /// Validate a build plan before execution
    pub fn validate_plan(&self, plan: &BuildPlan) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check for critical operations
        let critical_ops: Vec<_> = plan
            .operations
            .iter()
            .filter(|op| self.assess_risk(op) >= RiskLevel::High)
            .collect();

        if !critical_ops.is_empty() {
            warnings.push(format!(
                "Plan contains {} high-risk or critical operations",
                critical_ops.len()
            ));
        }

        // Check for conflicting operations
        let mut paths_modified = std::collections::HashSet::new();
        for operation in &plan.operations {
            let path = match operation {
                FileOperation::Create { path, .. }
                | FileOperation::Read { path }
                | FileOperation::Update { path, .. }
                | FileOperation::Delete { path } => path,
            };

            if !paths_modified.insert(path.clone()) {
                warnings.push(format!(
                    "Path modified multiple times in plan: {}",
                    path.display()
                ));
            }
        }

        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_assessment() {
        let service = BuildService::new("/tmp");

        let create_op = FileOperation::Create {
            path: PathBuf::from("/tmp/test.txt"),
            content: "test".to_string(),
        };
        assert_eq!(service.assess_risk(&create_op), RiskLevel::Low);

        let delete_critical = FileOperation::Delete {
            path: PathBuf::from("/tmp/Cargo.toml"),
        };
        assert_eq!(service.assess_risk(&delete_critical), RiskLevel::Critical);
    }

    #[test]
    fn test_is_critical_path() {
        let service = BuildService::new("/tmp");

        assert!(service.is_critical_path(Path::new("/tmp/Cargo.toml")));
        assert!(service.is_critical_path(Path::new("/etc/passwd")));
        assert!(!service.is_critical_path(Path::new("/tmp/test.txt")));
    }
}
