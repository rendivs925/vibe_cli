use shared::types::Result;
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};
use tokio::time::{timeout, Duration};

/// Sandbox environment for safe command execution
pub struct Sandbox {
    allowed_commands: HashSet<String>,
    blocked_commands: HashSet<String>,
    allowed_paths: HashSet<String>,
    blocked_paths: HashSet<String>,
    dangerous_patterns: Vec<String>,
    max_execution_time: Duration,
    max_output_size: usize,
}

impl Sandbox {
    /// Create a new sandbox with production-safe defaults
    pub fn new() -> Self {
        let mut allowed_commands = HashSet::new();
        let mut blocked_commands = HashSet::new();
        let mut allowed_paths = HashSet::new();
        let mut blocked_paths = HashSet::new();

        // Safe allowed commands
        for cmd in &["ls", "cat", "grep", "find", "head", "tail", "wc", "sort", "uniq", "pwd", "echo"] {
            allowed_commands.insert(cmd.to_string());
        }

        // Programming/development commands
        for cmd in &["cargo", "rustc", "npm", "node", "python", "python3", "pip", "pip3", "git", "make", "cmake"] {
            allowed_commands.insert(cmd.to_string());
        }

        // System monitoring (read-only)
        for cmd in &["ps", "top", "htop", "df", "du", "free", "uptime", "whoami", "id", "date", "systemctl", "journalctl", "hostname", "uname", "lsblk", "blkid", "fdisk", "parted", "lscpu", "lspci", "lsusb", "dmidecode", "sensors", "iostat", "vmstat", "sar", "sysctl"] {
            allowed_commands.insert(cmd.to_string());
        }

        // Blocked dangerous commands
        for cmd in &["rm", "rmdir", "del", "deltree", "format", "mkfs", "dd", "fdisk", "mkfs", "mount", "umount"] {
            blocked_commands.insert(cmd.to_string());
        }

        // System manipulation
        for cmd in &["kill", "killall", "pkill", "killpg", "shutdown", "reboot", "halt", "poweroff"] {
            blocked_commands.insert(cmd.to_string());
        }

        // Network manipulation
        for cmd in &["iptables", "ufw", "firewall-cmd", "wget", "curl"] {
            blocked_commands.insert(cmd.to_string());
        }

        // System paths that are allowed (read-only)
        for path in &["/usr/bin", "/bin", "/usr/local/bin", "/home", "/tmp", "/var/log"] {
            allowed_paths.insert(path.to_string());
        }

        // System paths that are blocked
        for path in &["/etc", "/sys", "/dev", "/proc", "/boot", "/root", "/usr/sbin"] {
            blocked_paths.insert(path.to_string());
        }

        Self {
            allowed_commands,
            blocked_commands,
            allowed_paths,
            blocked_paths,
            dangerous_patterns: Self::get_dangerous_patterns(),
            max_execution_time: Duration::from_secs(30),
            max_output_size: 1024 * 1024, // 1MB
        }
    }

    /// Get dangerous command patterns
    fn get_dangerous_patterns() -> Vec<String> {
        vec![
            r"rm\s+-rf\s+/".to_string(),                    // rm -rf /
            r"rm\s+-rf\s+\*".to_string(),                   // rm -rf *
            r":\(\)\{\s*:\|\:&\s*\};:".to_string(),        // Fork bomb
            r"os\.fork".to_string(),                             // Python fork calls
            r">/dev/sd[a-z]".to_string(),                   // Disk overwriting
            r"dd\s+if=.*of=/dev/".to_string(),              // Disk operations
            r"mkfs\.".to_string(),                          // Filesystem creation
            r"chmod\s+777\s+/".to_string(),                 // Dangerous permissions
            r"chown\s+root".to_string(),                    // Root ownership
            r"sudo\s+.*rm".to_string(),                     // Sudo remove operations
            r".*&&.*".to_string(),                          // Command chaining with &&
            r".*\|\|.*".to_string(),                        // Command chaining with ||
            r".*\|.*bash".to_string(),                      // Pipe to bash
            r".*\|.*sh".to_string(),                        // Pipe to shell
            r"curl.*\|.*bash".to_string(),                  // Pipe to bash
            r"wget.*\|.*sh".to_string(),                    // Pipe to shell
            r"eval\s+.*".to_string(),                       // Eval execution
            r"exec\s+.*".to_string(),                       // Exec execution
            r"source\s+.*".to_string(),                     // Source execution
        ]
    }

