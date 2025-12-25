use shared::types::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::task::JoinHandle;
use colored::Colorize;

/// Represents a sub-task that can be executed in parallel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub description: String,
    pub priority: u8, // 0-10, higher = more important
    pub dependencies: Vec<String>, // IDs of tasks that must complete first
    pub estimated_complexity: f32, // 0.0-1.0
}

/// Result of a sub-task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTaskResult {
    pub task_id: String,
    pub success: bool,
    pub output: String,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

/// Status of parallel execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelExecutionStatus {
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub running_tasks: usize,
    pub pending_tasks: usize,
}

/// Orchestrator for parallel agent execution
pub struct ParallelAgentOrchestrator {
    max_concurrent_tasks: usize,
    enable_load_balancing: bool,
}

impl ParallelAgentOrchestrator {
    /// Create a new parallel agent orchestrator
    pub fn new(max_concurrent_tasks: usize) -> Self {
        let cpu_count = num_cpus::get();
        let max_tasks = if max_concurrent_tasks == 0 {
            cpu_count
        } else {
            max_concurrent_tasks.min(cpu_count * 2) // Cap at 2x CPU count
        };

        println!(
            "{}",
            format!(
                "Initializing parallel orchestrator (CPUs: {}, Max concurrent: {})",
                cpu_count, max_tasks
            )
            .bright_cyan()
        );

        Self {
            max_concurrent_tasks: max_tasks,
            enable_load_balancing: true,
        }
    }

    /// Break down a complex task into parallel sub-tasks
    pub fn decompose_task(&self, goal: &str) -> Result<Vec<SubTask>> {
        // This is a placeholder implementation
        // In a real implementation, this would use AI to intelligently decompose the task

        println!("{}", format!("Decomposing task: {}", goal).bright_yellow());

        // For now, create a simple demonstration decomposition
        let sub_tasks = vec![
            SubTask {
                id: "task_1".to_string(),
                description: format!("Analyze requirements for: {}", goal),
                priority: 10,
                dependencies: vec![],
                estimated_complexity: 0.3,
            },
            SubTask {
                id: "task_2".to_string(),
                description: "Search codebase for relevant files".to_string(),
                priority: 8,
                dependencies: vec!["task_1".to_string()],
                estimated_complexity: 0.5,
            },
            SubTask {
                id: "task_3".to_string(),
                description: "Generate implementation plan".to_string(),
                priority: 7,
                dependencies: vec!["task_1".to_string(), "task_2".to_string()],
                estimated_complexity: 0.6,
            },
        ];

        println!(
            "{}",
            format!("Created {} sub-tasks", sub_tasks.len()).bright_green()
        );

        Ok(sub_tasks)
    }

    /// Execute sub-tasks in parallel with dependency resolution
    pub async fn execute_parallel<F, Fut>(
        &self,
        tasks: Vec<SubTask>,
        executor: F,
    ) -> Result<Vec<SubTaskResult>>
    where
        F: Fn(SubTask) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<SubTaskResult>> + Send + 'static,
    {
        let total_tasks = tasks.len();
        println!(
            "{}",
            format!("Starting parallel execution of {} tasks", total_tasks).bright_cyan()
        );

        let mut results = Vec::new();
        let mut completed_task_ids: Vec<String> = Vec::new();
        let mut remaining_tasks = tasks;

        while !remaining_tasks.is_empty() {
            // Find tasks that can be executed (dependencies met)
            let ready_tasks: Vec<SubTask> = remaining_tasks
                .iter()
                .filter(|task| {
                    task.dependencies
                        .iter()
                        .all(|dep| completed_task_ids.contains(dep))
                })
                .cloned()
                .collect();

            if ready_tasks.is_empty() && !remaining_tasks.is_empty() {
                return Err(anyhow::anyhow!(
                    "Circular dependency detected or missing dependencies"
                ));
            }

            // Remove ready tasks from remaining
            remaining_tasks.retain(|task| !ready_tasks.iter().any(|rt| rt.id == task.id));

            // Execute ready tasks in parallel (up to max_concurrent_tasks)
            let batch_size = ready_tasks.len().min(self.max_concurrent_tasks);
            let current_batch = &ready_tasks[..batch_size];

            println!(
                "{}",
                format!(
                    "Executing batch of {} tasks (remaining: {})",
                    batch_size,
                    remaining_tasks.len() + ready_tasks.len() - batch_size
                )
                .bright_yellow()
            );

            let mut handles: Vec<JoinHandle<Result<SubTaskResult>>> = Vec::new();

            for task in current_batch {
                let task_clone = task.clone();
                let executor_clone = executor.clone();

                let handle = tokio::spawn(async move {
                    println!("{}", format!("  → Starting: {}", task_clone.description).cyan());
                    let start = std::time::Instant::now();

                    match executor_clone(task_clone.clone()).await {
                        Ok(mut result) => {
                            result.execution_time_ms = start.elapsed().as_millis() as u64;
                            println!(
                                "{}",
                                format!(
                                    "  ✓ Completed: {} ({}ms)",
                                    task_clone.description, result.execution_time_ms
                                )
                                .green()
                            );
                            Ok(result)
                        }
                        Err(e) => {
                            let execution_time = start.elapsed().as_millis() as u64;
                            eprintln!(
                                "{}",
                                format!("  ✗ Failed: {} - {}", task_clone.description, e).red()
                            );
                            Ok(SubTaskResult {
                                task_id: task_clone.id.clone(),
                                success: false,
                                output: String::new(),
                                execution_time_ms: execution_time,
                                error: Some(e.to_string()),
                            })
                        }
                    }
                });

                handles.push(handle);
            }

            // Wait for all tasks in this batch to complete
            for handle in handles {
                let result = handle.await??;
                completed_task_ids.push(result.task_id.clone());
                results.push(result);
            }

            // Add remaining ready tasks that weren't in this batch
            if ready_tasks.len() > batch_size {
                remaining_tasks.extend(ready_tasks[batch_size..].to_vec());
            }
        }

        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();

        println!("\n{}", "=== Parallel Execution Summary ===".bright_cyan().bold());
        println!("{}: {}", "Total tasks".white(), total_tasks);
        println!("{}: {}", "Successful".green(), successful);
        if failed > 0 {
            println!("{}: {}", "Failed".red(), failed);
        }

        let total_time: u64 = results.iter().map(|r| r.execution_time_ms).sum();
        let max_time: u64 = results.iter().map(|r| r.execution_time_ms).max().unwrap_or(0);

        println!(
            "{}: {}ms (sequential would be ~{}ms)",
            "Execution time".white(),
            max_time,
            total_time
        );

        if total_time > 0 {
            let speedup = total_time as f64 / max_time.max(1) as f64;
            println!("{}: {:.2}x", "Speedup".bright_green(), speedup);
        }

        Ok(results)
    }

