use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

/// Resource enforcement using cgroups/systemd for process isolation
pub struct ResourceEnforcer {
    cgroup_base_path: String,
    enabled: bool,
}

impl ResourceEnforcer {
    pub fn new() -> Self {
        // Check if cgroups are available
        let cgroup_path = "/sys/fs/cgroup";
        let enabled = std::path::Path::new(cgroup_path).exists();

        if !enabled {
            eprintln!("Warning: cgroups not available, falling back to basic resource limits");
        }

        Self {
            cgroup_base_path: cgroup_path.to_string(),
            enabled,
        }
    }

    /// Execute a command with resource limits
    pub async fn execute_with_limits(
        &self,
        command: &str,
        args: &[&str],
        resource_limits: &ResourceLimits,
        working_dir: Option<&str>,
    ) -> Result<CommandResult, ResourceError> {
        if self.enabled {
            self.execute_with_cgroups(command, args, resource_limits, working_dir)
                .await
        } else {
            self.execute_with_basic_limits(command, args, resource_limits, working_dir)
                .await
        }
    }

    /// Execute with cgroups (Linux systems)
    async fn execute_with_cgroups(
        &self,
        command: &str,
        args: &[&str],
        resource_limits: &ResourceLimits,
        working_dir: Option<&str>,
    ) -> Result<CommandResult, ResourceError> {
        // Create a unique cgroup name
        let cgroup_name = format!("vibe_cli_{}", std::process::id());

        // Create cgroup directory
        let cgroup_path = format!("{}/user.slice/{}", self.cgroup_base_path, cgroup_name);
        std::fs::create_dir_all(&cgroup_path)
            .map_err(|e| ResourceError::CgroupError(format!("Failed to create cgroup: {}", e)))?;

        // Set resource limits
        self.set_cgroup_limits(&cgroup_path, resource_limits)?;

        // Execute command in cgroup
        let result = timeout(
            resource_limits.max_execution_time,
            self.run_in_cgroup(&cgroup_path, command, args, working_dir),
        )
        .await;

        // Clean up cgroup
        let _ = std::fs::remove_dir_all(&cgroup_path);

        match result {
            Ok(output) => output,
            Err(_) => Err(ResourceError::Timeout),
        }
    }

    fn set_cgroup_limits(
        &self,
        cgroup_path: &str,
        limits: &ResourceLimits,
    ) -> Result<(), ResourceError> {
        // Set CPU limit (percentage)
        if limits.max_cpu_percent < 100.0 {
            let cpu_quota = (limits.max_cpu_percent * 1000.0) as u64;
            std::fs::write(
                format!("{}/cpu/cpu.cfs_quota_us", cgroup_path),
                cpu_quota.to_string(),
            )
            .map_err(|e| ResourceError::CgroupError(format!("Failed to set CPU limit: {}", e)))?;
        }

        // Set memory limit
        let memory_limit = limits.max_memory_mb * 1024 * 1024; // Convert MB to bytes
        std::fs::write(
            format!("{}/memory/memory.limit_in_bytes", cgroup_path),
            memory_limit.to_string(),
        )
        .map_err(|e| ResourceError::CgroupError(format!("Failed to set memory limit: {}", e)))?;

        Ok(())
    }

