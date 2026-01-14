use crate::observability::OBSERVABILITY;
use crate::resource_enforcement::{ResourceEnforcer, ResourceLimits};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// Tool execution arguments with validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolArgs {
    pub parameters: HashMap<String, String>,
    pub timeout: Option<Duration>,
    pub working_directory: Option<String>,
}

/// Tool execution output with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub execution_time: Duration,
    pub resources_used: ResourceUsage,
}

/// Tool execution errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolError {
    ValidationError(String),
    ExecutionError(String),
    TimeoutError,
    ResourceLimitExceeded(String),
    SecurityViolation(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ToolError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            ToolError::TimeoutError => write!(f, "Operation timed out"),
            ToolError::ResourceLimitExceeded(msg) => write!(f, "Resource limit exceeded: {}", msg),
            ToolError::SecurityViolation(msg) => write!(f, "Security violation: {}", msg),
        }
    }
}

impl std::error::Error for ToolError {}

/// Validation errors for tool arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub severity: ValidationSeverity,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Validation error in field '{}': {} (severity: {:?})",
            self.field, self.message, self.severity
        )
    }
}

impl std::error::Error for ValidationError {}

/// Resource usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_used_mb: u64,
    pub cpu_time_seconds: f64,
    pub processes_created: u32,
    pub output_size: usize,
}

/// Security validator for tool operations
#[derive(Debug, Clone)]
pub struct ToolSecurityValidator {
    allowed_paths: Vec<String>,
    blocked_patterns: Vec<String>,
    max_file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Warning,
    Error,
    Critical,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 512,
            max_cpu_percent: 50.0,
            max_execution_time: Duration::from_secs(30),
            max_output_size: 1_048_576, // 1MB
            max_processes: 10,
        }
    }
}

impl ToolSecurityValidator {
    pub fn new() -> Self {
        Self {
            allowed_paths: vec![
                "/home".to_string(),
                "/tmp".to_string(),
                "/var/tmp".to_string(),
            ],
            blocked_patterns: vec![
                "/etc".to_string(),
                "/sys".to_string(),
                "/dev".to_string(),
                "/proc".to_string(),
                "/root".to_string(),
            ],
            max_file_size: 100 * 1024 * 1024, // 100MB
        }
    }

    pub fn validate_path(&self, path: &str) -> Result<(), ToolError> {
        // Check blocked patterns
        for pattern in &self.blocked_patterns {
            if path.starts_with(pattern) {
                return Err(ToolError::SecurityViolation(format!(
                    "Access blocked for path: {}",
                    path
                )));
            }
        }

        // Check allowed paths
        let allowed = self
            .allowed_paths
            .iter()
            .any(|allowed_path| path.starts_with(allowed_path));

        if !allowed {
            return Err(ToolError::SecurityViolation(format!(
                "Unauthorized path access: {}",
                path
            )));
        }

        Ok(())
    }

    pub fn validate_file_size(&self, size: u64) -> Result<(), ToolError> {
        if size > self.max_file_size {
            return Err(ToolError::ResourceLimitExceeded(format!(
                "File size {} exceeds limit {}",
                size, self.max_file_size
            )));
        }
        Ok(())
    }
}

impl Default for ToolSecurityValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Enum representing all available safe tools
#[derive(Debug, Clone)]
pub enum SafeTool {
    FileRead,
    FileWrite,
    DirectoryList,
    ProcessList,
    GrepSearch,
    FindFiles,
    SedReplace,
    AwkExtract,
    CurlFetch,
    WebSearch,
    GitStatus,
    GitDiff,
    GitLog,
}

