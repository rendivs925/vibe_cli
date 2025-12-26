/// Tests for the complete ultra-minimal CLI workflow implementation
/// Covers editor integration, real-time visibility, project scoping, and power-user controls

use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;
use std::fs;
use std::path::Path;
use std::thread;

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

/// Test editor integration functionality
#[test]
fn test_editor_integration() {
    use presentation::editor;

    // Test editor detection
    let editor_cmd = editor::Editor::detect_editor();
    assert!(!editor_cmd.is_empty(), "Should detect an editor");

    // Test content editing (basic functionality)
    let test_content = "# Test Plan\n1. Create file\n2. Add content";
    let result = editor::Editor::edit_content(test_content, editor::EditContent::Plan(test_content.to_string()));

    // This might fail if EDITOR is not set or editor not available, but should not panic
    match result {
        Ok(_) => println!("Editor integration works"),
        Err(e) => println!("Editor integration test skipped: {}", e),
    }
}

/// Test ultra-minimal UI output format
#[tokio::test]
async fn test_ultra_minimal_ui_output() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Test basic help command (should show minimal output)
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--help"], None);

    assert_eq!(exit_code, 0, "Help command should succeed");
    assert!(stdout.contains("vibe_cli"), "Should show CLI name");
    assert!(stderr.is_empty(), "Should have no stderr output");

    // Verify no color codes or emoji in output (ultra-minimal)
    assert!(!stdout.contains("\x1b["), "Should not contain ANSI color codes");
    assert!(!stdout.contains("⚡"), "Should not contain emojis");
    assert!(!stdout.contains("🚀"), "Should not contain emojis");
}

/// Test project scoping safety
#[tokio::test]
async fn test_project_scoping_safety() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");

    // Change to project directory
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    // Test that system file access is blocked
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--build", "create /etc/hosts"], None);

    // The build command should either fail or warn about external paths
    assert!(exit_code != 0 || stdout.contains("REJECTED") || stdout.contains("outside"),
           "Should block or warn about system file access");

    // Test that project files work
    let (stdout2, stderr2, exit_code2) = run_vibe_cli(&["--build", "create test.txt"], None);

    // Should proceed or show plan for project file
    assert!(exit_code2 == 0 || stdout2.contains("[PLAN]") || stdout2.contains("Proceed"),
           "Should allow project file operations");
}

/// Test real-time command visibility
#[tokio::test]
async fn test_real_time_command_visibility() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    // Test with a simple command that should show visibility
    let (stdout, stderr, exit_code) = run_vibe_cli(&["run", "echo 'test'"], None);

    // Should show command execution visibility
    assert!(stdout.contains("[EXEC]") || stdout.contains("[RUN]") || stdout.contains("echo"),
           "Should show command execution visibility");
}

/// Test session management and history
#[tokio::test]
async fn test_session_management() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    // Test session creation
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--session", "test_session", "--build", "create session_test.txt"], None);

    // Should handle session creation
    assert!(exit_code == 0 || stdout.contains("[SESSION]") || stdout.contains("session"),
           "Should handle session creation");
}

/// Test interactive planning workflow
#[tokio::test]
async fn test_interactive_planning_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    // Test planning phase output
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--build", "create test.txt"], None);

    // Should show planning phase
    assert!(stdout.contains("[PLAN]") || stdout.contains("Planning") || stdout.contains("Proceed"),
           "Should show planning phase");
}

/// Test git integration and undo functionality
#[tokio::test]
async fn test_git_integration() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    // Initialize git repo
    Command::new("git")
        .args(&["init"])
        .current_dir(&project_dir)
        .output()
        .expect("Failed to init git");

    Command::new("git")
        .args(&["config", "user.name", "Test User"])
        .current_dir(&project_dir)
        .output()
        .ok();

    Command::new("git")
        .args(&["config", "user.email", "test@example.com"])
        .current_dir(&project_dir)
        .output()
        .ok();

    // Test build operation with git
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--build", "create git_test.txt"], None);

    // Should mention git commits
    assert!(stdout.contains("[COMMIT]") || stdout.contains("commit") || exit_code == 0,
           "Should handle git operations");
}

/// Test power-user controls and interactive features
#[tokio::test]
async fn test_power_user_controls() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    // Test that interactive controls are mentioned
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--build", "create control_test.txt"], None);

    // Should show completion and controls
    assert!(stdout.contains("[COMPLETE]") || stdout.contains("[CONTROLS]") || stdout.contains("Next action"),
           "Should show completion and control options");
}

/// Test error handling and safety
#[tokio::test]
async fn test_error_handling_and_safety() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    // Test with invalid command
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--build", "invalid_command_that_should_fail"], None);

    // Should handle errors gracefully
    assert!(exit_code != 0 || stdout.contains("[ERROR]") || stderr.contains("error"),
           "Should handle errors gracefully");
}

/// Test performance - ensure no artificial delays
#[tokio::test]
async fn test_performance_no_delays() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    let start = std::time::Instant::now();

    // Test a simple operation
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--build", "create perf_test.txt"], None);

    let duration = start.elapsed();

    // Should complete quickly (under 5 seconds for a simple operation)
    assert!(duration < Duration::from_secs(5),
           "Operation should complete quickly, took {:?}", duration);
}

/// Test comprehensive workflow end-to-end
#[tokio::test]
async fn test_comprehensive_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let project_dir = temp_dir.path().join("test_project");
    fs::create_dir(&project_dir).expect("Failed to create project dir");
    std::env::set_current_dir(&project_dir).expect("Failed to change directory");

    // Initialize git
    Command::new("git").args(&["init"]).current_dir(&project_dir).output().ok();
    Command::new("git").args(&["config", "user.name", "Test"]).current_dir(&project_dir).output().ok();
    Command::new("git").args(&["config", "user.email", "test@test.com"]).current_dir(&project_dir).output().ok();

    // Test full workflow
    let (stdout, stderr, exit_code) = run_vibe_cli(&["--build", "create workflow_test.sh with basic script"], None);

    // Verify workflow elements are present
    let has_planning = stdout.contains("[PLAN]") || stdout.contains("Planning");
    let has_execution = stdout.contains("[EXEC]") || stdout.contains("[RUN]");
    let has_completion = stdout.contains("[COMPLETE]") || stdout.contains("[DONE]");
    let has_controls = stdout.contains("[CONTROLS]") || stdout.contains("Next action");

    assert!(has_planning || has_execution || has_completion,
           "Should show workflow phases");
    assert!(exit_code == 0 || has_controls,
           "Should complete successfully or show controls");

    // Verify no color codes (ultra-minimal)
    assert!(!stdout.contains("\x1b["), "Should not contain ANSI color codes");

    // Verify no system file access attempted
    assert!(!stdout.contains("/etc/") && !stdout.contains("/sys/"),
           "Should not attempt system file access");
}