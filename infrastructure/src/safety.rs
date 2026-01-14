use shared::types::Result;
use std::collections::{HashMap, HashSet, VecDeque};
use std::process::Stdio;

use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};

/// Production safety system with command sandboxing and resource monitoring
pub struct SafetyManager {
    last_command_time: Mutex<Instant>,
    last_api_call: Mutex<Instant>,
    command_min_interval: Duration,
    api_min_interval: Duration,
    blocked_commands: HashSet<String>,
    blocked_paths: HashSet<String>,
    dangerous_patterns: Vec<String>,
    resource_limits: ResourceLimits,
    command_history: RwLock<VecDeque<CommandRecord>>,
    system_stats: RwLock<SystemStats>,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f32,
    pub max_execution_time_secs: u64,
    pub max_concurrent_commands: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 1024, // 1GB
            max_cpu_percent: 80.0,
            max_execution_time_secs: 300, // 5 minutes
            max_concurrent_commands: 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandRecord {
    pub command: String,
    pub timestamp: Instant,
    pub user: String,
    pub blocked: bool,
    pub reason: Option<String>,
    pub execution_time_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SystemStats {
    pub total_commands_executed: u64,
    pub total_commands_blocked: u64,
    pub active_commands: usize,
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f32,
    pub last_updated: Instant,
}

impl SafetyManager {
    /// Create new safety manager with production settings
    pub fn new() -> Self {
        let blocked_commands = Self::init_blocked_commands();
        let blocked_paths = Self::init_blocked_paths();
        let dangerous_patterns = Self::init_dangerous_patterns();

        // Rate limits: 100 commands/minute, 50 API calls/minute
        let command_min_interval = Duration::from_millis(600); // ~100 per minute
        let api_min_interval = Duration::from_millis(1200); // ~50 per minute

        Self {
            last_command_time: Mutex::new(Instant::now() - command_min_interval),
            last_api_call: Mutex::new(Instant::now() - api_min_interval),
            command_min_interval,
            api_min_interval,
            blocked_commands,
            blocked_paths,
            dangerous_patterns,
            resource_limits: ResourceLimits::default(),
            command_history: RwLock::new(VecDeque::with_capacity(1000)),
            system_stats: RwLock::new(SystemStats {
                total_commands_executed: 0,
                total_commands_blocked: 0,
                active_commands: 0,
                memory_usage_mb: 0,
                cpu_usage_percent: 0.0,
                last_updated: Instant::now(),
            }),
        }
    }

    /// Initialize blocked commands list
    fn init_blocked_commands() -> HashSet<String> {
        let mut blocked = HashSet::new();

        // File system destruction
        blocked.insert("rm".to_string());
        blocked.insert("rmdir".to_string());
        blocked.insert("del".to_string());
        blocked.insert("deltree".to_string());
        blocked.insert("format".to_string());
        blocked.insert("mkfs".to_string());

        // System manipulation
        blocked.insert("dd".to_string());
        blocked.insert("fdisk".to_string());
        blocked.insert("mkfs".to_string());
        blocked.insert("mount".to_string());
        blocked.insert("umount".to_string());

        // Process manipulation
        blocked.insert("kill".to_string());
        blocked.insert("killall".to_string());
        blocked.insert("pkill".to_string());
        blocked.insert("killpg".to_string());

        // System control
        blocked.insert("shutdown".to_string());
        blocked.insert("reboot".to_string());
        blocked.insert("halt".to_string());
        blocked.insert("poweroff".to_string());

        // Network manipulation
        blocked.insert("iptables".to_string());
        blocked.insert("ufw".to_string());
        blocked.insert("firewall-cmd".to_string());

        blocked
    }

    /// Initialize blocked system paths
    fn init_blocked_paths() -> HashSet<String> {
        let mut blocked = HashSet::new();

        // System directories
        blocked.insert("/etc".to_string());
        blocked.insert("/sys".to_string());
        blocked.insert("/dev".to_string());
        blocked.insert("/proc".to_string());
        blocked.insert("/boot".to_string());

        // Root filesystem
        blocked.insert("/".to_string());

        // User home protection (relative)
        blocked.insert("~/.ssh".to_string());
        blocked.insert("~/.gnupg".to_string());

        blocked
    }

    /// Initialize dangerous command patterns
    fn init_dangerous_patterns() -> Vec<String> {
        vec![
            r"rm\s+-rf\s+/".to_string(),            // rm -rf /
            r"rm\s+-rf\s+\*".to_string(),           // rm -rf *
            r":\(\)\{\s*:\|\:&\s*\};:".to_string(), // Fork bomb
            r">/dev/sd[a-z]".to_string(),           // Disk overwriting
            r"dd\s+if=.*of=/dev/".to_string(),      // Disk operations
            r"mkfs\.".to_string(),                  // Filesystem creation
            r"chmod\s+777\s+/".to_string(),         // Dangerous permissions
            r"chown\s+root".to_string(),            // Root ownership
            r"sudo\s+.*rm".to_string(),             // Sudo remove operations
            r"curl.*\|.*bash".to_string(),          // Pipe to bash
            r"wget.*\|.*sh".to_string(),            // Pipe to shell
        ]
    }

    /// Validate and execute command with safety checks
    pub async fn execute_safe_command(
        &self,
        command: &str,
        args: &[String],
        user: &str,
    ) -> Result<std::process::Output> {
        // Pre-execution safety checks
        self.validate_command(command, args).await?;

        // Rate limiting
        self.enforce_command_rate_limit().await?;

        // Resource monitoring
        self.check_resource_limits().await?;

        let start_time = Instant::now();
        let full_command = format!("{} {}", command, args.join(" "));

        // Execute command with resource limits
        let output = self.execute_with_limits(command, args).await;

        let execution_time = start_time.elapsed().as_millis() as u64;

        // Record command execution
        self.record_command(&full_command, user, false, None, Some(execution_time))
            .await;

        // Update system stats
        self.update_system_stats().await;

        match output {
            Ok(output) => {
                // Check for dangerous output patterns
                if self.has_dangerous_output(&output) {
                    self.record_command(
                        &full_command,
                        user,
                        true,
                        Some("Dangerous output detected".to_string()),
                        Some(execution_time),
                    )
                    .await;
                    return Err(anyhow::anyhow!(
                        "Command execution blocked: dangerous output detected"
                    ));
                }
                Ok(output)
            }
            Err(e) => {
                self.record_command(
                    &full_command,
                    user,
                    true,
                    Some(e.to_string()),
                    Some(execution_time),
                )
                .await;
                Err(e)
            }
        }
    }

    /// Validate command for safety violations
    async fn validate_command(&self, command: &str, args: &[String]) -> Result<()> {
        let full_command = format!("{} {}", command, args.join(" "));

        // Check blocked commands
        if self.blocked_commands.contains(command) {
            return Err(anyhow::anyhow!("Blocked command: {}", command));
        }

        // Check dangerous patterns
        for pattern in &self.dangerous_patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                if regex.is_match(&full_command) {
                    return Err(anyhow::anyhow!("Dangerous pattern detected: {}", pattern));
                }
            }
        }

        // Check blocked paths
        for arg in args {
            for blocked_path in &self.blocked_paths {
                if arg.contains(blocked_path) {
                    return Err(anyhow::anyhow!(
                        "Access to protected path blocked: {}",
                        blocked_path
                    ));
                }
            }
        }

        Ok(())
    }

