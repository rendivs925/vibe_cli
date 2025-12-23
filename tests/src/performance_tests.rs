// Performance benchmarks and load testing for Vibe CLI

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
async fn benchmark_command_generation() {
    let queries = vec![
        "list files",
        "show disk usage",
        "display memory usage",
        "check network status",
        "find large files",
    ];
    
    let start = Instant::now();
    for query in queries {
        let (stdout, stderr, exit_code) = run_vibe_cli(&[query], Some("y\n"));
        assert!(exit_code == 0, "Query should succeed: {}", query);
    }
    let elapsed = start.elapsed();
    
    // Should complete all queries within reasonable time
    assert!(elapsed < Duration::from_secs(60), "All queries should complete quickly");
    println!("Completed {} queries in {:?}", queries.len(), elapsed);
}

#[tokio::test]
async fn benchmark_rag_indexing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create a moderately sized codebase
    for i in 0..20 {
        let file_path = temp_dir.path().join(format!("module_{}.rs", i));
        fs::write(&file_path, format!("
pub struct Module{} {{
    id: u32,
    name: String,
}}

impl Module{} {{
    pub fn new(id: u32, name: String) -> Self {{
        Self {{ id, name }}
    }}
    
    pub fn process(&self) -> Result<(), String> {{
        if self.id == 0 {{
            return Err(\"Invalid ID\".to_string());
        }}
        println!(\"Processing {{}}\", self.name);
        Ok(())
    }}
}}
", i, i)).unwrap();
    }
    
    // Test RAG indexing performance
    let start = Instant::now();
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "--rag",
        "How many modules are there?"
    ], Some("y\n"));
    let elapsed = start.elapsed();
    
    assert!(exit_code == 0, "RAG indexing should succeed");
    assert!(elapsed < Duration::from_secs(45), "RAG indexing should complete in reasonable time");
    println!("RAG indexing of {} files completed in {:?}", 20, elapsed);
}

#[tokio::test]
async fn test_large_file_processing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create a large file to test processing performance
    let mut large_content = String::new();
    for i in 0..1000 {
        large_content.push_str(&format!("
// Function {} - Some documentation
fn function_{}(param: i32) -> i32 {{
    // This is a test function {}
    let result = param * 2 + {};
    println!(\"Processing function {{}} with result {{}}\", {}, result);
    result
}}
", i, i, i, i, i, i));
    }
    
    let large_file = temp_dir.path().join("large_file.rs");
    fs::write(&large_file, large_content).unwrap();
    
    // Test file explanation performance
    let start = Instant::now();
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "--explain",
        large_file.to_str().unwrap()
    ], None);
    let elapsed = start.elapsed();
    
    assert!(exit_code == 0, "Large file processing should succeed");
    assert!(elapsed < Duration::from_secs(30), "Large file processing should complete quickly");
    println!("Processed large file ({} functions) in {:?}", 1000, elapsed);
}

#[tokio::test]
async fn test_concurrent_requests() {
    use tokio::task;
    
    let queries = vec![
        "list files",
        "show disk usage", 
        "display memory info",
        "check network status",
        "show running processes",
    ];
    
    let start = Instant::now();
    let mut handles = vec![];
    
    for query in queries {
        let handle = task::spawn_blocking(move || {
            run_vibe_cli(&[&query], Some("y\n"))
        });
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    for handle in handles {
        let (stdout, stderr, exit_code) = handle.await.unwrap();
        assert!(exit_code == 0, "Concurrent query should succeed");
        assert!(!stdout.is_empty(), "Should generate command");
    }
    
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(60), "Concurrent requests should complete efficiently");
    println!("Completed {} concurrent requests in {:?}", 5, elapsed);
}

#[tokio::test]
async fn test_memory_usage() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create multiple files to test memory efficiency
    for i in 0..50 {
        let file_path = temp_dir.path().join(format!("test_{}.txt", i));
        let content = format!("This is test file {} with some content to process.\n", i);
        fs::write(&file_path, content).unwrap();
    }
    
    // Test multiple RAG queries
    for i in 0..5 {
        let (stdout, stderr, exit_code) = run_vibe_cli(&[
            "--rag",
            &format!("What information is in test file {}?", i * 10)
        ], Some("y\n"));
        
        assert!(exit_code == 0, "RAG query {} should succeed", i);
    }
    
    // If we reach here without memory issues, the test passes
    // In a real test, you might monitor actual memory usage
    println!("Memory usage test completed successfully");
}

#[tokio::test]
async fn test_cache_performance() {
    let query = "list all files in the current directory";
    
    // First run - should cache the result
    let start = Instant::now();
    let (stdout1, stderr1, exit_code1) = run_vibe_cli(&[query], Some("y\n"));
    let first_run_time = start.elapsed();
    
    assert!(exit_code1 == 0, "First run should succeed");
    assert!(!stdout1.is_empty(), "Should generate command");
    
    // Second run - should use cache
    let start = Instant::now();
    let (stdout2, stderr2, exit_code2) = run_vibe_cli(&[query], Some("y\n"));
    let second_run_time = start.elapsed();
    
    assert!(exit_code2 == 0, "Second run should succeed");
    assert!(!stdout2.is_empty(), "Should provide cached result");
    
    // Cache should be significantly faster (or at least not slower)
    println!("First run: {:?}, Second run (cached): {:?}", first_run_time, second_run_time);
    
    // We allow some tolerance since network latency might vary
    let tolerance = Duration::from_secs(2);
    assert!(second_run_time <= first_run_time + tolerance, 
            "Cached response should not be significantly slower");
}

#[tokio::test]
async fn test_agent_mode_performance() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create a small project
    fs::write(temp_dir.path().join("README.md"), "# Test Project\n\nThis is a test project.").unwrap();
    fs::create_dir_all(temp_dir.path().join("src")).unwrap();
    fs::write(temp_dir.path().join("src/main.rs"), "fn main() { println!(\"Hello\"); }").unwrap();
    
    // Test agent mode with a complex multi-step task
    let start = Instant::now();
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "--agent",
        "analyze this project and create a summary"
    ], Some("y\ny\ny\n")); // Accept all steps
    let elapsed = start.elapsed();
    
    assert!(exit_code == 0, "Agent mode should succeed");
    assert!(elapsed < Duration::from_secs(120), "Agent mode should complete within reasonable time");
    println!("Agent mode completed in {:?}", elapsed);
}

#[tokio::test]
async fn test_scalability() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    
    // Create a large codebase structure
    for i in 0..100 {
        let module_dir = temp_dir.path().join(format!("module_{}", i));
        fs::create_dir_all(&module_dir).unwrap();
        
        // Create multiple files per module
        for j in 0..5 {
            let file_path = module_dir.join(format!("file_{}.rs", j));
            fs::write(&file_path, format!("
pub struct File{}_{{{
    id: u32,
    data: String,
}}

impl File{}_{{
    pub fn new(id: u32, data: String) -> Self {{
        Self {{ id, data }}
    }}
    
    pub fn process(&self) -> bool {{
        !self.data.is_empty()
    }}
}}
", j, i, j, i)).unwrap();
        }
    }
    
    // Test RAG query on large codebase
    let start = Instant::now();
    let (stdout, stderr, exit_code) = run_vibe_cli(&[
        "--rag",
        "How many structs are defined across all modules?"
    ], Some("y\n"));
    let elapsed = start.elapsed();
    
    assert!(exit_code == 0, "Large codebase RAG should succeed");
    assert!(elapsed < Duration::from_secs(90), "Should handle large codebase efficiently");
    println!("Processed {} files in {:?}", 500, elapsed);
}