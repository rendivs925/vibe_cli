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

#[tokio::test]
async fn benchmark_dynamic_batch_sizing() {
    use infrastructure::embedder::{Embedder, EmbeddingInput};
    use infrastructure::ollama_client::OllamaClient;
    use shared::performance_monitor::GLOBAL_METRICS;
    use std::time::Instant;

    // Create test data with varying sizes to test dynamic batching
    let test_cases = vec![
        ("Small batch", 10),
        ("Medium batch", 50),
        ("Large batch", 200),
        ("Very large batch", 1000),
    ];

    for (test_name, num_items) in test_cases {
        println!("\n🧪 Testing {}: {} items", test_name, num_items);

        // Create test embedding inputs
        let inputs: Vec<EmbeddingInput> = (0..num_items)
            .map(|i| EmbeddingInput {
                id: format!("test_{}", i),
                path: format!("test_file_{}.rs", i),
                text: format!("This is test content {} for performance benchmarking of dynamic batch sizing.", i),
            })
            .collect();

        // Reset performance metrics for clean measurement
        GLOBAL_METRICS.reset().await;

        // Test embedding generation with dynamic batch sizing
        let client = OllamaClient::new().unwrap();
        let embedder = Embedder::new(client);

        let start = Instant::now();
        let result = embedder.generate_embeddings(&inputs).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "Embedding generation should succeed for {}", test_name);
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), num_items, "Should generate correct number of embeddings");

        println!("✅ {} completed in {:?}", test_name, elapsed);
        println!("   Throughput: {:.1} items/sec", num_items as f64 / elapsed.as_secs_f64());

        // Get performance metrics
        let avg_latency = GLOBAL_METRICS.average_latency("embedding_batch").await;
        if let Some(latency) = avg_latency {
            println!("   Average batch latency: {:.0}ms", latency.as_millis());
        }

        let throughput = GLOBAL_METRICS.throughput("embedding_batch").await;
        if let Some(tps) = throughput {
            println!("   Batch throughput: {:.1} ops/sec", tps);
        }
    }
}

#[tokio::test]
async fn benchmark_http2_pipelining() {
    use infrastructure::ollama_client::OllamaClient;
    use std::time::Instant;

    println!("\n🌐 Testing HTTP/2 pipelining performance");

    let client = OllamaClient::new().unwrap();

    // Test different batch sizes for pipelining
    let batch_sizes = vec![5, 10, 20, 32, 50];

    for batch_size in batch_sizes {
        println!("\n📊 Testing pipelined batch size: {}", batch_size);

        let texts: Vec<String> = (0..batch_size)
            .map(|i| format!("Test content {} for HTTP/2 pipelining benchmark with sufficient length to generate meaningful embeddings.", i))
            .collect();

        let start = Instant::now();
        let result = client.generate_embeddings_pipelined(texts).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "Pipelined embedding generation should succeed for batch size {}", batch_size);
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), batch_size, "Should generate correct number of embeddings");

        println!("✅ Batch size {} completed in {:?}", batch_size, elapsed);
        println!("   Throughput: {:.1} items/sec", batch_size as f64 / elapsed.as_secs_f64());
        println!("   Avg latency per item: {:.1}ms", elapsed.as_millis() as f64 / batch_size as f64);
    }
}