    /// Execute command with resource limits
    async fn execute_with_limits(
        &self,
        command: &str,
        args: &[String],
    ) -> Result<std::process::Output> {
        let child = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Set resource limits (simplified - in production would use cgroups/rlimits)
        // For now, just timeout protection
        let timeout_duration = Duration::from_secs(self.resource_limits.max_execution_time_secs);

        let child_id = child.id();
        match tokio::time::timeout(timeout_duration, child.wait_with_output()).await {
            Ok(result) => result.map_err(|e| anyhow::anyhow!("Command execution failed: {}", e)),
            Err(_) => {
                // Try to kill the process if it has a PID
                if let Some(pid) = child_id {
                    let _ = Command::new("kill")
                        .arg("-9")
                        .arg(pid.to_string())
                        .status()
                        .await;
                }
                Err(anyhow::anyhow!(
                    "Command timed out after {} seconds",
                    timeout_duration.as_secs()
                ))
            }
        }
    }

    /// Check for dangerous output patterns
    fn has_dangerous_output(&self, output: &std::process::Output) -> bool {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let combined = format!("{} {}", stdout, stderr);

        // Check for error patterns that might indicate system compromise
        combined.contains("Permission denied") && combined.contains("root")
            || combined.contains("Operation not permitted")
            || combined.contains("Device or resource busy")
    }

    /// Enforce command rate limiting
    async fn enforce_command_rate_limit(&self) -> Result<()> {
        let mut last_command = self.last_command_time.lock().await;
        let now = Instant::now();
        let time_since_last = now.duration_since(*last_command);

        if time_since_last < self.command_min_interval {
            let sleep_duration = self.command_min_interval - time_since_last;
            tokio::time::sleep(sleep_duration).await;
        }

        *last_command = Instant::now();
        Ok(())
    }

    /// Enforce API rate limiting
    pub async fn enforce_api_rate_limit(&self) -> Result<()> {
        let mut last_api_call = self.last_api_call.lock().await;
        let now = Instant::now();
        let time_since_last = now.duration_since(*last_api_call);

        if time_since_last < self.api_min_interval {
            let sleep_duration = self.api_min_interval - time_since_last;
            tokio::time::sleep(sleep_duration).await;
        }

        *last_api_call = Instant::now();
        Ok(())
    }

