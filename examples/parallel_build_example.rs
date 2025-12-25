/// Example demonstrating parallel build mode with task decomposition
///
/// This example shows how to:
/// 1. Use BuildService for safe code modifications
/// 2. Decompose complex tasks into parallel subtasks
/// 3. Execute tasks in parallel with the orchestrator
/// 4. Aggregate results with conflict resolution
/// 5. Use transactions for safety

use application::{
    build_service::{BuildService, BuildPlan, FileOperation, ConfirmationMode},
    parallel_agent::{ParallelAgentOrchestrator, SubTask, SubTaskResult},
    task_decomposer::{TaskDecomposer, DecompositionStrategy},
    result_aggregator::{ResultAggregator, ConflictResolution, AggregationStrategy},
    transaction::Transaction,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Vibe CLI Parallel Build Example ===\n");

    // Example 1: Basic Build Service
    example_build_service().await?;

    // Example 2: Task Decomposition
    example_task_decomposition()?;

    // Example 3: Parallel Execution
    example_parallel_execution().await?;

    // Example 4: Result Aggregation
    example_result_aggregation()?;

    // Example 5: Complete Integration
    example_complete_integration().await?;

    println!("\n=== All Examples Completed Successfully ===");
    Ok(())
}

/// Example 1: Using BuildService for safe code modifications
async fn example_build_service() -> anyhow::Result<()> {
    println!("Example 1: BuildService with Transactions\n");

    let workspace = std::env::temp_dir().join("vibe_example");
    std::fs::create_dir_all(&workspace)?;

    // Create a BuildService
    let mut build_service = BuildService::new(&workspace);
    build_service.set_confirmation_mode(ConfirmationMode::None); // Auto-approve for example

    // Create a build plan
    let plan = BuildPlan {
        goal: "Create example module".to_string(),
        description: "Demonstrates safe file operations".to_string(),
        operations: vec![
            FileOperation::Create {
                path: workspace.join("example.rs"),
                content: "// Example module\npub fn hello() {\n    println!(\"Hello!\");\n}\n".to_string(),
            },
        ],
        estimated_risk: application::build_service::RiskLevel::Low,
    };

    // Preview the plan
    build_service.preview_plan(&plan)?;

    // Execute with transaction safety
    let result = build_service.execute_plan(&plan).await?;

    println!("Build completed: {} operations successful\n", result.operations_completed);

    // Cleanup
    std::fs::remove_dir_all(&workspace)?;

    Ok(())
}

/// Example 2: Task Decomposition Strategies
fn example_task_decomposition() -> anyhow::Result<()> {
    println!("Example 2: Intelligent Task Decomposition\n");

    let goal = "Implement user authentication with database integration";

    // Try different decomposition strategies
    let strategies = vec![
        DecompositionStrategy::ByFile,
        DecompositionStrategy::ByFeature,
        DecompositionStrategy::ByLayer,
        DecompositionStrategy::Intelligent,
    ];

    for strategy in strategies {
        let decomposer = TaskDecomposer::new(strategy);
        let tasks = decomposer.decompose(goal)?;

        println!("Strategy: {:?}", strategy);
        println!("Tasks created: {}", tasks.len());
        for task in tasks.iter().take(3) {
            println!("  - {} (priority: {})", task.description, task.priority);
        }
        println!();
    }

    Ok(())
}

/// Example 3: Parallel Execution with Dependencies
async fn example_parallel_execution() -> anyhow::Result<()> {
    println!("Example 3: Parallel Task Execution\n");

    // Create orchestrator
    let orchestrator = ParallelAgentOrchestrator::new(4);

    // Create tasks with dependencies
    let tasks = vec![
        SubTask {
            id: "analyze".to_string(),
            description: "Analyze requirements".to_string(),
            priority: 10,
            dependencies: vec![],
            estimated_complexity: 0.3,
        },
        SubTask {
            id: "impl_a".to_string(),
            description: "Implement feature A".to_string(),
            priority: 9,
            dependencies: vec!["analyze".to_string()],
            estimated_complexity: 0.6,
        },
        SubTask {
            id: "impl_b".to_string(),
            description: "Implement feature B".to_string(),
            priority: 9,
            dependencies: vec!["analyze".to_string()],
            estimated_complexity: 0.6,
        },
        SubTask {
            id: "integrate".to_string(),
            description: "Integrate features".to_string(),
            priority: 8,
            dependencies: vec!["impl_a".to_string(), "impl_b".to_string()],
            estimated_complexity: 0.4,
        },
    ];

    // Executor function (simulated)
    let executor = |task: SubTask| async move {
        // Simulate work
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        Ok(SubTaskResult {
            task_id: task.id.clone(),
            success: true,
            output: format!("Completed: {}", task.description),
            execution_time_ms: 100,
            error: None,
        })
    };

    // Execute in parallel
    let results = orchestrator.execute_parallel(tasks, executor).await?;

    println!("Executed {} tasks in parallel", results.len());
    println!("All tasks succeeded: {}\n", results.iter().all(|r| r.success));

    Ok(())
}