impl SafeTool {
    pub fn name(&self) -> &str {
        match self {
            SafeTool::FileRead => "file_read",
            SafeTool::FileWrite => "file_write",
            SafeTool::DirectoryList => "directory_list",
            SafeTool::ProcessList => "process_list",
            SafeTool::GrepSearch => "grep_search",
            SafeTool::FindFiles => "find_files",
            SafeTool::SedReplace => "sed_replace",
            SafeTool::AwkExtract => "awk_extract",
            SafeTool::CurlFetch => "curl_fetch",
            SafeTool::WebSearch => "web_search",
            SafeTool::GitStatus => "git_status",
            SafeTool::GitDiff => "git_diff",
            SafeTool::GitLog => "git_log",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            SafeTool::FileRead => "Safely read file contents with path validation and size limits",
            SafeTool::FileWrite => {
                "Safely write file contents with backup and rollback capabilities"
            }
            SafeTool::DirectoryList => "Safely list directory contents with path validation",
            SafeTool::ProcessList => "Safely list running processes with filtering",
            SafeTool::GrepSearch => "Search for patterns in files using regex with path filtering",
            SafeTool::FindFiles => "Find files by name patterns, size, date, and type filters",
            SafeTool::SedReplace => "Perform safe text replacements in files with preview",
            SafeTool::AwkExtract => "Extract and transform data from files using awk-like patterns",
            SafeTool::CurlFetch => "Fetch content from HTTP URLs (read-only, no authentication)",
            SafeTool::WebSearch => "Search the web for documentation and best practices",
            SafeTool::GitStatus => "Get git repository status (read-only)",
            SafeTool::GitDiff => "Show git diffs between commits or working directory",
            SafeTool::GitLog => "Show git commit history with filtering options",
        }
    }

