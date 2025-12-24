use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::process::Command as TokioCommand;

/// Enum representing all available safe tools
#[derive(Debug, Clone)]
pub enum SafeTool {
    FileRead(crate::safe_tools::FileReadTool),
    FileWrite(crate::safe_tools::FileWriteTool),
    DirectoryList(crate::safe_tools::DirectoryListTool),
    ProcessList(crate::safe_tools::ProcessListTool),
}

impl SafeTool {
    pub fn name(&self) -> &str {
        match self {
            SafeTool::FileRead(_) => "file_read",
            SafeTool::FileWrite(_) => "file_write",
            SafeTool::DirectoryList(_) => "directory_list",
            SafeTool::ProcessList(_) => "process_list",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            SafeTool::FileRead(_) => "Safely read file contents with path validation and size limits",
            SafeTool::FileWrite(_) => "Safely write file contents with backup and rollback capabilities",
            SafeTool::DirectoryList(_) => "Safely list directory contents with path validation",
            SafeTool::ProcessList(_) => "Safely list running processes with filtering",
        }
    }

    pub async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        match self {
            SafeTool::FileRead(tool) => tool.execute(args).await,
            SafeTool::FileWrite(tool) => tool.execute(args).await,
            SafeTool::DirectoryList(tool) => tool.execute(args).await,
            SafeTool::ProcessList(tool) => tool.execute(args).await,
        }
    }

    pub fn validate_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        match self {
            SafeTool::FileRead(tool) => tool.validate_args(args),
            SafeTool::FileWrite(tool) => tool.validate_args(args),
            SafeTool::DirectoryList(tool) => tool.validate_args(args),
            SafeTool::ProcessList(tool) => tool.validate_args(args),
        }
    }

    pub fn get_resource_limits(&self) -> ResourceLimits {
        match self {
            SafeTool::FileRead(tool) => tool.get_resource_limits(),
            SafeTool::FileWrite(tool) => tool.get_resource_limits(),
            SafeTool::DirectoryList(tool) => tool.get_resource_limits(),
            SafeTool::ProcessList(tool) => tool.get_resource_limits(),
        }
    }
}

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
pub enum SecurityViolation {
    BlockedPath(String),
    UnauthorizedPath(String),
    FileSizeExceeded(u64, u64),
    MaliciousCommand(String),
    ResourceLimitExceeded(String),
}

impl std::fmt::Display for SecurityViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityViolation::BlockedPath(path) => write!(f, "Access blocked for path: {}", path),
            SecurityViolation::UnauthorizedPath(path) => write!(f, "Unauthorized path access: {}", path),
            SecurityViolation::FileSizeExceeded(size, limit) => write!(f, "File size {} exceeds limit {}", size, limit),
            SecurityViolation::MaliciousCommand(cmd) => write!(f, "Malicious command detected: {}", cmd),
            SecurityViolation::ResourceLimitExceeded(msg) => write!(f, "Resource limit exceeded: {}", msg),
        }
    }
}

impl std::error::Error for SecurityViolation {}

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
        write!(f, "Validation error in field '{}': {} (severity: {:?})", self.field, self.message, self.severity)
    }
}

impl std::error::Error for ValidationError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Warning,
    Error,
    Critical,
}

/// Resource limits for tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f32,
    pub max_execution_time: Duration,
    pub max_output_size: usize,
    pub max_processes: u32,
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

/// Resource usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_used_mb: u64,
    pub cpu_time_seconds: f64,
    pub processes_created: u32,
    pub output_size: usize,
}

/// Tool registry for managing available tools
pub struct ToolRegistry {
    tools: HashMap<String, SafeTool>,
    audit_logger: ToolAuditLogger,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            audit_logger: ToolAuditLogger::new(),
        }
    }

    pub fn register_tool(&mut self, tool: SafeTool) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn get_tool(&self, name: &str) -> Option<&SafeTool> {
        self.tools.get(name)
    }

    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub async fn execute_tool(&mut self, tool_name: &str, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        // Log execution attempt
        self.audit_logger.log_execution_attempt(tool_name, &args);

        // Get tool
        let tool = self.tools.get(tool_name)
            .ok_or_else(|| ToolError::ValidationError(format!("Tool '{}' not found", tool_name)))?;

        // Validate arguments
        tool.validate_args(&args)
            .map_err(|e| ToolError::ValidationError(format!("Argument validation failed: {}", e)))?;

        // Execute with resource limits
        let start_time = std::time::Instant::now();
        let result = tool.execute(args).await;
        let execution_time = start_time.elapsed();

        // Log execution result
        match &result {
            Ok(output) => self.audit_logger.log_execution_success(tool_name, output),
            Err(error) => self.audit_logger.log_execution_failure(tool_name, error),
        }

        result
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Audit logging for tool executions
pub struct ToolAuditLogger {
    execution_log: Vec<ToolAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAuditEntry {
    pub timestamp: std::time::SystemTime,
    pub tool_name: String,
    pub args: ToolArgs,
    pub result: Option<Result<ToolOutput, ToolError>>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
}

impl ToolAuditLogger {
    pub fn new() -> Self {
        Self {
            execution_log: Vec::new(),
        }
    }

    pub fn log_execution_attempt(&mut self, tool_name: &str, args: &ToolArgs) {
        let entry = ToolAuditEntry {
            timestamp: std::time::SystemTime::now(),
            tool_name: tool_name.to_string(),
            args: args.clone(),
            result: None,
            user_id: None,
            session_id: None,
        };
        self.execution_log.push(entry);
    }

    pub fn log_execution_success(&mut self, tool_name: &str, output: &ToolOutput) {
        if let Some(entry) = self.execution_log.last_mut() {
            if entry.tool_name == tool_name && entry.result.is_none() {
                entry.result = Some(Ok(output.clone()));
            }
        }
    }

    pub fn log_execution_failure(&mut self, tool_name: &str, error: &ToolError) {
        if let Some(entry) = self.execution_log.last_mut() {
            if entry.tool_name == tool_name && entry.result.is_none() {
                entry.result = Some(Err(error.clone()));
            }
        }
    }

    pub fn get_audit_trail(&self) -> &[ToolAuditEntry] {
        &self.execution_log
    }

    pub fn clear_audit_trail(&mut self) {
        self.execution_log.clear();
    }
}

impl Default for ToolAuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// Security validator for tool operations
pub struct ToolSecurityValidator {
    allowed_paths: Vec<String>,
    blocked_patterns: Vec<String>,
    max_file_size: u64,
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

    pub fn validate_path(&self, path: &str) -> Result<(), SecurityViolation> {
        // Check blocked patterns
        for pattern in &self.blocked_patterns {
            if path.starts_with(pattern) {
                return Err(SecurityViolation::BlockedPath(path.to_string()));
            }
        }

        // Check allowed paths
        let allowed = self.allowed_paths.iter().any(|allowed_path| {
            path.starts_with(allowed_path)
        });

        if !allowed {
            return Err(SecurityViolation::UnauthorizedPath(path.to_string()));
        }

        Ok(())
    }

    pub fn validate_file_size(&self, size: u64) -> Result<(), SecurityViolation> {
        if size > self.max_file_size {
            return Err(SecurityViolation::FileSizeExceeded(size, self.max_file_size));
        }
        Ok(())
    }
}



impl Default for ToolSecurityValidator {
    fn default() -> Self {
        Self::new()
    }
}