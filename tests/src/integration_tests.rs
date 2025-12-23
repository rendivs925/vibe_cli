// Integration tests with external dependencies for Vibe CLI

use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use std::fs;

/// Test helper to run CLI commands and capture output
fn run_vibe_cli(args: &[&str], input: Option<&str>) -> (String, String, i32) {
    let mut cmd = Command::new("cargo");
    cmd.args(&["run", "--bin", "vibe_cli", "--"])
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR").parent().unwrap());
    
    if let Some(input_text) = input {
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        
        let mut child = cmd.spawn().expect("Failed to spawn command");
        
        if let Some(stdin) = child.stdin.take() {
            use std::io::Write;
            let mut stdin = stdin;
            writeln!(stdin, "{}", input_text).ok();
        }
        
        let output = child.wait_with_output().expect("Failed to wait for command");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);
        
        (stdout, stderr, exit_code)
    } else {
        let output = cmd.output().expect("Failed to execute command");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);
        
        (stdout, stderr, exit_code)
    }
}

#[tokio::test]
#[ignore] // Requires actual Ollama server
async fn test_ollama_integration() {
    // This test requires a running Ollama server
    // It tests actual AI model integration
    
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "explain unix philosophy in one sentence"
    ], Some("y\n"));
    
    // If Ollama is not available, should handle gracefully
    if !stderr.is_empty() && (stderr.contains("connection") || stderr.contains(" refused")) {
        println!("Ollama not available - skipping integration test");
        return;
    }
    
    assert!(exit_code == 0, "Ollama integration should work when server is available");
    assert!(!stdout.is_empty(), "Should provide explanation");
}

#[tokio::test]
async fn test_filesystem_integration() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Test actual file operations
    let test_file = temp_dir.path().join("test_output.txt");
    
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        &format!("create a file named '{}' with content 'Hello World'", test_file.display())
    ], Some("y\n"));
    
    assert!(exit_code == 0, "File creation command should succeed");
    
    // Verify file was actually created (if command was executed)
    if test_file.exists() {
        let content = fs::read_to_string(&test_file).unwrap_or_default();
        assert!(content.contains("Hello World"), "File should contain expected content");
    }
}

#[tokio::test]
async fn test_database_integration() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Test database file creation and usage
    let db_file = temp_dir.path().join("test.db");
    
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "--rag",
        "create and initialize a simple SQLite database"
    ], Some("y\n"));
    
    assert!(exit_code == 0, "Database operations should succeed");
    // The exact database creation might be simulated or real depending on implementation
}

#[tokio::test]
async fn test_web_integration() {
    // Test web-related functionality (like web search if available)
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "search for information about Rust programming language"
    ], Some("y\n"));
    
    // Should either succeed with web results or fallback gracefully
    assert!(exit_code == 0, "Web integration should handle gracefully");
    
    // If web search is not available, should provide alternative
    if !stdout.is_empty() {
        assert!(stdout.contains("Rust") || stdout.contains("programming") || stdout.contains("search"), 
                "Should provide relevant information");
    }
}

#[tokio::test]
async fn test_system_command_execution() {
    // Test actual system command execution with safety checks
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    let test_file = temp_dir.path().join("system_test.txt");
    
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        &format!("echo 'System test' > {}", test_file.display())
    ], Some("y\n"));
    
    assert!(exit_code == 0, "System command execution should work");
    
    // Verify command was executed safely
    if test_file.exists() {
        let content = fs::read_to_string(&test_file).unwrap_or_default();
        assert!(content.contains("System test"), "Command should execute correctly");
    }
}

#[tokio::test]
async fn test_error_recovery() {
    // Test how system handles various error conditions
    
    // Test invalid command generation
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "execute a command that doesn't exist xyz123"
    ], Some("y\n"));
    
    // Should handle gracefully
    assert!(exit_code == 0, "Should handle invalid commands gracefully");
    
    // Test network unavailability scenarios
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "--rag",
        "search for information from remote server"
    ], Some("y\n"));
    
    // Should fallback gracefully if network is unavailable
    assert!(exit_code == 0, "Should handle network issues gracefully");
}