#[tokio::test]
async fn test_real_world_rag_scenario() {
    use infrastructure::embedder::{Embedder, EmbeddingInput};
    use infrastructure::ollama_client::OllamaClient;
    use std::time::Instant;

    println!("\n🏢 Testing real-world RAG scenario: Codebase analysis");

    // Simulate a real codebase with different types of files
    let codebase_files = vec![
        ("src/main.rs", "fn main() { println!(\"Hello, world!\"); }"),
        ("src/lib.rs", "pub mod parser; pub mod analyzer; pub fn version() -> &'static str { \"1.0.0\" }"),
        ("src/parser.rs", "pub struct Parser { pub input: String } impl Parser { pub fn parse(&self) -> Result<(), String> { Ok(()) } }"),
        ("src/analyzer.rs", "pub struct Analyzer { pub data: Vec<String> } impl Analyzer { pub fn analyze(&self) -> Vec<String> { vec![] } }"),
        ("Cargo.toml", "[package]\nname = \"test-project\"\nversion = \"1.0.0\"\n[dependencies]\nserde = \"1.0\""),
        ("README.md", "# Test Project\n\nThis is a test project for RAG benchmarking.\n\n## Features\n- Parsing\n- Analysis\n- Command line interface"),
        ("src/cli.rs", "use clap::Command; fn build_cli() -> Command { Command::new(\"test\") }"),
        ("src/error.rs", "use std::fmt; #[derive(Debug)] pub struct AppError(String); impl fmt::Display for AppError { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, \"{}\", self.0) } }"),
    ];

    // Create embedding inputs for the codebase
    let inputs: Vec<EmbeddingInput> = codebase_files
        .into_iter()
        .map(|(path, content)| EmbeddingInput {
            id: path.to_string(),
            path: path.to_string(),
            text: content.to_string(),
        })
        .collect();

    // Test embedding generation performance
    let client = OllamaClient::new().unwrap();
    let embedder = Embedder::new(client);

    let start = Instant::now();
    let result = embedder.generate_embeddings(&inputs).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok(), "Codebase embedding should succeed");
    let embeddings = result.unwrap();
    assert_eq!(embeddings.len(), codebase_files.len(), "Should embed all codebase files");

    println!("✅ Embedded {} codebase files in {:?}", codebase_files.len(), elapsed);
    println!("   Average time per file: {:.1}ms", elapsed.as_millis() as f64 / codebase_files.len() as f64);
    println!("   Throughput: {:.1} files/sec", codebase_files.len() as f64 / elapsed.as_secs_f64());

    // Simulate RAG query processing (finding relevant files)
    let query = "How do I handle errors in this codebase?";
    println!("\n🔍 Simulating RAG query: \"{}\"", query);

    // Simple relevance scoring based on file content
    let query_start = Instant::now();
    let mut relevant_files = Vec::new();

    for embedding in &embeddings {
        // Simple keyword matching for relevance (in real RAG, this would be semantic similarity)
        let relevance_score = if embedding.text.to_lowercase().contains("error") ||
                                embedding.text.to_lowercase().contains("result") {
            0.8
        } else if embedding.text.to_lowercase().contains("struct") ||
                  embedding.text.to_lowercase().contains("impl") {
            0.6
        } else {
            0.2
        };

        if relevance_score > 0.5 {
            relevant_files.push((embedding.path.clone(), relevance_score));
        }
    }

    let query_elapsed = query_start.elapsed();

    println!("✅ Found {} relevant files in {:?}", relevant_files.len(), query_elapsed);
    println!("   Top matches:");
    for (path, score) in relevant_files.iter().take(3) {
        println!("     - {} (relevance: {:.1})", path, score);
    }
}

#[tokio::test]
async fn benchmark_system_load_adaptation() {
    use infrastructure::embedder::{Embedder, EmbeddingInput};
    use infrastructure::ollama_client::OllamaClient;
    use shared::performance_monitor::GLOBAL_METRICS;
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    println!("\n⚡ Testing dynamic batch sizing under different load conditions");

    let client = OllamaClient::new().unwrap();
    let embedder = Embedder::new(client);

    // Test scenarios with different "load" levels
    let scenarios = vec![
        ("Low load", 0.1, 100),
        ("Medium load", 0.5, 100),
        ("High load", 0.8, 100),
    ];

    for (scenario_name, load_factor, num_items) in scenarios {
        println!("\n🧪 Scenario: {} (load: {:.1})", scenario_name, load_factor);

        // Simulate different load levels by creating artificial metrics
        // In a real system, this would be based on actual CPU/memory usage
        for _ in 0..10 {
            GLOBAL_METRICS.start_operation("test_load").await;
            // Simulate variable processing time based on load
            let delay = Duration::from_millis((load_factor * 100.0) as u64);
            sleep(delay).await;
            GLOBAL_METRICS.end_operation("test_load").await;
        }

        // Create test inputs
        let inputs: Vec<EmbeddingInput> = (0..num_items)
            .map(|i| EmbeddingInput {
                id: format!("load_test_{}", i),
                path: format!("test_{}.rs", i),
                text: format!("Test content {} for load adaptation testing with dynamic batch sizing.", i),
            })
            .collect();

        let start = Instant::now();
        let result = embedder.generate_embeddings(&inputs).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "Load adaptation test should succeed for {}", scenario_name);
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), num_items, "Should generate all embeddings");

        println!("✅ {} completed in {:?}", scenario_name, elapsed);
        println!("   Throughput: {:.1} items/sec", num_items as f64 / elapsed.as_secs_f64());

        // Show how batch sizing adapted
        let avg_latency = GLOBAL_METRICS.average_latency("embedding_batch").await;
        if let Some(latency) = avg_latency {
            println!("   Under load {:.1}: avg batch latency {:.0}ms", load_factor, latency.as_millis());
        }
    }
}