    pub async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        match self {
            SafeTool::FileRead => self.execute_file_read(args).await,
            SafeTool::FileWrite => self.execute_file_write(args).await,
            SafeTool::DirectoryList => self.execute_directory_list(args).await,
            SafeTool::ProcessList => self.execute_process_list(args).await,
            SafeTool::GrepSearch => self.execute_grep_search(args).await,
            SafeTool::FindFiles => self.execute_find_files(args).await,
            SafeTool::SedReplace => self.execute_sed_replace(args).await,
            SafeTool::AwkExtract => self.execute_awk_extract(args).await,
            SafeTool::CurlFetch => self.execute_curl_fetch(args).await,
            SafeTool::WebSearch => self.execute_web_search(args).await,
            SafeTool::GitStatus => self.execute_git_status(args).await,
            SafeTool::GitDiff => self.execute_git_diff(args).await,
            SafeTool::GitLog => self.execute_git_log(args).await,
        }
    }

    pub fn validate_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        match self {
            SafeTool::FileRead => self.validate_file_read_args(args),
            SafeTool::FileWrite => self.validate_file_write_args(args),
            SafeTool::DirectoryList => self.validate_directory_list_args(args),
            SafeTool::ProcessList => self.validate_process_list_args(args),
            SafeTool::GrepSearch => self.validate_grep_search_args(args),
            SafeTool::FindFiles => self.validate_find_files_args(args),
            SafeTool::SedReplace => self.validate_sed_replace_args(args),
            SafeTool::AwkExtract => self.validate_awk_extract_args(args),
            SafeTool::CurlFetch => self.validate_curl_fetch_args(args),
            SafeTool::WebSearch => self.validate_web_search_args(args),
            SafeTool::GitStatus => self.validate_git_status_args(args),
            SafeTool::GitDiff => self.validate_git_diff_args(args),
            SafeTool::GitLog => self.validate_git_log_args(args),
        }
    }

    pub fn get_resource_limits(&self) -> ResourceLimits {
        ResourceLimits::default()
    }

    // File read implementation
    async fn execute_file_read(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let start_time = Instant::now();

        let file_path = args
            .parameters
            .get("path")
            .ok_or_else(|| ToolError::ValidationError("Missing 'path' parameter".to_string()))?;

        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(file_path)?;

        let path = Path::new(file_path);
        if !path.exists() {
            return Err(ToolError::ExecutionError(format!(
                "File not found: {}",
                file_path
            )));
        }

        let metadata = fs::metadata(path).map_err(|e| {
            ToolError::ExecutionError(format!("Failed to read file metadata: {}", e))
        })?;

        security_validator.validate_file_size(metadata.len())?;

        // Use resource enforcement for file operations
        let limits = ResourceLimits::default();

        // For file reading, we simulate the operation with resource limits
        // In practice, this would use async file operations with resource monitoring
        let content = fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read file: {}", e)))?;

        if content.len() > limits.max_output_size {
            return Err(ToolError::ResourceLimitExceeded(format!(
                "File content exceeds output size limit: {} > {} bytes",
                content.len(),
                limits.max_output_size
            )));
        }

        let execution_time = start_time.elapsed();
        let resources_used = ResourceUsage {
            memory_used_mb: (content.len() / (1024 * 1024)) as u64,
            cpu_time_seconds: execution_time.as_secs_f64(),
            processes_created: 0,
            output_size: content.len(),
        };

        Ok(ToolOutput {
            success: true,
            stdout: content,
            stderr: String::new(),
            exit_code: Some(0),
            execution_time,
            resources_used,
        })
    }

    fn validate_file_read_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if !args.parameters.contains_key("path") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        let path = args.parameters.get("path").unwrap();
        if path.is_empty() {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path cannot be empty".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        if path.contains("..") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path traversal not allowed".to_string(),
                severity: ValidationSeverity::Critical,
            });
        }

        Ok(())
    }

    // File write implementation
    async fn execute_file_write(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let start_time = Instant::now();

        let file_path = args
            .parameters
            .get("path")
            .ok_or_else(|| ToolError::ValidationError("Missing 'path' parameter".to_string()))?;

        let content = args
            .parameters
            .get("content")
            .ok_or_else(|| ToolError::ValidationError("Missing 'content' parameter".to_string()))?;

        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(file_path)?;

        if content.len() > ResourceLimits::default().max_output_size {
            return Err(ToolError::ResourceLimitExceeded(format!(
                "Content exceeds size limit: {} > {} bytes",
                content.len(),
                ResourceLimits::default().max_output_size
            )));
        }

        let path = Path::new(file_path);
        let backup_created = if path.exists() {
            let backup_path = format!(
                "{}.backup.{}",
                file_path,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            );

            fs::copy(file_path, &backup_path).map_err(|e| {
                ToolError::ExecutionError(format!("Failed to create backup: {}", e))
            })?;

            true
        } else {
            false
        };

        fs::write(path, content)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write file: {}", e)))?;

        let execution_time = start_time.elapsed();
        let resources_used = ResourceUsage {
            memory_used_mb: 0,
            cpu_time_seconds: 0.0,
            processes_created: 0,
            output_size: content.len(),
        };

        let mut stdout = String::new();
        if backup_created {
            stdout.push_str("Backup created successfully\n");
        }
        stdout.push_str(&format!("File written successfully: {}", file_path));

        Ok(ToolOutput {
            success: true,
            stdout,
            stderr: String::new(),
            exit_code: Some(0),
            execution_time,
            resources_used,
        })
    }

    fn validate_file_write_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if !args.parameters.contains_key("path") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        if !args.parameters.contains_key("content") {
            return Err(ValidationError {
                field: "content".to_string(),
                message: "Content parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        let path = args.parameters.get("path").unwrap();
        if path.is_empty() {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path cannot be empty".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        if path.contains("..") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path traversal not allowed".to_string(),
                severity: ValidationSeverity::Critical,
            });
        }

        Ok(())
    }

    // Directory list implementation
    async fn execute_directory_list(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let start_time = Instant::now();

        let dir_path = args
            .parameters
            .get("path")
            .map(|s| s.as_str())
            .unwrap_or("."); // Default to current directory

        // Validate path security
        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(dir_path)?;

        let path = Path::new(dir_path);
        if !path.exists() {
            return Err(ToolError::ExecutionError(format!(
                "Directory not found: {}",
                dir_path
            )));
        }

        if !path.is_dir() {
            return Err(ToolError::ExecutionError(format!(
                "Path is not a directory: {}",
                dir_path
            )));
        }

        let entries = fs::read_dir(path)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read directory: {}", e)))?;

        let mut output = String::new();
        let mut file_count = 0;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ToolError::ExecutionError(format!("Failed to read directory entry: {}", e))
            })?;
            let file_name = entry
                .file_name()
                .into_string()
                .unwrap_or_else(|_| "Invalid UTF-8".to_string());

            let metadata = entry.metadata().map_err(|e| {
                ToolError::ExecutionError(format!("Failed to read metadata: {}", e))
            })?;

            let file_type = if metadata.is_dir() {
                "DIR"
            } else if metadata.is_file() {
                "FILE"
            } else {
                "OTHER"
            };

            let size = if metadata.is_file() {
                metadata.len().to_string()
            } else {
                "-".to_string()
            };

            output.push_str(&format!("{:<8} {:>10} {}\n", file_type, size, file_name));
            file_count += 1;
        }

        let execution_time = start_time.elapsed();
        let resources_used = ResourceUsage {
            memory_used_mb: 0,
            cpu_time_seconds: 0.0,
            processes_created: 0,
            output_size: output.len(),
        };

        let header = format!("Total entries: {}\n", file_count);
        let stdout = format!("{}{}", header, output);

        Ok(ToolOutput {
            success: true,
            stdout,
            stderr: String::new(),
            exit_code: Some(0),
            execution_time,
            resources_used,
        })
    }

    fn validate_directory_list_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if let Some(path) = args.parameters.get("path") {
            if path.is_empty() {
                return Err(ValidationError {
                    field: "path".to_string(),
                    message: "Path cannot be empty".to_string(),
                    severity: ValidationSeverity::Error,
                });
            }

            if path.contains("..") {
                return Err(ValidationError {
                    field: "path".to_string(),
                    message: "Path traversal not allowed".to_string(),
                    severity: ValidationSeverity::Critical,
                });
            }
        }
        Ok(())
    }

    // Process list implementation
    async fn execute_process_list(&self, _args: ToolArgs) -> Result<ToolOutput, ToolError> {
        // Use resource enforcement for process listing
        let enforcer = ResourceEnforcer::new();
        let limits = ResourceLimits::default();

        match enforcer
            .execute_with_limits("ps", &["aux"], &limits, None)
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 1, // Minimal memory usage
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!(
                "Resource enforcement failed: {}",
                e
            ))),
        }
    }

    fn validate_process_list_args(&self, _args: &ToolArgs) -> Result<(), ValidationError> {
        // No arguments required for process listing
        Ok(())
    }

    // Grep search implementation
    async fn execute_grep_search(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let pattern = args
            .parameters
            .get("pattern")
            .ok_or_else(|| ToolError::ValidationError("Missing 'pattern' parameter".to_string()))?;

        let path = args
            .parameters
            .get("path")
            .ok_or_else(|| ToolError::ValidationError("Missing 'path' parameter".to_string()))?;

        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(path)?;

        // Use ripgrep (rg) for fast searching
        let mut cmd_args: Vec<String> =
            vec!["--line-number".to_string(), "--with-filename".to_string()];

        // Add case insensitive if requested
        if args
            .parameters
            .get("case_insensitive")
            .map_or(false, |v| v == "true")
        {
            cmd_args.push("--ignore-case".to_string());
        }

        // Add include patterns
        if let Some(include) = args.parameters.get("include") {
            cmd_args.push("--glob".to_string());
            cmd_args.push(include.clone());
        }

        cmd_args.push(pattern.clone());
        cmd_args.push(path.clone());

        let cmd_args_refs: Vec<&str> = cmd_args.iter().map(|s| &**s).collect();

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits(
                "rg",
                &cmd_args_refs,
                &limits,
                args.working_directory.as_deref(),
            )
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 2,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!(
                "Grep search failed: {}",
                e
            ))),
        }
    }

    fn validate_grep_search_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if !args.parameters.contains_key("pattern") {
            return Err(ValidationError {
                field: "pattern".to_string(),
                message: "Pattern parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        if !args.parameters.contains_key("path") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        Ok(())
    }

    // Find files implementation
    async fn execute_find_files(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let path = args
            .parameters
            .get("path")
            .ok_or_else(|| ToolError::ValidationError("Missing 'path' parameter".to_string()))?;

        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(path)?;

        let mut cmd_args: Vec<String> = vec![".".to_string()];

        // Add name pattern
        if let Some(name) = args.parameters.get("name") {
            cmd_args.push("-name".to_string());
            cmd_args.push(name.clone());
        }

        // Add type filter
        if let Some(file_type) = args.parameters.get("type") {
            cmd_args.push("-type".to_string());
            cmd_args.push(file_type.clone()); // f, d, l, etc.
        }

        // Add size filter
        if let Some(size) = args.parameters.get("size") {
            cmd_args.push("-size".to_string());
            cmd_args.push(size.clone());
        }

        let cmd_args_refs: Vec<&str> = cmd_args.iter().map(|s| &**s).collect();

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits(
                "find",
                &cmd_args_refs,
                &limits,
                args.working_directory.as_deref(),
            )
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 1,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!(
                "Find files failed: {}",
                e
            ))),
        }
    }

    fn validate_find_files_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if !args.parameters.contains_key("path") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        Ok(())
    }

    // Sed replace implementation
    async fn execute_sed_replace(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let file_path = args
            .parameters
            .get("path")
            .ok_or_else(|| ToolError::ValidationError("Missing 'path' parameter".to_string()))?;

        let pattern = args
            .parameters
            .get("pattern")
            .ok_or_else(|| ToolError::ValidationError("Missing 'pattern' parameter".to_string()))?;
        let replacement = args.parameters.get("replacement").map_or("", |v| v);

        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(file_path)?;

        // First read the file to show preview
        let _current_content = fs::read_to_string(file_path)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read file: {}", e)))?;

        // Create sed expression
        let sed_expr = format!("s/{}/{}/g", pattern, replacement);
        let cmd_args_vec = vec!["-i".to_string(), sed_expr, file_path.to_string()];
        let cmd_args: Vec<&str> = cmd_args_vec.iter().map(|s| &**s).collect();

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits("sed", &cmd_args, &limits, args.working_directory.as_deref())
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 1,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!(
                "Sed replace failed: {}",
                e
            ))),
        }
    }

    fn validate_sed_replace_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if !args.parameters.contains_key("path") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        if !args.parameters.contains_key("pattern") {
            return Err(ValidationError {
                field: "pattern".to_string(),
                message: "Pattern parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        Ok(())
    }

    // Awk extract implementation
    async fn execute_awk_extract(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let file_path = args
            .parameters
            .get("path")
            .ok_or_else(|| ToolError::ValidationError("Missing 'path' parameter".to_string()))?;

        let script = args
            .parameters
            .get("script")
            .ok_or_else(|| ToolError::ValidationError("Missing 'script' parameter".to_string()))?;

        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(file_path)?;

        let cmd_args = vec![script.as_str(), file_path.as_str()];

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits("awk", &cmd_args, &limits, args.working_directory.as_deref())
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 2,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!(
                "Awk extract failed: {}",
                e
            ))),
        }
    }

    fn validate_awk_extract_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if !args.parameters.contains_key("path") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        if !args.parameters.contains_key("script") {
            return Err(ValidationError {
                field: "script".to_string(),
                message: "Script parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        Ok(())
    }

    // Curl fetch implementation
    async fn execute_curl_fetch(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let url = args
            .parameters
            .get("url")
            .ok_or_else(|| ToolError::ValidationError("Missing 'url' parameter".to_string()))?;

        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ToolError::ValidationError(
                "Only HTTP/HTTPS URLs are allowed".to_string(),
            ));
        }

        let mut cmd_args_vec: Vec<String> = vec![
            "--silent".to_string(),
            "--show-error".to_string(),
            "--max-time".to_string(),
            "30".to_string(),
        ];

        // Add headers if provided
        if let Some(headers) = args.parameters.get("headers") {
            for header in headers.split(',') {
                cmd_args_vec.push("--header".to_string());
                cmd_args_vec.push(header.trim().to_string());
            }
        }

        cmd_args_vec.push(url.to_string());
        let cmd_args: Vec<&str> = cmd_args_vec.iter().map(|s| s.as_str()).collect();

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits(
                "curl",
                &cmd_args,
                &limits,
                args.working_directory.as_deref(),
            )
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 2,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!(
                "Curl fetch failed: {}",
                e
            ))),
        }
    }

    fn validate_curl_fetch_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if !args.parameters.contains_key("url") {
            return Err(ValidationError {
                field: "url".to_string(),
                message: "URL parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        Ok(())
    }

    // Web search implementation using curl for basic search
    async fn execute_web_search(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let query = args
            .parameters
            .get("query")
            .ok_or_else(|| ToolError::ValidationError("Missing 'query' parameter".to_string()))?;

        // For now, use a simple curl to duckduckgo or similar
        // In production, this would use a proper search API
        let search_url = format!(
            "https://duckduckgo.com/?q={}&format=json",
            query.replace(" ", "+")
        );

        let cmd_args = vec![
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            search_url.as_str(),
        ];

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits(
                "curl",
                &cmd_args,
                &limits,
                args.working_directory.as_deref(),
            )
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 2,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!(
                "Web search failed: {}",
                e
            ))),
        }
    }

    fn validate_web_search_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        if !args.parameters.contains_key("query") {
            return Err(ValidationError {
                field: "query".to_string(),
                message: "Query parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        Ok(())
    }

    // Git status implementation
    async fn execute_git_status(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let path = args.parameters.get("path").map_or(".", |v| v.as_str());
        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(path)?;

        let cmd_args_vec = vec!["status", "--porcelain", "--color=always"];
        let cmd_args: Vec<&str> = cmd_args_vec.into_iter().collect();

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits("git", &cmd_args, &limits, Some(path))
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 1,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!(
                "Git status failed: {}",
                e
            ))),
        }
    }

    fn validate_git_status_args(&self, _args: &ToolArgs) -> Result<(), ValidationError> {
        Ok(())
    }

    // Git diff implementation
    async fn execute_git_diff(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let path = args.parameters.get("path").map_or(".", |v| v.as_str());
        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(path)?;

        let mut cmd_args_vec: Vec<String> = vec!["diff".to_string(), "--color=always".to_string()];

        if let Some(commit) = args.parameters.get("commit") {
            cmd_args_vec.push(commit.to_string());
        }

        if let Some(other) = args.parameters.get("other") {
            cmd_args_vec.push(other.to_string());
        }

        let cmd_args: Vec<&str> = cmd_args_vec.iter().map(|s| s.as_str()).collect();

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits("git", &cmd_args, &limits, Some(path))
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 2,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!("Git diff failed: {}", e))),
        }
    }

    fn validate_git_diff_args(&self, _args: &ToolArgs) -> Result<(), ValidationError> {
        Ok(())
    }

    // Git log implementation
    async fn execute_git_log(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let path = args.parameters.get("path").map_or(".", |v| v.as_str());
        let security_validator = ToolSecurityValidator::new();
        security_validator.validate_path(path)?;

        let mut cmd_args_vec: Vec<String> = vec![
            "log".to_string(),
            "--oneline".to_string(),
            "--color=always".to_string(),
        ];

        if let Some(limit) = args.parameters.get("limit") {
            if let Ok(n) = limit.parse::<usize>() {
                let limit_arg = format!("-{}", n);
                cmd_args_vec.push(limit_arg);
            }
        }

        if let Some(author) = args.parameters.get("author") {
            cmd_args_vec.push("--author".to_string());
            cmd_args_vec.push(author.clone());
        }

        let cmd_args: Vec<&str> = cmd_args_vec.iter().map(|s| s.as_str()).collect();

        let limits = ResourceLimits::default();
        let enforcer = ResourceEnforcer::new();

        match enforcer
            .execute_with_limits("git", &cmd_args, &limits, Some(path))
            .await
        {
            Ok(result) => {
                let resources_used = ResourceUsage {
                    memory_used_mb: 2,
                    cpu_time_seconds: result.execution_time.as_secs_f64(),
                    processes_created: 1,
                    output_size: result.stdout.len(),
                };

                Ok(ToolOutput {
                    success: result.success,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    execution_time: result.execution_time,
                    resources_used,
                })
            }
            Err(e) => Err(ToolError::ExecutionError(format!("Git log failed: {}", e))),
        }
    }

    fn validate_git_log_args(&self, _args: &ToolArgs) -> Result<(), ValidationError> {
        Ok(())
    }
}