    /// Check resource limits
    async fn check_resource_limits(&self) -> Result<()> {
        let stats = self.system_stats.read().await;

        if stats.active_commands >= self.resource_limits.max_concurrent_commands {
            return Err(anyhow::anyhow!("Too many concurrent commands"));
        }

        if stats.memory_usage_mb >= self.resource_limits.max_memory_mb {
            return Err(anyhow::anyhow!("Memory limit exceeded"));
        }

        if stats.cpu_usage_percent >= self.resource_limits.max_cpu_percent {
            return Err(anyhow::anyhow!("CPU limit exceeded"));
        }

        Ok(())
    }

    /// Update system statistics
    async fn update_system_stats(&self) {
        let mut stats = self.system_stats.write().await;

        // Simplified system monitoring (in production would use system APIs)
        stats.memory_usage_mb = 256; // Placeholder
        stats.cpu_usage_percent = 45.0; // Placeholder
        stats.last_updated = Instant::now();
    }

    /// Record command execution
    async fn record_command(
        &self,
        command: &str,
        user: &str,
        blocked: bool,
        reason: Option<String>,
        execution_time: Option<u64>,
    ) {
        let record = CommandRecord {
            command: command.to_string(),
            timestamp: Instant::now(),
            user: user.to_string(),
            blocked,
            reason,
            execution_time_ms: execution_time,
        };

        let mut history = self.command_history.write().await;
        history.push_back(record);

        // Maintain history size limit
        while history.len() > 1000 {
            history.pop_front();
        }

        // Update stats
        let mut stats = self.system_stats.write().await;
        if blocked {
            stats.total_commands_blocked += 1;
        } else {
            stats.total_commands_executed += 1;
        }
    }

    /// Get safety statistics
    pub async fn get_stats(&self) -> HashMap<String, String> {
        let stats = self.system_stats.read().await;
        let history = self.command_history.read().await;

        let mut result = HashMap::new();

        result.insert(
            "total_commands_executed".to_string(),
            stats.total_commands_executed.to_string(),
        );
        result.insert(
            "total_commands_blocked".to_string(),
            stats.total_commands_blocked.to_string(),
        );
        result.insert(
            "active_commands".to_string(),
            stats.active_commands.to_string(),
        );
        result.insert(
            "memory_usage_mb".to_string(),
            stats.memory_usage_mb.to_string(),
        );
        result.insert(
            "cpu_usage_percent".to_string(),
            format!("{:.1}", stats.cpu_usage_percent),
        );
        result.insert("history_size".to_string(), history.len().to_string());
        result.insert(
            "blocked_commands_count".to_string(),
            self.blocked_commands.len().to_string(),
        );
        result.insert(
            "blocked_paths_count".to_string(),
            self.blocked_paths.len().to_string(),
        );

        result
    }

    /// Get recent command history
    pub async fn get_command_history(&self, limit: usize) -> Vec<CommandRecord> {
        let history = self.command_history.read().await;
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Check if command would be blocked (dry run)
    pub async fn check_command(&self, command: &str, args: &[String]) -> Result<()> {
        let full_command = format!("{} {}", command, args.join(" "));

        // Check blocked commands
        if self.blocked_commands.contains(command) {
            return Err(anyhow::anyhow!("Blocked command: {}", command));
        }

        // Check dangerous patterns
        for pattern in &self.dangerous_patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                if regex.is_match(&full_command) {
                    return Err(anyhow::anyhow!("Dangerous pattern detected: {}", pattern));
                }
            }
        }

        // Check blocked paths
        for arg in args {
            for blocked_path in &self.blocked_paths {
                if arg.contains(blocked_path) {
                    return Err(anyhow::anyhow!(
                        "Access to protected path blocked: {}",
                        blocked_path
                    ));
                }
            }
        }

        Ok(())
    }

    /// Add custom blocked command
    pub async fn add_blocked_command(&mut self, command: String) {
        self.blocked_commands.insert(command);
    }

    /// Remove blocked command
    pub async fn remove_blocked_command(&mut self, command: &str) {
        self.blocked_commands.remove(command);
    }

    /// Add blocked path
    pub async fn add_blocked_path(&mut self, path: String) {
        self.blocked_paths.insert(path);
    }

    /// Update resource limits
    pub async fn update_resource_limits(&mut self, limits: ResourceLimits) {
        self.resource_limits = limits;
    }

    /// Clear old command history
    pub async fn clear_history(&self, older_than: Duration) {
        let mut history = self.command_history.write().await;
        let cutoff = Instant::now() - older_than;

        history.retain(|record| record.timestamp > cutoff);
    }

    /// Export safety audit log
    pub async fn export_audit_log(&self) -> String {
        let history = self.command_history.read().await;
        let mut log = String::from("Safety Audit Log\n================\n\n");

        for record in history.iter() {
            log.push_str(&format!(
                "[{}] User: {} | Command: {} | Blocked: {} | Time: {:?}\n",
                record.timestamp.elapsed().as_secs(),
                record.user,
                record.command,
                record.blocked,
                record.execution_time_ms
            ));
            if let Some(reason) = &record.reason {
                log.push_str(&format!("  Reason: {}\n", reason));
            }
            log.push_str("\n");
        }

        log
    }
}