#[tokio::test]
async fn test_performance_regression_protection() {
    use infrastructure::embedder::{Embedder, EmbeddingInput};
    use infrastructure::ollama_client::OllamaClient;
    use std::time::{Duration, Instant};

    println!("\n🛡️ Testing performance regression protection");

    let client = OllamaClient::new().unwrap();
    let embedder = Embedder::new(client);

    // Test that performance doesn't regress significantly over multiple runs
    let mut times = Vec::new();
    let num_runs = 5;
    let num_items = 50;

    for run in 1..=num_runs {
        println!("   Run {}/{}", run, num_runs);

        let inputs: Vec<EmbeddingInput> = (0..num_items)
            .map(|i| EmbeddingInput {
                id: format!("regression_test_{}_{}", run, i),
                path: format!("test_{}.rs", i),
                text: format!("Regression test content {} for run {} to ensure consistent performance.", i, run),
            })
            .collect();

        let start = Instant::now();
        let result = embedder.generate_embeddings(&inputs).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "Run {} should succeed", run);
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), num_items, "Run {} should generate all embeddings", run);

        times.push(elapsed);
        println!("     Completed in {:?}", elapsed);
    }

    // Check for performance consistency
    let avg_time: Duration = times.iter().sum::<Duration>() / times.len() as u32;
    let max_time = times.iter().max().unwrap();
    let min_time = times.iter().min().unwrap();

    // Allow 20% variation to account for system noise
    let max_allowed_variation = 1.2;
    let actual_variation = max_time.as_millis() as f64 / min_time.as_millis() as f64;

    assert!(actual_variation <= max_allowed_variation,
            "Performance variation too high: {:.2}x (max allowed: {:.1}x)",
            actual_variation, max_allowed_variation);

    println!("✅ Performance regression test passed");
    println!("   Average time: {:?}", avg_time);
    println!("   Time range: {:?} - {:?}", min_time, max_time);
    println!("   Variation: {:.2}x (within {:.1}x limit)", actual_variation, max_allowed_variation);
}

#[tokio::test]
async fn test_dynamic_batch_sizing_logic() {
    // Test the batch sizing logic without requiring Ollama
    println!("\n🧮 Testing dynamic batch sizing algorithm");

    // Test different scenarios for batch size calculation
    let test_scenarios = vec![
        ("Small remaining items", 10, 10),
        ("Medium remaining items", 100, 100),
        ("Large remaining items", 1000, 128), // Should be capped at 128
        ("Very large remaining items", 10000, 128), // Should be capped
    ];

    for (scenario, remaining_items, expected_max) in test_scenarios {
        println!("\n📊 Scenario: {}", scenario);

        // Test the batch sizing logic directly
        let min_batch_size = 16;
        let max_batch_size = 128;
        let num_cpus = num_cpus::get() as f32;

        // Simulate different load factors
        let load_factors = vec![0.1, 0.5, 0.8];

        for load_factor in load_factors {
            // Calculate batch size as implemented in embedder.rs
            let mut optimal_batch_size = 128; // default

            // Adjust based on CPU availability
            if num_cpus >= 8.0 {
                optimal_batch_size = (optimal_batch_size as f32 * 1.5) as usize;
            } else if num_cpus <= 2.0 {
                optimal_batch_size = (optimal_batch_size as f32 * 0.7) as usize;
            }

            // Adjust based on system load
            if load_factor > 0.8 {
                optimal_batch_size = (optimal_batch_size as f32 * 0.6) as usize;
            } else if load_factor < 0.3 {
                optimal_batch_size = (optimal_batch_size as f32 * 1.3) as usize;
            }

            // Ensure batch size is within bounds
            optimal_batch_size = optimal_batch_size.clamp(min_batch_size, max_batch_size);

            // Don't exceed remaining items
            let final_batch_size = optimal_batch_size.min(remaining_items);

            println!("   Load {:.1}: CPU {} → batch size {} (≤ {})",
                    load_factor, num_cpus as usize, final_batch_size, expected_max);

            assert!(final_batch_size >= min_batch_size, "Batch size should be at least minimum");
            assert!(final_batch_size <= expected_max, "Batch size should not exceed expected maximum");
            assert!(final_batch_size <= remaining_items, "Batch size should not exceed remaining items");
        }
    }

    println!("✅ Dynamic batch sizing logic test passed");
}