    /// Get current execution status
    pub fn get_status(&self, results: &[SubTaskResult], total_tasks: usize) -> ParallelExecutionStatus {
        ParallelExecutionStatus {
            total_tasks,
            completed_tasks: results.iter().filter(|r| r.success).count(),
            failed_tasks: results.iter().filter(|r| !r.success).count(),
            running_tasks: 0, // Would track in real implementation
            pending_tasks: total_tasks.saturating_sub(results.len()),
        }
    }

    /// Aggregate results from parallel execution
    pub fn aggregate_results(&self, results: Vec<SubTaskResult>) -> String {
        let mut output = String::new();

        output.push_str("Aggregated Results:\n\n");

        for result in &results {
            output.push_str(&format!("Task: {}\n", result.task_id));
            output.push_str(&format!("Status: {}\n", if result.success { "✓" } else { "✗" }));
            if !result.output.is_empty() {
                output.push_str(&format!("Output: {}\n", result.output));
            }
            if let Some(error) = &result.error {
                output.push_str(&format!("Error: {}\n", error));
            }
            output.push_str("\n");
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parallel_orchestrator_creation() {
        let orchestrator = ParallelAgentOrchestrator::new(4);
        assert!(orchestrator.max_concurrent_tasks > 0);
    }

    #[tokio::test]
    async fn test_task_decomposition() {
        let orchestrator = ParallelAgentOrchestrator::new(4);
        let tasks = orchestrator
            .decompose_task("Implement a new feature")
            .unwrap();
        assert!(!tasks.is_empty());
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        let orchestrator = ParallelAgentOrchestrator::new(2);

        let tasks = vec![
            SubTask {
                id: "task_1".to_string(),
                description: "Task 1".to_string(),
                priority: 10,
                dependencies: vec![],
                estimated_complexity: 0.5,
            },
            SubTask {
                id: "task_2".to_string(),
                description: "Task 2".to_string(),
                priority: 9,
                dependencies: vec![],
                estimated_complexity: 0.5,
            },
        ];

        let executor = |task: SubTask| async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Ok(SubTaskResult {
                task_id: task.id,
                success: true,
                output: "Success".to_string(),
                execution_time_ms: 100,
                error: None,
            })
        };

        let results = orchestrator.execute_parallel(tasks, executor).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
    }

    #[tokio::test]
    async fn test_dependency_resolution() {
        let orchestrator = ParallelAgentOrchestrator::new(4);

        let tasks = vec![
            SubTask {
                id: "task_1".to_string(),
                description: "Task 1".to_string(),
                priority: 10,
                dependencies: vec![],
                estimated_complexity: 0.5,
            },
            SubTask {
                id: "task_2".to_string(),
                description: "Task 2".to_string(),
                priority: 9,
                dependencies: vec!["task_1".to_string()],
                estimated_complexity: 0.5,
            },
        ];

        let executor = |task: SubTask| async move {
            Ok(SubTaskResult {
                task_id: task.id,
                success: true,
                output: "Success".to_string(),
                execution_time_ms: 10,
                error: None,
            })
        };

        let results = orchestrator.execute_parallel(tasks, executor).await.unwrap();
        assert_eq!(results.len(), 2);

        // Task 1 should complete before task 2
        let task_1_idx = results.iter().position(|r| r.task_id == "task_1").unwrap();
        let task_2_idx = results.iter().position(|r| r.task_id == "task_2").unwrap();
        assert!(task_1_idx < task_2_idx);
    }
}