/// Example 4: Result Aggregation with Conflict Resolution
fn example_result_aggregation() -> anyhow::Result<()> {
    println!("Example 4: Result Aggregation\n");

    // Create some sample results
    let results = vec![
        SubTaskResult {
            task_id: "task_1".to_string(),
            success: true,
            output: "Implemented authentication module".to_string(),
            execution_time_ms: 150,
            error: None,
        },
        SubTaskResult {
            task_id: "task_2".to_string(),
            success: true,
            output: "Added database integration".to_string(),
            execution_time_ms: 200,
            error: None,
        },
        SubTaskResult {
            task_id: "task_3".to_string(),
            success: true,
            output: "Created test suite".to_string(),
            execution_time_ms: 100,
            error: None,
        },
    ];

    // Create aggregator
    let aggregator = ResultAggregator::new(
        ConflictResolution::Merge,
        AggregationStrategy::Structured,
    );

    // Aggregate results
    let aggregated = aggregator.aggregate(results)?;

    println!("Aggregation complete:");
    println!("  Tasks: {}", aggregated.task_count);
    println!("  Successful: {}", aggregated.success_count);
    println!("  Conflicts resolved: {}", aggregated.conflicts_resolved);
    println!("\nCombined output:\n{}\n", aggregated.combined_output);

    Ok(())
}

/// Example 5: Complete Integration - Build + Parallel + Aggregation
async fn example_complete_integration() -> anyhow::Result<()> {
    println!("Example 5: Complete Integration\n");

    let workspace = std::env::temp_dir().join("vibe_integration");
    std::fs::create_dir_all(&workspace)?;

    // Step 1: Decompose the task
    let goal = "Create a new Rust module with tests";
    let decomposer = TaskDecomposer::new(DecompositionStrategy::Intelligent);
    let tasks = decomposer.decompose(goal)?;

    println!("Decomposed into {} tasks", tasks.len());

    // Step 2: Execute tasks in parallel
    let orchestrator = ParallelAgentOrchestrator::new(0); // Auto-detect CPUs

    let executor = |task: SubTask| async move {
        // Simulate task execution
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        Ok(SubTaskResult {
            task_id: task.id.clone(),
            success: true,
            output: format!("Executed: {}", task.description),
            execution_time_ms: 50,
            error: None,
        })
    };

    let results = orchestrator.execute_parallel(tasks, executor).await?;

    // Step 3: Aggregate results
    let aggregator = ResultAggregator::new(
        ConflictResolution::Merge,
        AggregationStrategy::Summary,
    );

    let final_result = aggregator.aggregate(results)?;

    println!("\nFinal Results:");
    println!("{}", final_result.execution_summary);
    println!("\nSuccess rate: {}/{}",
        final_result.success_count,
        final_result.task_count
    );

    // Cleanup
    std::fs::remove_dir_all(&workspace)?;

    Ok(())
}

/// Bonus: Transaction safety example
#[allow(dead_code)]
async fn example_transaction_safety() -> anyhow::Result<()> {
    println!("Bonus: Transaction Safety\n");

    let temp_file = std::env::temp_dir().join("transaction_test.txt");

    // Create initial file
    std::fs::write(&temp_file, "original content")?;

    // Use transaction for safe modification
    let mut transaction = Transaction::new();
    transaction.begin()?;

    // Modify file within transaction
    transaction.write_file(&temp_file, b"modified content")?;

    // Simulate an error
    println!("Simulating error - transaction will auto-rollback");
    drop(transaction); // Auto-rollback on drop

    // Verify rollback worked
    let content = std::fs::read_to_string(&temp_file)?;
    assert_eq!(content, "original content");

    println!("Transaction rolled back successfully!\n");

    // Cleanup
    std::fs::remove_file(&temp_file)?;

    Ok(())
}