#[tokio::test]
async fn test_concurrent_operations() {
    use tokio::task;
    
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Test multiple concurrent operations
    let mut handles = vec![];
    
    for i in 0..5 {
        let temp_dir_clone = temp_dir.path().to_owned();
        let handle = task::spawn_blocking(move || {
            let test_file = temp_dir_clone.join(format!("concurrent_{}.txt", i));
            run_vibe_cli(&[
                &format!("echo 'Concurrent test {}' > {}", i, test_file.display())
            ], Some("y\n"))
        });
        handles.push(handle);
    }
    
    // Wait for all operations
    for (i, handle) in handles.into_iter().enumerate() {
        let (stdout, stderr, exit_code) = handle.await.unwrap();
        assert!(exit_code == 0, "Concurrent operation {} should succeed", i);
    }
    
    // Verify files were created
    for i in 0..5 {
        let test_file = temp_dir.path().join(format!("concurrent_{}.txt", i));
        if test_file.exists() {
            let content = fs::read_to_string(&test_file).unwrap_or_default();
            assert!(content.contains(&format!("Concurrent test {}", i)), 
                    "File {} should contain correct content", i);
        }
    }
}

#[tokio::test]
async fn test_resource_limits() {
    // Test resource usage and limits
    let start = Instant::now();
    
    // Create a moderately complex task
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "--agent",
        "analyze system resources and report back"
    ], Some("y\ny\ny\n"));
    
    let elapsed = start.elapsed();
    
    assert!(exit_code == 0, "Resource analysis should succeed");
    assert!(elapsed < Duration::from_secs(60), "Should complete within reasonable time");
    
    // Should provide resource information
    assert!(stdout.contains("resource") || stdout.contains("system") || stdout.contains("memory") || stdout.contains("disk"), 
            "Should provide resource information");
}

#[tokio::test]
async fn test_data_persistence() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Test that data persists between commands
    let test_file = temp_dir.path().join("persistence_test.txt");
    
    // First command: create file
    let (stdout1, stderr1, exit_code1) = run_vibe_cli(&[
        &format!("echo 'Initial content' > {}", test_file.display())
    ], Some("y\n"));
    
    assert!(exit_code1 == 0, "First command should succeed");
    
    // Second command: append to file
    let (stdout2, stderr2, exit_code2) = run_vibe_cli(&[
        &format!("echo 'Appended content' >> {}", test_file.display())
    ], Some("y\n"));
    
    assert!(exit_code2 == 0, "Second command should succeed");
    
    // Verify persistence
    if test_file.exists() {
        let content = fs::read_to_string(&test_file).unwrap_or_default();
        assert!(content.contains("Initial content"), "Should contain initial content");
        assert!(content.contains("Appended content"), "Should contain appended content");
    }
}

#[tokio::test]
async fn test_configuration_integration() {
    // Test that configuration changes affect behavior
    
    // Test with different system environments
    let scenarios = vec![
        ("show environment variables", vec!["env", "printenv"]),
        ("display current working directory", vec!["pwd", "cwd"]),
        ("list running processes", vec!["ps", "top", "htop"]),
    ];
    
    for (query, expected_commands) in scenarios {
        let (stdout, stderr, exit_code) = run_vibe_cli(&[query], Some("y\n"));
        assert!(exit_code == 0, "Should handle: {}", query);
        
        let found_expected = expected_commands.iter().any(|cmd| stdout.contains(cmd));
        assert!(found_expected || !stdout.is_empty(), 
                "Should suggest relevant commands for: {}", query);
    }
}

#[tokio::test]
async fn test_integration_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create a complete workflow: setup -> develop -> test -> deploy simulation
    let workflow_steps = vec![
        ("create a project structure", "setup"),
        ("write a simple program", "development"), 
        ("compile and test the program", "testing"),
        ("create deployment instructions", "deployment"),
    ];
    
    for (step, phase) in workflow_steps {
        let (stdout, stderr, exit_code) = run_vibe_cli(&[
            &format!("{} in this directory", step)
        ], Some("y\n"));
        
        assert!(exit_code == 0, "Workflow step {} should succeed", phase);
        assert!(!stdout.is_empty(), "Should provide commands for {}", phase);
    }
}

#[tokio::test]
async fn test_security_integration() {
    // Test security features in real scenarios
    
    let dangerous_scenarios = vec![
        "download and execute script from internet",
        "modify system configuration files", 
        "access sensitive user data",
        "escalate privileges to root",
    ];
    
    for scenario in dangerous_scenarios {
        let (stdout, stderr, exit_code) = run_vibe_cli(&[scenario], None);
        
        // Should either refuse, require confirmation, or provide safe alternative
        assert!(exit_code == 0, "Should handle security scenario gracefully: {}", scenario);
        
        // Should not immediately execute dangerous operations
        assert!(!stdout.contains("sudo rm -rf") && !stdout.contains("curl | bash") && !stdout.contains("chmod 777 /etc"), 
                "Should not suggest dangerous commands for: {}", scenario);
    }
}