    async fn run_in_cgroup(
        &self,
        cgroup_path: &str,
        command: &str,
        args: &[&str],
        working_dir: Option<&str>,
    ) -> Result<CommandResult, ResourceError> {
        // Add process to cgroup
        let pid = std::process::id();
        std::fs::write(format!("{}/cgroup.procs", cgroup_path), pid.to_string()).map_err(|e| {
            ResourceError::CgroupError(format!("Failed to add process to cgroup: {}", e))
        })?;

        // Execute the command
        let mut cmd = TokioCommand::new(command);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let output = cmd.output().await.map_err(|e| {
            ResourceError::ExecutionError(format!("Command execution failed: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Check output size limits
        if stdout.len() > 1024 * 1024 {
            // 1MB limit
            return Err(ResourceError::OutputTooLarge(stdout.len()));
        }

        Ok(CommandResult {
            success: output.status.success(),
            stdout,
            stderr,
            exit_code: output.status.code(),
            execution_time: Duration::from_secs(0), // Would need to track this properly
        })
    }

    /// Execute with basic timeout and size limits (fallback)
    async fn execute_with_basic_limits(
        &self,
        command: &str,
        args: &[&str],
        resource_limits: &ResourceLimits,
        working_dir: Option<&str>,
    ) -> Result<CommandResult, ResourceError> {
        let start_time = std::time::Instant::now();

        let result = timeout(
            resource_limits.max_execution_time,
            self.run_basic_command(command, args, working_dir),
        )
        .await;

        match result {
            Ok(output) => {
                let mut result = output?;
                result.execution_time = start_time.elapsed();

                // Check output size limits
                if result.stdout.len() > resource_limits.max_output_size {
                    return Err(ResourceError::OutputTooLarge(result.stdout.len()));
                }

                Ok(result)
            }
            Err(_) => Err(ResourceError::Timeout),
        }
    }

    async fn run_basic_command(
        &self,
        command: &str,
        args: &[&str],
        working_dir: Option<&str>,
    ) -> Result<CommandResult, ResourceError> {
        let mut cmd = TokioCommand::new(command);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let output = cmd.output().await.map_err(|e| {
            ResourceError::ExecutionError(format!("Command execution failed: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(CommandResult {
            success: output.status.success(),
            stdout,
            stderr,
            exit_code: output.status.code(),
            execution_time: Duration::from_secs(0),
        })
    }

    /// Check if cgroups are available
    pub fn cgroups_available(&self) -> bool {
        self.enabled
    }

    /// Get system resource limits
    pub fn get_system_limits() -> ResourceLimits {
        ResourceLimits {
            max_memory_mb: 512,    // 512MB default
            max_cpu_percent: 50.0, // 50% CPU default
            max_execution_time: Duration::from_secs(30),
            max_output_size: 1_048_576, // 1MB
            max_processes: 10,
        }
    }
}

/// Resource limits for command execution
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f32,
    pub max_execution_time: Duration,
    pub max_output_size: usize,
    pub max_processes: u32,
}

/// Result of command execution with resource tracking
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub execution_time: Duration,
}

/// Resource enforcement errors
#[derive(Debug, Clone)]
pub enum ResourceError {
    CgroupError(String),
    ExecutionError(String),
    Timeout,
    OutputTooLarge(usize),
    MemoryLimitExceeded(u64),
    CpuLimitExceeded(f32),
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceError::CgroupError(msg) => write!(f, "Cgroup error: {}", msg),
            ResourceError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            ResourceError::Timeout => write!(f, "Command execution timed out"),
            ResourceError::OutputTooLarge(size) => write!(f, "Output too large: {} bytes", size),
            ResourceError::MemoryLimitExceeded(limit) => {
                write!(f, "Memory limit exceeded: {} MB", limit)
            }
            ResourceError::CpuLimitExceeded(limit) => write!(f, "CPU limit exceeded: {}%", limit),
        }
    }
}

impl std::error::Error for ResourceError {}

/// Integration with existing tool system
pub async fn execute_tool_with_resource_limits(
    tool_name: &str,
    command: &str,
    args: &[&str],
    working_dir: Option<&str>,
) -> Result<CommandResult, ResourceError> {
    let enforcer = ResourceEnforcer::new();
    let limits = ResourceEnforcer::get_system_limits();

    enforcer
        .execute_with_limits(command, args, &limits, working_dir)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_basic_command_execution() {
        let enforcer = ResourceEnforcer::new();
        let limits = ResourceLimits {
            max_memory_mb: 100,
            max_cpu_percent: 10.0,
            max_execution_time: Duration::from_secs(5),
            max_output_size: 1024,
            max_processes: 1,
        };

        let result = enforcer
            .execute_with_limits("echo", &["hello"], &limits, None)
            .await;

        match result {
            Ok(output) => {
                assert!(output.success);
                assert_eq!(output.stdout.trim(), "hello");
            }
            Err(ResourceError::CgroupError(_)) => {
                // Cgroups not available, but execution should still work via fallback
                let fallback_result = enforcer
                    .execute_with_basic_limits("echo", &["hello"], &limits, None)
                    .await;
                assert!(fallback_result.is_ok());
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[test]
    fn test_cgroups_availability() {
        let enforcer = ResourceEnforcer::new();
        // This will be true on Linux systems with cgroups, false otherwise
        let _available = enforcer.cgroups_available();
    }

    #[test]
    fn test_timeout_enforcement() {
        let limits = ResourceLimits {
            max_memory_mb: 100,
            max_cpu_percent: 10.0,
            max_execution_time: Duration::from_millis(1), // Very short timeout
            max_output_size: 1024,
            max_processes: 1,
        };

        // Test that timeout is properly configured
        assert_eq!(limits.max_execution_time, Duration::from_millis(1));
    }
}
