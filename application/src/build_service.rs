use crate::transaction::Transaction;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use shared::confirmation::ask_confirmation;
use shared::types::Result;
use std::path::{Path, PathBuf};

/// Represents a file operation in the build process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOperation {
    Create {
        path: PathBuf,
        content: String,
    },
    Read {
        path: PathBuf,
    },
    Update {
        path: PathBuf,
        old_content: String,
        new_content: String,
    },
    Delete {
        path: PathBuf,
    },
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

/// Confirmation mode for build operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationMode {
    /// Ask for confirmation on each operation
    Interactive,
    /// Confirm all operations at once
    ConfirmAll,
    /// No confirmation (auto-approve)
    None,
}

/// Represents a complex operation involving multiple files and dependencies
#[derive(Debug, Clone)]
pub struct ComplexOperation {
    pub name: String,
    pub description: String,
    pub file_operations: Vec<FileOperation>,
    pub dependencies: Vec<String>, // Names of other operations this depends on
    pub estimated_risk: RiskLevel,
    pub validation_rules: Vec<ValidationRule>,
}

/// Validation rules for operations
#[derive(Debug, Clone)]
pub enum ValidationRule {
    FileExists(String),
    FileNotExists(String),
    DirectoryExists(String),
    HasDependency(String),
    ContentContains(String, String), // file_path, pattern
}

/// Graph for managing operation dependencies and execution order
#[derive(Debug, Clone)]
pub struct OperationGraph {
    operations: Vec<ComplexOperation>,
    execution_order: Vec<usize>, // Indices in dependency-safe order
    validated: bool,
}