    /// Execute command safely in sandbox
    pub async fn execute_safe(&self, command: &str, args: Vec<String>) -> Result<String> {
        // Pre-execution validation
        self.validate_command(command, &args)?;

        // Execute with timeout and output limits
        let command = command.to_string();
        let args = args.to_owned();
        let output = timeout(self.max_execution_time, tokio::task::spawn_blocking(move || {
            let mut cmd = Command::new(&command);
            cmd.args(&args);
            cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped());
            cmd.output()
        })).await???;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Command failed with exit code: {}", output.status));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check output size limits
        if stdout.len() + stderr.len() > self.max_output_size {
            return Err(anyhow::anyhow!("Command output too large"));
        }

        // Check for dangerous output patterns
        let combined_output = format!("{} {}", stdout, stderr);
        if self.has_dangerous_output(&combined_output) {
            return Err(anyhow::anyhow!("Command produced dangerous output"));
        }

        Ok(combined_output)
    }

    /// Validate command for safety
    fn validate_command(&self, command: &str, args: &[String]) -> Result<()> {
        // Check if command is explicitly blocked
        if self.blocked_commands.contains(command) {
            return Err(anyhow::anyhow!("Command '{}' is blocked for security reasons", command));
        }

        // Check if command is allowed (if whitelist is enabled)
        if !self.allowed_commands.is_empty() && !self.allowed_commands.contains(command) {
            return Err(anyhow::anyhow!("Command '{}' is not in the allowed commands list", command));
        }

        // Check arguments for dangerous patterns
        let full_command = format!("{} {}", command, args.join(" "));
        for pattern in &self.dangerous_patterns {
            if regex::Regex::new(pattern).unwrap().is_match(&full_command) {
                return Err(anyhow::anyhow!("Command matches dangerous pattern: {}", pattern));
            }
        }

        // Check paths in arguments
        for arg in args {
            if let Some(path_str) = self.extract_path(arg) {
                if self.is_blocked_path(&path_str) {
                    return Err(anyhow::anyhow!("Access to blocked path: {}", path_str));
                }
            }
        }

        Ok(())
    }

    /// Extract path from argument
    fn extract_path(&self, arg: &str) -> Option<String> {
        // Simple path extraction - looks for / or ./ or ../
        if arg.starts_with('/') || arg.starts_with("./") || arg.starts_with("../") {
            Some(arg.to_string())
        } else if arg.contains('/') {
            Some(arg.to_string())
        } else {
            None
        }
    }

    /// Check if path is blocked
    fn is_blocked_path(&self, path: &str) -> bool {
        let _path_obj = Path::new(path);

        // Check exact matches
        if self.blocked_paths.contains(path) {
            return true;
        }

        // Check parent directories
        for blocked in &self.blocked_paths {
            if path.starts_with(blocked) {
                return true;
            }
        }

        // Check for dangerous patterns
        let dangerous_patterns = ["/etc", "/sys", "/dev", "/proc", "/boot"];
        for pattern in &dangerous_patterns {
            if path.starts_with(pattern) {
                return true;
            }
        }

        false
    }

    /// Check for dangerous output patterns
    fn has_dangerous_output(&self, output: &str) -> bool {
        let dangerous_indicators = [
            "Permission denied",
            "Operation not permitted",
            "Device or resource busy",
            "No such file or directory",
            "Segmentation fault",
            "Bus error",
            "Illegal instruction",
        ];

        for indicator in &dangerous_indicators {
            if output.contains(indicator) {
                return true;
            }
        }

        false
    }

    /// Test command without executing it
    pub fn test_command(&self, command: &str, args: &[String]) -> Result<()> {
        self.validate_command(command, args)
    }

    /// Get allowed commands list
    pub fn get_allowed_commands(&self) -> Vec<String> {
        self.allowed_commands.iter().cloned().collect()
    }

    /// Get blocked commands list
    pub fn get_blocked_commands(&self) -> Vec<String> {
        self.blocked_commands.iter().cloned().collect()
    }

    /// Add custom allowed command
    pub fn allow_command(&mut self, command: String) {
        self.allowed_commands.insert(command);
    }

    /// Block additional command
    pub fn block_command(&mut self, command: String) {
        self.blocked_commands.insert(command);
    }

    /// Add allowed path
    pub fn allow_path(&mut self, path: String) {
        self.allowed_paths.insert(path);
    }

    /// Block additional path
    pub fn block_path(&mut self, path: String) {
        self.blocked_paths.insert(path);
    }

    /// Configure sandbox settings
    pub fn configure(&mut self, max_time: Duration, max_output: usize) {
        self.max_execution_time = max_time;
        self.max_output_size = max_output;
    }

    /// Get sandbox statistics
    pub fn get_stats(&self) -> std::collections::HashMap<String, String> {
        let mut stats = std::collections::HashMap::new();

        stats.insert("allowed_commands".to_string(), self.allowed_commands.len().to_string());
        stats.insert("blocked_commands".to_string(), self.blocked_commands.len().to_string());
        stats.insert("allowed_paths".to_string(), self.allowed_paths.len().to_string());
        stats.insert("blocked_paths".to_string(), self.blocked_paths.len().to_string());
        stats.insert("dangerous_patterns".to_string(), self.dangerous_patterns.len().to_string());
        stats.insert("max_execution_time_secs".to_string(), self.max_execution_time.as_secs().to_string());
        stats.insert("max_output_size_kb".to_string(), (self.max_output_size / 1024).to_string());

        stats
    }
}

