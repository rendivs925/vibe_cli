use crate::tools::{SafeTool, ToolArgs, ToolOutput, ToolError, ResourceLimits, ResourceUsage, ValidationError, ValidationSeverity, ToolSecurityValidator};
use anyhow::{anyhow, Result};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::fs as tokio_fs;
use tokio::process::Command as TokioCommand;

/// Safe file reading tool with path validation
pub struct FileReadTool {
    security_validator: ToolSecurityValidator,
    resource_limits: ResourceLimits,
}

impl FileReadTool {
    pub fn new() -> Self {
        Self {
            security_validator: ToolSecurityValidator::new(),
            resource_limits: ResourceLimits::default(),
        }
    }
}


    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Safely read file contents with path validation and size limits"
    }

    async fn execute(&self, args: ToolArgs) -> Result<ToolOutput, ToolError> {
        let start_time = Instant::now();
        
        // Get file path from args
        let file_path = args.parameters.get("path")
            .ok_or_else(|| ToolError::ValidationError("Missing 'path' parameter".to_string()))?;

        // Validate path security
        self.security_validator.validate_path(file_path)
            .map_err(|e| ToolError::SecurityViolation(format!("Path validation failed: {:?}", e)))?;

        // Check if file exists and get metadata
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(ToolError::ExecutionError(format!("File not found: {}", file_path)));
        }

        let metadata = fs::metadata(path)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read file metadata: {}", e)))?;

        // Validate file size
        self.security_validator.validate_file_size(metadata.len())
            .map_err(|e| ToolError::SecurityViolation(format!("File size validation failed: {:?}", e)))?;

        // Read file content
        let content = fs::read_to_string(path)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read file: {}", e)))?;

        // Check output size limit
        if content.len() > self.resource_limits.max_output_size {
            return Err(ToolError::ResourceLimitExceeded(format!(
                "File content exceeds output size limit: {} > {} bytes",
                content.len(),
                self.resource_limits.max_output_size
            )));
        }

        let execution_time = start_time.elapsed();
        let resources_used = ResourceUsage {
            memory_used_mb: 0,
            cpu_time_seconds: 0.0,
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

    fn validate_args(&self, args: &ToolArgs) -> Result<(), ValidationError> {
        // Check required parameters
        if !args.parameters.contains_key("path") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path parameter is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        let path = args.parameters.get("path").unwrap();
        
        // Validate path format
        if path.is_empty() {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path cannot be empty".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        // Check for dangerous path patterns
        if path.contains("..") {
            return Err(ValidationError {
                field: "path".to_string(),
                message: "Path traversal not allowed".to_string(),
                severity: ValidationSeverity::Critical,
            });
        }

        Ok(())
    }

impl FileWriteTool {
    pub fn new() -> Self {
        Self {
            security_validator: ToolSecurityValidator::new(),
            resource_limits: ResourceLimits::default(),
        }
    }
}



/// Safe directory listing tool
pub struct DirectoryListTool {
    security_validator: ToolSecurityValidator,
    resource_limits: ResourceLimits,
}

impl DirectoryListTool {
    pub fn new() -> Self {
        Self {
            security_validator: ToolSecurityValidator::new(),
            resource_limits: ResourceLimits::default(),
        }
    }
}



/// Safe process listing tool
pub struct ProcessListTool {
    resource_limits: ResourceLimits,
}

impl ProcessListTool {
    pub fn new() -> Self {
        Self {
            resource_limits: ResourceLimits::default(),
        }
    }
}



/// Factory function to create all safe tools
pub fn create_safe_tools() -> Vec<SafeTool> {
    vec![
        SafeTool::FileRead(FileReadTool::new()),
        SafeTool::FileWrite(FileWriteTool::new()),
        SafeTool::DirectoryList(DirectoryListTool::new()),
        SafeTool::ProcessList(ProcessListTool::new()),
    ]
}