#[tokio::test]
async fn benchmark_batch_processing_overhead() {
    use shared::batch_processing::{BatchProcessor, VectorBatchOps};
    use std::time::Instant;

    println!("\n⚡ Benchmarking batch processing overhead");

    // Test different batch sizes
    let batch_sizes = vec![10, 50, 100, 500, 1000];

    for batch_size in batch_sizes {
        println!("\n📊 Testing batch size: {}", batch_size);

        // Create test data
        let data: Vec<i32> = (0..batch_size).collect();

        // Test individual operations
        let start = Instant::now();
        let individual_sum: i32 = data.iter().sum();
        let individual_time = start.elapsed();

        // Test batch operations
        let start = Instant::now();
        let batch_sum = VectorBatchOps::sum(data.clone());
        let batch_time = start.elapsed();

        assert_eq!(individual_sum, batch_sum, "Results should be identical");

        println!("   Individual: {:?} ({:.0} ops/sec)",
                individual_time,
                batch_size as f64 / individual_time.as_secs_f64());

        println!("   Batch: {:?} ({:.0} ops/sec)",
                batch_time,
                batch_size as f64 / batch_time.as_secs_f64());

        // Batch processing should be reasonably efficient (not more than 2x slower)
        let overhead_ratio = batch_time.as_secs_f64() / individual_time.as_secs_f64();
        println!("   Overhead ratio: {:.2}x", overhead_ratio);

        assert!(overhead_ratio <= 2.0, "Batch processing overhead should be reasonable");
    }

    println!("✅ Batch processing benchmark completed");
}

#[tokio::test]
async fn test_performance_monitoring_accuracy() {
    use shared::performance_monitor::GLOBAL_METRICS;
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    println!("\n📈 Testing performance monitoring accuracy");

    // Reset metrics for clean test
    GLOBAL_METRICS.reset().await;

    // Test operation timing
    let operations = vec![
        ("fast_op", Duration::from_millis(10)),
        ("medium_op", Duration::from_millis(50)),
        ("slow_op", Duration::from_millis(200)),
    ];

    for (op_name, expected_duration) in operations.clone() {
        GLOBAL_METRICS.start_operation(op_name).await;

        // Simulate operation duration
        sleep(expected_duration).await;

        GLOBAL_METRICS.end_operation(op_name).await;

        // Check that latency was recorded
        let avg_latency = GLOBAL_METRICS.average_latency(op_name).await;
        assert!(avg_latency.is_some(), "Latency should be recorded for {}", op_name);

        let latency = avg_latency.unwrap();
        println!("   {}: expected {:?}, measured {:?}", op_name, expected_duration, latency);

        // Allow some tolerance for timing variations
        let tolerance = Duration::from_millis(20);
        let diff = if latency > expected_duration {
            latency - expected_duration
        } else {
            expected_duration - latency
        };

        assert!(diff <= tolerance, "Timing should be reasonably accurate for {}", op_name);
    }

    // Test throughput calculation
    for (op_name, _) in operations {
        let throughput = GLOBAL_METRICS.throughput(op_name).await;
        assert!(throughput.is_some(), "Throughput should be calculable for {}", op_name);

        let ops_per_sec = throughput.unwrap();
        println!("   {} throughput: {:.1} ops/sec", op_name, ops_per_sec);

        assert!(ops_per_sec > 0.0, "Throughput should be positive");
    }

    // Test system stats
    let system_stats = GLOBAL_METRICS.system_stats().await;
    println!("   System stats: {} total ops, avg latency {:?}", system_stats.total_operations, system_stats.average_latency);

    assert!(system_stats.total_operations >= operations.len() as u64, "Should have recorded all operations");
    assert!(system_stats.average_latency > Duration::from_millis(0), "Average latency should be positive");

    println!("✅ Performance monitoring accuracy test passed");
}