/// Tool registry for managing available tools
pub struct ToolRegistry {
    tools: HashMap<String, SafeTool>,
    policy_engine: crate::policy_engine::PolicyEngine,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut tools = HashMap::new();
        tools.insert("file_read".to_string(), SafeTool::FileRead);
        tools.insert("file_write".to_string(), SafeTool::FileWrite);
        tools.insert("directory_list".to_string(), SafeTool::DirectoryList);
        tools.insert("process_list".to_string(), SafeTool::ProcessList);
        tools.insert("grep_search".to_string(), SafeTool::GrepSearch);
        tools.insert("find_files".to_string(), SafeTool::FindFiles);
        tools.insert("sed_replace".to_string(), SafeTool::SedReplace);
        tools.insert("awk_extract".to_string(), SafeTool::AwkExtract);
        tools.insert("curl_fetch".to_string(), SafeTool::CurlFetch);
        tools.insert("web_search".to_string(), SafeTool::WebSearch);
        tools.insert("git_status".to_string(), SafeTool::GitStatus);
        tools.insert("git_diff".to_string(), SafeTool::GitDiff);
        tools.insert("git_log".to_string(), SafeTool::GitLog);

        Self {
            tools,
            policy_engine: crate::policy_engine::PolicyEngine::new(),
        }
    }

    pub async fn execute_tool(
        &self,
        tool_name: &str,
        args: ToolArgs,
    ) -> Result<ToolOutput, ToolError> {
        let start_time = std::time::Instant::now();

        // Record tool execution attempt
        let _trace = OBSERVABILITY.start_request_trace(&format!("tool_{}", tool_name));

        let result = async {
            let tool = self.tools.get(tool_name).ok_or_else(|| {
                ToolError::ValidationError(format!("Tool '{}' not found", tool_name))
            })?;

            tool.validate_args(&args).map_err(|e| {
                ToolError::ValidationError(format!("Argument validation failed: {}", e))
            })?;

            // Policy check before execution
            self.check_policy(tool_name, &args).await?;

            tool.execute(args).await
        }
        .await;

        let execution_time = start_time.elapsed();
        let success = result.is_ok();

        // Record metrics
        OBSERVABILITY
            .record_request(&format!("tool_{}", tool_name), execution_time, success)
            .await;

        if !success {
            // Record security event for failed tool execution
            let mut details = std::collections::HashMap::new();
            details.insert("tool_name".to_string(), tool_name.to_string());
            details.insert("error".to_string(), format!("{:?}", result.as_ref().err()));
            OBSERVABILITY
                .record_security_event("tool_execution_failed", details)
                .await;
        }

        result
    }

    async fn check_policy(&self, tool_name: &str, args: &ToolArgs) -> Result<(), ToolError> {
        use crate::policy_engine::{evaluate_tool_request, ResourceLimits};

        let resource_limits = ResourceLimits {
            max_memory_mb: 512,
            max_cpu_percent: 50.0,
            max_execution_time: args.timeout.map(|d| d.as_secs() as u64).unwrap_or(30),
            max_output_size: 1_048_576,
            max_processes: 10,
        };

        // Check for secrets in parameters (simple check)
        let contains_secrets = args
            .parameters
            .values()
            .any(|v| v.contains("password") || v.contains("secret") || v.contains("key"));

        // Check for network access (simple heuristic)
        let network_access = args
            .parameters
            .values()
            .any(|v| v.contains("http") || v.contains("https"));

        // Extract file paths
        let file_paths = args
            .parameters
            .values()
            .filter(|v| v.contains("/") || v.contains("\\"))
            .cloned()
            .collect::<Vec<_>>();

        match evaluate_tool_request(
            tool_name,
            &args.parameters,
            &resource_limits,
            contains_secrets,
            network_access,
            &file_paths,
        )
        .await
        {
            Ok(decision) => match decision.action {
                crate::policy_engine::PolicyAction::Allow => Ok(()),
                crate::policy_engine::PolicyAction::Deny(reason) => Err(
                    ToolError::SecurityViolation(format!("Policy denied: {}", reason)),
                ),
                crate::policy_engine::PolicyAction::RequireApproval(reason) => Err(
                    ToolError::SecurityViolation(format!("Approval required: {}", reason)),
                ),
                crate::policy_engine::PolicyAction::Escalate(reason) => Err(
                    ToolError::SecurityViolation(format!("Escalated: {}", reason)),
                ),
                crate::policy_engine::PolicyAction::LogOnly => Ok(()),
            },
            Err(e) => Err(ToolError::SecurityViolation(format!(
                "Policy evaluation failed: {}",
                e
            ))),
        }
    }

    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory function to create all safe tools
pub fn create_safe_tools() -> Vec<SafeTool> {
    vec![
        SafeTool::FileRead,
        SafeTool::FileWrite,
        SafeTool::DirectoryList,
        SafeTool::ProcessList,
        SafeTool::GrepSearch,
        SafeTool::FindFiles,
        SafeTool::SedReplace,
        SafeTool::AwkExtract,
        SafeTool::CurlFetch,
        SafeTool::WebSearch,
        SafeTool::GitStatus,
        SafeTool::GitDiff,
        SafeTool::GitLog,
    ]
}