/// Confirmation system for destructive operations
pub struct ConfirmationManager {
    dangerous_operations: HashSet<String>,
    require_confirmation: bool,
}

impl ConfirmationManager {
    /// Create new confirmation manager
    pub fn new() -> Self {
        let mut dangerous_operations = HashSet::new();

        // Operations that require confirmation
        for op in &[
            "delete", "remove", "rm", "uninstall", "drop", "destroy",
            "format", "wipe", "clean", "purge", "truncate",
            "overwrite", "replace", "modify", "edit", "update"
        ] {
            dangerous_operations.insert(op.to_string());
        }

        Self {
            dangerous_operations,
            require_confirmation: true,
        }
    }

    /// Check if operation requires confirmation
    pub fn requires_confirmation(&self, operation: &str, target: &str) -> bool {
        if !self.require_confirmation {
            return false;
        }

        let operation_lower = operation.to_lowercase();
        let target_lower = target.to_lowercase();

        // Check operation keywords
        if self.dangerous_operations.iter().any(|op| operation_lower.contains(op)) {
            return true;
        }

        // Check for system files
        if target_lower.contains("/etc/") ||
           target_lower.contains("/sys/") ||
           target_lower.contains("/dev/") ||
           target_lower.contains("/proc/") {
            return true;
        }

        // Check for important file extensions
        let important_extensions = [".db", ".sql", ".key", ".pem", ".crt", ".conf", ".config"];
        if important_extensions.iter().any(|ext| target_lower.ends_with(ext)) {
            return true;
        }

        false
    }

    /// Get confirmation prompt
    pub fn get_confirmation_prompt(&self, operation: &str, target: &str) -> String {
        format!(
            "⚠️  WARNING: This operation may be destructive!\n\n\
            Operation: {}\n\
            Target: {}\n\n\
            Are you sure you want to proceed? (type 'yes' to confirm): ",
            operation, target
        )
    }

    /// Validate confirmation response
    pub fn validate_confirmation(&self, response: &str) -> bool {
        response.trim().to_lowercase() == "yes"
    }

    /// Toggle confirmation requirement
    pub fn set_require_confirmation(&mut self, require: bool) {
        self.require_confirmation = require;
    }
}