impl OperationGraph {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            execution_order: Vec::new(),
            validated: false,
        }
    }

    pub fn add_operation(&mut self, operation: ComplexOperation) {
        self.operations.push(operation);
        self.validated = false; // Invalidate on changes
    }

    pub fn validate_and_order(&mut self, workspace_root: &Path) -> Result<()> {
        // Check for circular dependencies
        self.detect_circular_dependencies()?;

        // Validate all operations
        for operation in &self.operations {
            self.validate_operation(operation, workspace_root)?;
        }

        // Compute execution order using topological sort
        self.compute_execution_order()?;

        self.validated = true;
        Ok(())
    }

    pub fn get_execution_order(&self) -> Result<&[usize]> {
        if !self.validated {
            return Err(anyhow::anyhow!(
                "Operation graph must be validated before getting execution order"
            ));
        }
        Ok(&self.execution_order)
    }

    pub fn get_operation(&self, index: usize) -> Option<&ComplexOperation> {
        self.operations.get(index)
    }

    fn detect_circular_dependencies(&self) -> Result<()> {
        // Simple cycle detection using DFS
        let mut visited = vec![false; self.operations.len()];
        let mut recursion_stack = vec![false; self.operations.len()];

        for i in 0..self.operations.len() {
            if self.has_cycle(i, &mut visited, &mut recursion_stack)? {
                return Err(anyhow::anyhow!(
                    "Circular dependency detected in operation graph"
                ));
            }
        }

        Ok(())
    }

    fn has_cycle(
        &self,
        node: usize,
        visited: &mut [bool],
        recursion_stack: &mut [bool],
    ) -> Result<bool> {
        if recursion_stack[node] {
            return Ok(true);
        }

        if visited[node] {
            return Ok(false);
        }

        visited[node] = true;
        recursion_stack[node] = true;

        // Check dependencies
        for dep_name in &self.operations[node].dependencies {
            if let Some(dep_index) = self.find_operation_index(dep_name) {
                if self.has_cycle(dep_index, visited, recursion_stack)? {
                    return Ok(true);
                }
            }
        }

        recursion_stack[node] = false;
        Ok(false)
    }

    fn find_operation_index(&self, name: &str) -> Option<usize> {
        self.operations.iter().position(|op| op.name == name)
    }

    fn validate_operation(
        &self,
        operation: &ComplexOperation,
        workspace_root: &Path,
    ) -> Result<()> {
        for rule in &operation.validation_rules {
            match rule {
                ValidationRule::FileExists(path) => {
                    let full_path = workspace_root.join(path);
                    if !full_path.exists() {
                        return Err(anyhow::anyhow!(
                            "Validation failed: file {} does not exist",
                            path
                        ));
                    }
                }
                ValidationRule::FileNotExists(path) => {
                    let full_path = workspace_root.join(path);
                    if full_path.exists() {
                        return Err(anyhow::anyhow!(
                            "Validation failed: file {} already exists",
                            path
                        ));
                    }
                }
                ValidationRule::DirectoryExists(path) => {
                    let full_path = workspace_root.join(path);
                    if !full_path.is_dir() {
                        return Err(anyhow::anyhow!(
                            "Validation failed: directory {} does not exist",
                            path
                        ));
                    }
                }
                ValidationRule::HasDependency(dep_name) => {
                    if !self.operations.iter().any(|op| op.name == *dep_name) {
                        return Err(anyhow::anyhow!(
                            "Validation failed: dependency {} not found",
                            dep_name
                        ));
                    }
                }
                ValidationRule::ContentContains(file_path, pattern) => {
                    let full_path = workspace_root.join(file_path);
                    if full_path.exists() {
                        match std::fs::read_to_string(&full_path) {
                            Ok(content) => {
                                if !content.contains(pattern) {
                                    return Err(anyhow::anyhow!(
                                        "Validation failed: {} does not contain {}",
                                        file_path,
                                        pattern
                                    ));
                                }
                            }
                            Err(_) => {
                                return Err(anyhow::anyhow!(
                                    "Validation failed: cannot read {}",
                                    file_path
                                ))
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn compute_execution_order(&mut self) -> Result<()> {
        let mut in_degree = vec![0; self.operations.len()];
        let mut queue = std::collections::VecDeque::new();
        let mut result = Vec::new();

        // Calculate in-degrees
        for (i, operation) in self.operations.iter().enumerate() {
            for dep_name in &operation.dependencies {
                if let Some(dep_index) = self.find_operation_index(dep_name) {
                    in_degree[i] += 1;
                }
            }
        }

        // Find operations with no dependencies
        for (i, &degree) in in_degree.iter().enumerate() {
            if degree == 0 {
                queue.push_back(i);
            }
        }

        // Topological sort
        while let Some(node) = queue.pop_front() {
            result.push(node);

            // For each operation that depends on this one
            for (i, operation) in self.operations.iter().enumerate() {
                if operation.dependencies.contains(&self.operations[node].name) {
                    in_degree[i] -= 1;
                    if in_degree[i] == 0 {
                        queue.push_back(i);
                    }
                }
            }
        }

        if result.len() != self.operations.len() {
            return Err(anyhow::anyhow!(
                "Cannot resolve operation dependencies - possible cycle"
            ));
        }

        self.execution_order = result;
        Ok(())
    }
}

/// Service for managing build mode operations
pub struct BuildService {
    /// Workspace root directory
    workspace_root: PathBuf,
    /// Enable dry-run mode (preview only)
    dry_run: bool,
    /// Confirmation mode
    confirmation_mode: ConfirmationMode,
    /// Whether to show diffs for operations
    show_diff: bool,
    /// Whether to show verbose previews
    verbose: bool,
    /// Buffered operations for incremental streaming
    buffered_operations: Vec<FileOperation>,
    /// Complex operations graph for dependency management
    operation_graph: OperationGraph,
    /// Project root for strict scoping (prevents system file access)
    project_root: PathBuf,
    /// Cached project scan for performance optimization
    cached_project_scan: Option<ProjectScanCache>,
}

/// Cached project scan information for performance
#[derive(Debug, Clone)]
struct ProjectScanCache {
    /// When the scan was performed
    scanned_at: std::time::SystemTime,
    /// List of files found
    files: Vec<PathBuf>,
    /// Total size in bytes
    total_size: u64,
    /// Whether this is a git repository
    is_git_repo: bool,
}

impl BuildService {
    /// Create a new BuildService with project scoping
    pub fn new<P: AsRef<Path>>(workspace_root: P) -> Self {
        let workspace_path = workspace_root.as_ref().to_path_buf();
        // Detect project root (git repo or current dir)
        let project_root = Self::detect_project_root(&workspace_path);

        Self {
            workspace_root: workspace_path,
            dry_run: false,
            confirmation_mode: ConfirmationMode::Interactive,
            show_diff: false,
            verbose: false,
            buffered_operations: Vec::new(),
            operation_graph: OperationGraph::new(),
            project_root,
            cached_project_scan: None,
        }
    }

    /// Detect project root (nearest .git directory or current directory)
    fn detect_project_root(workspace_root: &Path) -> PathBuf {
        let mut current = workspace_root.to_path_buf();
        loop {
            // Check for project indicators
            let project_files = [
                ".git",
                "Cargo.toml",
                "package.json",
                "requirements.txt",
                "Pipfile",
                "pyproject.toml",
                "setup.py",
                "Makefile",
                "CMakeLists.txt",
                "configure.ac",
                "go.mod",
                "Gemfile",
                "composer.json",
            ];

            for file in &project_files {
                if current.join(file).exists() {
                    return current;
                }
            }

            if !current.pop() {
                // No project root found, use workspace root
                return workspace_root.to_path_buf();
            }
        }
    }

    fn strip_fences_for_preview(content: &str) -> String {
        let trimmed = content.trim();
        if trimmed.starts_with("```") && trimmed.ends_with("```") {
            let mut lines: Vec<&str> = trimmed.lines().collect();
            if !lines.is_empty()
                && lines
                    .first()
                    .map(|l| l.trim().starts_with("```"))
                    .unwrap_or(false)
            {
                lines.remove(0);
            }
            if !lines.is_empty() && lines.last().map(|l| l.trim() == "```").unwrap_or(false) {
                lines.pop();
            }
            return lines.join("\n").trim().to_string();
        }
        trimmed.trim_matches('`').trim().to_string()
    }

    /// Enable or disable dry-run mode
    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.dry_run = dry_run;
    }

    /// Enable or disable diff previews
    pub fn set_show_diff(&mut self, show_diff: bool) {
        self.show_diff = show_diff;
    }

    /// Enable verbose previews
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// Set confirmation mode
    pub fn set_confirmation_mode(&mut self, mode: ConfirmationMode) {
        self.confirmation_mode = mode;
    }

    /// Assess risk level of a file operation with project scoping
    pub fn assess_risk(&self, operation: &FileOperation) -> RiskLevel {
        let path = match operation {
            FileOperation::Read { path } => path,
            FileOperation::Create { path, .. } => path,
            FileOperation::Update { path, .. } => path,
            FileOperation::Delete { path } => path,
        };

        // First, validate project scoping - if outside project, critical risk
        if !self.is_path_in_project(path) {
            return RiskLevel::Critical;
        }

        match operation {
            FileOperation::Read { .. } => {
                if self.is_critical_path(path) {
                    RiskLevel::High
                } else {
                    RiskLevel::Low
                }
            }
            FileOperation::Create { .. } => {
                if self.is_critical_path(path) {
                    RiskLevel::High
                } else {
                    RiskLevel::Low
                }
            }
            FileOperation::Update { .. } => {
                if self.is_critical_path(path) {
                    RiskLevel::Critical
                } else {
                    RiskLevel::Medium
                }
            }
            FileOperation::Delete { .. } => {
                if self.is_critical_path(path) {
                    RiskLevel::Critical
                } else {
                    RiskLevel::High
                }
            }
        }
    }

    /// Check if a path is within the project root (strict scoping)
    fn is_path_in_project(&self, path: &Path) -> bool {
        path.starts_with(&self.project_root)
    }

    /// Validate a path against project boundaries - rejects any path outside project
    fn validate_project_path(&self, path: &Path) -> Result<()> {
        if !self.is_path_in_project(path) {
            return Err(anyhow::anyhow!(
                "REJECTED: Path '{}' is outside project root '{}'. Operation blocked for safety.",
                path.display(),
                self.project_root.display()
            ));
        }
        Ok(())
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

        critical_patterns
            .iter()
            .any(|pattern| path_str.contains(pattern))
    }

    /// Display a file operation in plain text
    pub fn display_operation(&self, operation: &FileOperation, risk: RiskLevel) {
        let risk_label = format!("[{:?}]", risk);

        match operation {
            FileOperation::Create { path, .. } => {
                println!("  {} CREATE: {}", risk_label, path.display());
            }
            FileOperation::Read { path } => {
                println!("  {} READ: {}", risk_label, path.display());
            }
            FileOperation::Update { path, .. } => {
                println!("  {} UPDATE: {}", risk_label, path.display());
            }
            FileOperation::Delete { path } => {
                println!("  {} DELETE: {}", risk_label, path.display());
            }
        }
    }

    /// Preview a build plan in plain text
    pub fn preview_plan(&self, plan: &BuildPlan) -> Result<()> {
        println!("\n[BUILD_PLAN_PREVIEW]");
        println!("Goal: {}", plan.goal);
        println!("Description: {}", plan.description);
        println!("Estimated Risk: {:?}", plan.estimated_risk);
        println!("\nPlanned Operations:");

        for (i, operation) in plan.operations.iter().enumerate() {
            println!("\nOperation {}/{}:", i + 1, plan.operations.len());
            let risk = self.assess_risk(operation);
            self.display_operation(operation, risk);

            if self.verbose {
                match operation {
                    FileOperation::Create { content, .. } => {
                        println!("\nContent preview:");
                        let cleaned = Self::strip_fences_for_preview(content);
                        let snippet = if cleaned.len() > 200 {
                            &cleaned[..200]
                        } else {
                            &cleaned
                        };
                        println!("    {}", snippet);
                        if cleaned.len() > 200 {
                            println!("    ... ({} more chars)", cleaned.len() - 200);
                        }
                    }
                    FileOperation::Update { new_content, .. } => {
                        println!("\nContent preview:");
                        let cleaned = Self::strip_fences_for_preview(new_content);
                        let snippet = if cleaned.len() > 200 {
                            &cleaned[..200]
                        } else {
                            &cleaned
                        };
                        println!("    {}", snippet);
                        if cleaned.len() > 200 {
                            println!("    ... ({} more chars)", cleaned.len() - 200);
                        }
                    }
                    _ => {}
                }
            }
        }

        println!("\n[END_PREVIEW]");
        Ok(())
    }

    /// Display detailed operation preview with content
    pub fn display_operation_detail(&self, operation: &FileOperation) -> Result<()> {
        let risk = self.assess_risk(operation);

        println!("\n{}", "─".repeat(60));
        self.display_operation(operation, risk);

        match operation {
            FileOperation::Create { path, content } => {
                println!("\nContent to be created:");
                let cleaned = Self::strip_fences_for_preview(content);
                let preview = if cleaned.len() > 500 {
                    format!("{}... ({} bytes total)", &cleaned[..500], cleaned.len())
                } else {
                    cleaned
                };
                println!("{}", preview);
            }
            FileOperation::Update {
                path,
                old_content,
                new_content,
            } => {
                println!("\nChanges:");
                if self.show_diff || self.verbose {
                    self.display_diff(old_content, new_content);
                } else {
                    println!("{}", "(use --show-diff for detailed diff)");
                }
            }
            FileOperation::Delete { path } => {
                if path.exists() {
                    let size = std::fs::metadata(path)?.len();
                    println!("File size: {} bytes", size);
                }
            }
            FileOperation::Read { .. } => {
                // No additional details for read operations
            }
        }

        Ok(())
    }

    /// Display a simple diff between old and new content
    fn display_diff(&self, old_content: &str, new_content: &str) {
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        let max_lines = old_lines.len().max(new_lines.len()).min(20);

        for i in 0..max_lines {
            match (old_lines.get(i), new_lines.get(i)) {
                (Some(old_line), Some(new_line)) => {
                    if old_line != new_line {
                        println!("{}", format!("- {}", old_line).red());
                        println!("{}", format!("+ {}", new_line).green());
                    } else {
                        println!("{}", format!("  {}", old_line).bright_black());
                    }
                }
                (Some(old_line), None) => {
                    println!("{}", format!("- {}", old_line).red());
                }
                (None, Some(new_line)) => {
                    println!("{}", format!("+ {}", new_line).green());
                }
                (None, None) => break,
            }
        }

        if old_lines.len() > max_lines || new_lines.len() > max_lines {
            println!("... (diff truncated)");
        }
    }

    /// Ask for user confirmation for an operation
    fn confirm_operation(
        &self,
        operation: &FileOperation,
        operation_num: usize,
        total_ops: usize,
    ) -> Result<bool> {
        match self.confirmation_mode {
            ConfirmationMode::None => Ok(true),
            ConfirmationMode::ConfirmAll => {
                // Already confirmed at plan level
                Ok(true)
            }
            ConfirmationMode::Interactive => {
                let risk = self.assess_risk(operation);

                // Show detailed preview
                self.display_operation_detail(operation)?;

                // Higher risk operations default to 'no'
                let default_yes = risk <= RiskLevel::Medium;

                let prompt = format!(
                    "\nProceed with operation {}/{}?",
                    operation_num + 1,
                    total_ops
                );

                ask_confirmation(&prompt, default_yes)
            }
        }
    }

    /// Ask for confirmation to execute entire plan
    pub fn confirm_plan(&self, plan: &BuildPlan) -> Result<bool> {
        match self.confirmation_mode {
            ConfirmationMode::None => Ok(true),
            ConfirmationMode::Interactive | ConfirmationMode::ConfirmAll => {
                println!();
                ask_confirmation(
                    &format!(
                        "Execute this build plan ({} operations, estimated {:?} risk)?",
                        plan.operations.len(),
                        plan.estimated_risk
                    ),
                    false,
                )
            }
        }
    }

    /// Execute a single file operation
    async fn execute_operation(&self, operation: &FileOperation) -> Result<()> {
        if self.dry_run {
            println!("{}", format!("DRY RUN: Would execute {:?}", operation));
            return Ok(());
        }

        // Validate project scoping before any operation
        let path = match operation {
            FileOperation::Read { path } => path,
            FileOperation::Create { path, .. } => path,
            FileOperation::Update { path, .. } => path,
            FileOperation::Delete { path } => path,
        };
        self.validate_project_path(path)?;

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
                println!("{}", format!("Created: {}", path.display()));
                Ok(())
            }
            FileOperation::Read { path } => {
                let _content = std::fs::read_to_string(path)?;
                println!("{}", format!("Read: {}", path.display()));
                Ok(())
            }
            FileOperation::Update {
                path,
                old_content,
                new_content,
            } => {
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
                println!("{}", format!("Updated: {}", path.display()));
                Ok(())
            }
            FileOperation::Delete { path } => {
                if !path.exists() {
                    return Err(anyhow::anyhow!("File does not exist: {}", path.display()));
                }

                std::fs::remove_file(path)?;
                println!("{}", format!("Deleted: {}", path.display()));
                Ok(())
            }
        }
    }

    /// Execute a build plan with transaction support and user confirmation
    pub async fn execute_plan(&mut self, plan: &BuildPlan) -> Result<BuildResult> {
        let mut result = BuildResult {
            success: true,
            operations_completed: 0,
            operations_failed: 0,
            error_messages: Vec::new(),
            rollback_performed: false,
        };

        // Get plan-level confirmation if needed
        if !self.confirm_plan(plan)? {
            println!("{}", "Build plan cancelled by user.");
            return Ok(result);
        }

        // Create transaction for atomic operations
        let mut transaction = Transaction::new();
        transaction.begin()?;

        println!("\n[EXECUTING] {} operations...", plan.operations.len());

        let total_ops = plan.operations.len();

        // Execute operations within transaction
        for (idx, operation) in plan.operations.iter().enumerate() {
            // Get operation-level confirmation in interactive mode
            if self.confirmation_mode == ConfirmationMode::Interactive {
                if !self.confirm_operation(operation, idx, total_ops)? {
                    println!("{}", "Operation skipped by user.");
                    continue;
                }
            }

            match self
                .execute_operation_transactional(operation, &mut transaction)
                .await
            {
                Ok(_) => {
                    result.operations_completed += 1;
                }
                Err(e) => {
                    result.operations_failed += 1;
                    result.success = false;
                    result
                        .error_messages
                        .push(format!("{:?}: {}", operation, e));
                    eprintln!("{}", format!("Operation failed: {}", e));

                    // Ask if user wants to rollback
                    let should_rollback = if self.confirmation_mode == ConfirmationMode::Interactive
                    {
                        ask_confirmation("Rollback all changes?", true)?
                    } else {
                        true // Auto-rollback in non-interactive mode
                    };

                    if should_rollback {
                        println!("{}", "Rolling back all operations...");
                        transaction.rollback()?;
                        result.rollback_performed = true;
                    }
                    break;
                }
            }
        }

        // Commit transaction if all operations succeeded
        if result.success {
            transaction.commit()?;

            // Auto-commit to git if available
            if let Err(e) = self.git_commit_changes(plan).await {
                eprintln!("{} {}", "Warning: Git commit failed:", e);
                // Don't fail the build for git issues
            }
        }

        // Print summary
        println!("\n[BUILD_SUMMARY]");
        println!("Operations completed: {}", result.operations_completed);
        if result.operations_failed > 0 {
            println!("Operations failed: {}", result.operations_failed);
        }
        if result.rollback_performed {
            println!("Transaction rolled back");
        } else if result.success {
            println!("All changes committed successfully");
        }

        Ok(result)
    }

    /// Auto-commit changes to git if repository exists
    async fn git_commit_changes(&self, plan: &BuildPlan) -> Result<()> {
        // Check if we're in a git repository
        let repo_path = std::env::current_dir()?;
        if !repo_path.join(".git").exists() {
            return Ok(()); // Not a git repo, skip
        }

        // Create commit message
        let commit_msg = format!(
            "feat: {}\n\nApplied {} operations via elite agentic CLI\n\nOperations:\n{}",
            plan.goal,
            plan.operations.len(),
            plan.operations
                .iter()
                .map(|op| format!("- {:?}", op))
                .collect::<Vec<_>>()
                .join("\n")
        );

        self.commit_message(&commit_msg).await?;

        println!("[COMMIT] Changes committed to git");
        Ok(())
    }

    /// Execute a single file operation within a transaction
    async fn execute_operation_transactional(
        &self,
        operation: &FileOperation,
        transaction: &mut Transaction,
    ) -> Result<()> {
        if self.dry_run {
            println!("{}", format!("DRY RUN: Would execute {:?}", operation));
            return Ok(());
        }

        // Validate project scoping before any operation
        let path = match operation {
            FileOperation::Read { path } => path,
            FileOperation::Create { path, .. } => path,
            FileOperation::Update { path, .. } => path,
            FileOperation::Delete { path } => path,
        };
        self.validate_project_path(path)?;

        match operation {
            FileOperation::Create { path, content } => {
                if path.exists() {
                    return Err(anyhow::anyhow!("File already exists: {}", path.display()));
                }

                transaction.write_file(path, content.as_bytes())?;
                println!("{}", format!("Created: {}", path.display()));
                Ok(())
            }
            FileOperation::Read { path } => {
                let _content = std::fs::read_to_string(path)?;
                println!("{}", format!("Read: {}", path.display()));
                Ok(())
            }
            FileOperation::Update {
                path,
                old_content,
                new_content,
            } => {
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

                transaction.write_file(path, new_content.as_bytes())?;
                println!("{}", format!("Updated: {}", path.display()));
                Ok(())
            }
            FileOperation::Delete { path } => {
                if !path.exists() {
                    return Err(anyhow::anyhow!("File does not exist: {}", path.display()));
                }

                transaction.delete_file(path)?;
                println!("{}", format!("Deleted: {}", path.display()));
                Ok(())
            }
        }
    }

    /// Create a build plan from a goal description using AI agent
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

    /// Buffer an operation for incremental streaming
    pub fn buffer_operation(&mut self, operation: FileOperation) {
        self.buffered_operations.push(operation);
    }

    /// Replace buffered operations (used by agentic planners)
    pub fn set_buffered_operations(&mut self, operations: Vec<FileOperation>) {
        self.buffered_operations = operations;
    }

    /// Filter operations to enforce project scoping and flag anything outside the workspace
    pub fn enforce_project_scope(
        &self,
        operations: Vec<FileOperation>,
    ) -> (Vec<FileOperation>, Vec<String>) {
        let mut sanitized = Vec::new();
        let mut warnings = Vec::new();

        for op in operations {
            let path = match &op {
                FileOperation::Create { path, .. }
                | FileOperation::Read { path }
                | FileOperation::Update { path, .. }
                | FileOperation::Delete { path } => path,
            };

            // Reject paths that escape the workspace root
            if path.is_absolute() && !path.starts_with(&self.workspace_root) {
                warnings.push(format!(
                    "Skipping operation on external path: {}",
                    path.display()
                ));
                continue;
            }

            sanitized.push(op);
        }

        (sanitized, warnings)
    }

    /// Stream a file operation in plain text
    pub fn stream_operation(
        &self,
        operation: &FileOperation,
        step_number: usize,
        total_steps: usize,
    ) -> Result<()> {
        println!("\n[STEP] {}/{}", step_number, total_steps);

        let risk = self.assess_risk(operation);
        let risk_label = format!("[{:?}]", risk);

        match operation {
            FileOperation::Create { path, content } => {
                println!("{} Creating: {}", risk_label, path.display());
                println!("\nCode preview:");

                // Show first 20 lines in plain text
                let lines: Vec<&str> = content.lines().collect();
                let preview_lines = lines.iter().take(20);

                for (i, line) in preview_lines.enumerate() {
                    println!("{:3} {}", i + 1, line);
                }

                if lines.len() > 20 {
                    println!("... ({} more lines)", lines.len() - 20);
                }
            }
            FileOperation::Update {
                path,
                old_content,
                new_content,
            } => {
                println!("{} Updating: {}", risk_label, path.display());
                if self.show_diff {
                    println!("\nChanges:");
                    self.display_diff(old_content, new_content);
                }
            }
            FileOperation::Read { path } => {
                println!("{} Reading: {}", risk_label, path.display());
            }
            FileOperation::Delete { path } => {
                println!("{} Deleting: {}", risk_label, path.display());
                if path.exists() {
                    let size = std::fs::metadata(path)?.len();
                    println!("  File size: {} bytes", size);
                }
            }
        }

        Ok(())
    }

    /// Get buffered operations count
    pub fn buffered_count(&self) -> usize {
        self.buffered_operations.len()
    }

    /// Get reference to buffered operations
    pub fn get_buffered_operations(&self) -> &[FileOperation] {
        &self.buffered_operations
    }

    /// Add a complex operation to the graph
    pub fn add_complex_operation(&mut self, operation: ComplexOperation) {
        self.operation_graph.add_operation(operation);
    }

    /// Validate and order complex operations
    pub fn validate_complex_operations(&mut self) -> Result<()> {
        self.operation_graph
            .validate_and_order(&self.workspace_root)
    }

    /// Get complex operations in execution order
    pub fn get_complex_execution_order(&self) -> Result<Vec<&ComplexOperation>> {
        let indices = self.operation_graph.get_execution_order()?;
        let mut operations = Vec::new();

        for &index in indices {
            if let Some(op) = self.operation_graph.get_operation(index) {
                operations.push(op);
            }
        }

        Ok(operations)
    }

    /// Execute complex operations in dependency order
    pub async fn execute_complex_operations(&mut self) -> Result<BuildResult> {
        let operations = self.get_complex_execution_order()?;
        let mut result = BuildResult {
            success: true,
            operations_completed: 0,
            operations_failed: 0,
            error_messages: Vec::new(),
            rollback_performed: false,
        };

        // Get plan-level confirmation if needed
        if !self.confirm_plan_for_complex(&operations)? {
            println!("{}", "Complex operations cancelled by user.");
            return Ok(result);
        }

        println!(
            "\n{}",
            format!(
                "Executing {} complex operations in dependency order...",
                operations.len()
            )
        );

        let mut transaction = crate::transaction::Transaction::new();
        transaction.begin()?;

        for (idx, operation) in operations.iter().enumerate() {
            println!(
                "\n{}",
                format!(
                    "Executing complex operation {}/{}: {}",
                    idx + 1,
                    operations.len(),
                    operation.name
                )
            );

            // Get operation-level confirmation in interactive mode
            if self.confirmation_mode == ConfirmationMode::Interactive {
                if !self.confirm_complex_operation(operation, idx, operations.len())? {
                    println!("{}", "Complex operation skipped by user.");
                    continue;
                }
            }

            match self
                .execute_complex_operation(operation, &mut transaction)
                .await
            {
                Ok(_) => {
                    result.operations_completed += operation.file_operations.len();
                }
                Err(e) => {
                    result.operations_failed += 1;
                    result.success = false;
                    result
                        .error_messages
                        .push(format!("Complex operation '{}': {}", operation.name, e));

                    eprintln!(
                        "{}",
                        format!("Complex operation '{}' failed: {}", operation.name, e)
                    );

                    // Ask if user wants to rollback
                    let should_rollback = if self.confirmation_mode == ConfirmationMode::Interactive
                    {
                        shared::confirmation::ask_confirmation(
                            "Rollback all complex operations?",
                            true,
                        )?
                    } else {
                        true // Auto-rollback in non-interactive mode
                    };

                    if should_rollback {
                        println!("{}", "Rolling back all complex operations...");
                        transaction.rollback()?;
                        result.rollback_performed = true;
                    }
                    break;
                }
            }
        }

        // Commit transaction if all operations succeeded
        if result.success {
            transaction.commit()?;
        }

        Ok(result)
    }

    /// Confirm plan for complex operations
    fn confirm_plan_for_complex(&self, operations: &[&ComplexOperation]) -> Result<bool> {
        match self.confirmation_mode {
            ConfirmationMode::None => Ok(true),
            ConfirmationMode::Interactive | ConfirmationMode::ConfirmAll => {
                println!("\n{}", "Complex Operations Plan Preview");

                for (i, op) in operations.iter().enumerate() {
                    println!(
                        "\n{}: {}",
                        format!("{}. {}", i + 1, op.name),
                        op.description
                    );
                    println!("  Files: {}", op.file_operations.len());
                    println!("  Risk: {:?}", op.estimated_risk);
                    if !op.dependencies.is_empty() {
                        println!("  Dependencies: {}", op.dependencies.join(", "));
                    }
                }

                let total_files = operations
                    .iter()
                    .map(|op| op.file_operations.len())
                    .sum::<usize>();
                let max_risk = operations
                    .iter()
                    .map(|op| op.estimated_risk)
                    .max()
                    .unwrap_or(RiskLevel::Low);

                shared::confirmation::ask_confirmation(
                    &format!(
                        "Execute {} complex operations ({} total files, estimated {:?} max risk)?",
                        operations.len(),
                        total_files,
                        max_risk
                    ),
                    false,
                )
            }
        }
    }

    /// Confirm individual complex operation
    fn confirm_complex_operation(
        &self,
        operation: &ComplexOperation,
        operation_num: usize,
        total_ops: usize,
    ) -> Result<bool> {
        println!(
            "\n{}",
            format!("Complex Operation {}/{}", operation_num + 1, total_ops)
        );
        println!("Name: {}", operation.name);
        println!("Description: {}", operation.description);
        println!("Risk: {:?}", operation.estimated_risk);
        println!("Files to modify: {}", operation.file_operations.len());

        for (i, file_op) in operation.file_operations.iter().enumerate() {
            println!("  {}. {:?}", i + 1, file_op);
        }

        shared::confirmation::ask_confirmation("Proceed with this complex operation?", true)
    }

    /// Execute a single complex operation
    async fn execute_complex_operation(
        &self,
        operation: &ComplexOperation,
        transaction: &mut crate::transaction::Transaction,
    ) -> Result<()> {
        for file_operation in &operation.file_operations {
            self.execute_operation_transactional(file_operation, transaction)
                .await?;
        }
        Ok(())
    }

    /// Clear all buffered operations
    pub fn clear_buffer(&mut self) {
        self.buffered_operations.clear();
    }

    /// Apply all buffered operations atomically
    pub async fn apply_buffered_operations(&mut self) -> Result<BuildResult> {
        let operations = std::mem::take(&mut self.buffered_operations);
        let plan = BuildPlan {
            goal: "Incremental build".to_string(),
            operations,
            description: "Buffered operations from incremental streaming".to_string(),
            estimated_risk: RiskLevel::Low, // Will be recalculated
        };

        // Recalculate risk
        let actual_risk = plan
            .operations
            .iter()
            .map(|op| self.assess_risk(op))
            .max()
            .unwrap_or(RiskLevel::Low);

        let plan_with_risk = BuildPlan {
            estimated_risk: actual_risk,
            ..plan
        };

        self.execute_plan(&plan_with_risk).await
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

    /// Execute a single operation with its own transaction
    pub async fn execute_operation_once(&self, operation: &FileOperation) -> Result<()> {
        let mut transaction = Transaction::new();
        transaction.begin()?;
        self.execute_operation_transactional(operation, &mut transaction)
            .await?;
        transaction.commit()?;
        Ok(())
    }

    /// Commit current working tree with a custom message
    pub async fn commit_message(&self, message: &str) -> Result<()> {
        let repo_path = std::env::current_dir()?;
        if !repo_path.join(".git").exists() {
            return Ok(());
        }

        let repo = git2::Repository::open(&repo_path)
            .map_err(|e| anyhow::anyhow!("Failed to open git repository: {}", e))?;

        let mut index = repo
            .index()
            .map_err(|e| anyhow::anyhow!("Failed to get git index: {}", e))?;

        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| anyhow::anyhow!("Failed to add files to git index: {}", e))?;
        index
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to write git index: {}", e))?;

        let sig = git2::Signature::now("Elite Agentic CLI", "agent@cli.local")
            .map_err(|e| anyhow::anyhow!("Failed to create git signature: {}", e))?;

        let head_commit = match repo.head() {
            Ok(head) => {
                let oid = head.target().unwrap();
                Some(
                    repo.find_commit(oid)
                        .map_err(|e| anyhow::anyhow!("Failed to find head commit: {}", e))?,
                )
            }
            Err(_) => None,
        };

        let parents = if let Some(ref commit) = head_commit {
            vec![commit]
        } else {
            vec![]
        };

        let tree_oid = index
            .write_tree()
            .map_err(|e| anyhow::anyhow!("Failed to write tree: {}", e))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| anyhow::anyhow!("Failed to find tree: {}", e))?;

        let _commit_oid = repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .map_err(|e| anyhow::anyhow!("Failed to create commit: {}", e))?;

        Ok(())
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

    #[test]
    fn test_enforce_project_scope_filters_external_paths() {
        let service = BuildService::new("/tmp/project");

        let ops = vec![
            FileOperation::Create {
                path: PathBuf::from("/tmp/project/health.sh"),
                content: "echo ok".to_string(),
            },
            FileOperation::Delete {
                path: PathBuf::from("/etc/passwd"),
            },
            FileOperation::Read {
                path: PathBuf::from("relative.txt"),
            },
        ];

        let (scoped, warnings) = service.enforce_project_scope(ops);

        assert_eq!(scoped.len(), 2);
        assert_eq!(warnings.len(), 1);
        assert!(matches!(scoped[0], FileOperation::Create { .. }));
        assert!(matches!(scoped[1], FileOperation::Read { .. }));
    }
}
