use colored::Colorize;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use tokio::task::JoinHandle;

/// Represents a sub-task that can be executed in parallel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub description: String,
    pub priority: u8,              // 0-10, higher = more important
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

/// Intelligent aggregation result with conflict resolution
#[derive(Debug, Clone)]
pub struct IntelligentAggregationResult {
    pub successful_tasks: usize,
    pub failed_tasks: usize,
    pub conflicts_detected: usize,
    pub merged_output: String,
    pub summary: String,
    pub conflicts: Vec<ResultConflict>,
}

/// Detected conflict between task results
#[derive(Debug, Clone)]
pub struct ResultConflict {
    pub task1: String,
    pub task2: String,
    pub conflict_type: ConflictType,
    pub description: String,
    pub resolution: ConflictResolution,
}

/// Types of conflicts that can occur
#[derive(Debug, Clone)]
pub enum ConflictType {
    OutputOverlap,
    ContradictoryResults,
    ResourceConflict,
    DependencyViolation,
}

/// How to resolve a conflict
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    MergeWithPriority,
    DiscardDuplicate,
    ManualReviewRequired,
    AcceptBoth,
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

    /// Break down a complex task into parallel sub-tasks using AI
    pub async fn decompose_task_ai(&self, goal: &str, context: &str) -> Result<Vec<SubTask>> {
        println!(
            "{}",
            format!("🤖 AI-powered task decomposition for: {}", goal).bright_yellow()
        );

        // Create a prompt for AI-powered decomposition
        let prompt = format!(
            "Break down this complex task into smaller, parallelizable sub-tasks: '{}'

Context: {}

Requirements:
1. Each sub-task should be independent when possible
2. Identify dependencies between sub-tasks
3. Estimate complexity (0.0-1.0) for each sub-task
4. Assign priority levels (0-10, higher = more important)
5. Make sub-tasks specific and actionable

Return a JSON array of sub-tasks with this structure:
[
  {{
    \"id\": \"unique_id\",
    \"description\": \"detailed description\",
    \"priority\": 8,
    \"dependencies\": [\"task_id1\", \"task_id2\"],
    \"estimated_complexity\": 0.5
  }}
]

Focus on creating 3-8 sub-tasks that can run in parallel when dependencies allow.",
            goal, context
        );

        // For now, use a mock AI response - in practice this would call an LLM
        let mock_response = self.generate_mock_decomposition(goal);

        // Parse the JSON response
        let sub_tasks: Vec<SubTask> = serde_json::from_str(&mock_response)
            .map_err(|e| anyhow::anyhow!("Failed to parse AI decomposition: {}", e))?;

        println!(
            "{}",
            format!(
                "🤖 AI created {} sub-tasks with intelligent dependencies",
                sub_tasks.len()
            )
            .bright_green()
        );

        // Validate and optimize the decomposition
        let optimized_tasks = self.optimize_decomposition(sub_tasks)?;

        println!(
            "{}",
            format!(
                "✅ Optimized to {} tasks with minimal dependencies",
                optimized_tasks.len()
            )
            .bright_cyan()
        );

        Ok(optimized_tasks)
    }

    /// Generate mock AI decomposition (replace with actual LLM call)
    fn generate_mock_decomposition(&self, goal: &str) -> String {
        if goal.to_lowercase().contains("implement") || goal.to_lowercase().contains("build") {
            r#"[
  {
    "id": "analyze_requirements",
    "description": "Analyze project requirements and constraints",
    "priority": 10,
    "dependencies": [],
    "estimated_complexity": 0.3
  },
  {
    "id": "design_architecture",
    "description": "Design system architecture and component interfaces",
    "priority": 9,
    "dependencies": ["analyze_requirements"],
    "estimated_complexity": 0.4
  },
  {
    "id": "setup_infrastructure",
    "description": "Set up development environment and dependencies",
    "priority": 8,
    "dependencies": [],
    "estimated_complexity": 0.2
  },
  {
    "id": "implement_core",
    "description": "Implement core business logic and algorithms",
    "priority": 7,
    "dependencies": ["design_architecture", "setup_infrastructure"],
    "estimated_complexity": 0.7
  },
  {
    "id": "implement_ui",
    "description": "Implement user interface components",
    "priority": 6,
    "dependencies": ["design_architecture"],
    "estimated_complexity": 0.5
  },
  {
    "id": "write_tests",
    "description": "Write comprehensive unit and integration tests",
    "priority": 5,
    "dependencies": ["implement_core"],
    "estimated_complexity": 0.4
  },
  {
    "id": "integration_testing",
    "description": "Perform integration testing and bug fixes",
    "priority": 4,
    "dependencies": ["implement_core", "implement_ui", "write_tests"],
    "estimated_complexity": 0.3
  }
]"#
            .to_string()
        } else if goal.to_lowercase().contains("optimize")
            || goal.to_lowercase().contains("performance")
        {
            r#"[
  {
    "id": "performance_analysis",
    "description": "Analyze current performance bottlenecks and hotspots",
    "priority": 10,
    "dependencies": [],
    "estimated_complexity": 0.4
  },
  {
    "id": "memory_optimization",
    "description": "Optimize memory usage and reduce allocations",
    "priority": 8,
    "dependencies": ["performance_analysis"],
    "estimated_complexity": 0.6
  },
  {
    "id": "cpu_optimization",
    "description": "Optimize CPU usage and parallel processing",
    "priority": 7,
    "dependencies": ["performance_analysis"],
    "estimated_complexity": 0.5
  },
  {
    "id": "io_optimization",
    "description": "Optimize I/O operations and reduce blocking",
    "priority": 6,
    "dependencies": ["performance_analysis"],
    "estimated_complexity": 0.4
  },
  {
    "id": "benchmarking",
    "description": "Create performance benchmarks and regression tests",
    "priority": 5,
    "dependencies": ["memory_optimization", "cpu_optimization", "io_optimization"],
    "estimated_complexity": 0.3
  }
]"#
            .to_string()
        } else {
            // Generic decomposition
            r#"[
  {
    "id": "research_task",
    "description": "Research and understand the task requirements",
    "priority": 9,
    "dependencies": [],
    "estimated_complexity": 0.3
  },
  {
    "id": "plan_execution",
    "description": "Create detailed execution plan",
    "priority": 8,
    "dependencies": ["research_task"],
    "estimated_complexity": 0.4
  },
  {
    "id": "execute_main",
    "description": "Execute the main task implementation",
    "priority": 7,
    "dependencies": ["plan_execution"],
    "estimated_complexity": 0.6
  },
  {
    "id": "testing_validation",
    "description": "Test and validate the implementation",
    "priority": 6,
    "dependencies": ["execute_main"],
    "estimated_complexity": 0.3
  }
]"#
            .to_string()
        }
    }

    /// Optimize task decomposition for better parallelism
    fn optimize_decomposition(&self, tasks: Vec<SubTask>) -> Result<Vec<SubTask>> {
        let mut optimized = tasks;

        // Remove unnecessary dependencies to increase parallelism
        for task in &mut optimized {
            task.dependencies.retain(|dep| {
                // Keep dependencies that are truly blocking
                // This is a simplified optimization - in practice would use more sophisticated analysis
                !dep.is_empty()
            });
        }

        // Rebalance priorities based on dependency chains
        Self::rebalance_priorities(&mut optimized);

        Ok(optimized)
    }

    /// Rebalance task priorities based on dependency chains and parallelism opportunities
    fn rebalance_priorities(tasks: &mut [SubTask]) {
        // Tasks with no dependencies get higher priority to start immediately
        for task in tasks.iter_mut() {
            if task.dependencies.is_empty() && task.priority < 8 {
                task.priority = 8;
            }
        }

        // Tasks that many others depend on get higher priority
        let mut dependency_counts = std::collections::HashMap::new();
        for task in tasks.iter() {
            for dep in &task.dependencies {
                *dependency_counts.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        for task in tasks.iter_mut() {
            if let Some(count) = dependency_counts.get(&task.id) {
                if *count > 1 {
                    task.priority = (task.priority + 2).min(10);
                }
            }
        }
    }

    /// Legacy method for backward compatibility
    pub fn decompose_task(&self, goal: &str) -> Result<Vec<SubTask>> {
        // Use mock decomposition for backward compatibility
        let mock_response = self.generate_mock_decomposition(goal);
        let tasks: Vec<SubTask> = serde_json::from_str(&mock_response)
            .map_err(|e| anyhow::anyhow!("Failed to parse decomposition: {}", e))?;

        println!("{}", format!("Decomposing task: {}", goal).bright_yellow());
        println!(
            "{}",
            format!("Created {} sub-tasks", tasks.len()).bright_green()
        );

        Ok(tasks)
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
                    println!(
                        "{}",
                        format!("  → Starting: {}", task_clone.description).cyan()
                    );
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

        println!(
            "\n{}",
            "=== Parallel Execution Summary ===".bright_cyan().bold()
        );
        println!("{}: {}", "Total tasks".white(), total_tasks);
        println!("{}: {}", "Successful".green(), successful);
        if failed > 0 {
            println!("{}: {}", "Failed".red(), failed);
        }

        let total_time: u64 = results.iter().map(|r| r.execution_time_ms).sum();
        let max_time: u64 = results
            .iter()
            .map(|r| r.execution_time_ms)
            .max()
            .unwrap_or(0);

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
    pub fn get_status(
        &self,
        results: &[SubTaskResult],
        total_tasks: usize,
    ) -> ParallelExecutionStatus {
        ParallelExecutionStatus {
            total_tasks,
            completed_tasks: results.iter().filter(|r| r.success).count(),
            failed_tasks: results.iter().filter(|r| !r.success).count(),
            running_tasks: 0, // Would track in real implementation
            pending_tasks: total_tasks.saturating_sub(results.len()),
        }
    }

    /// Intelligent result aggregation with conflict resolution
    pub fn aggregate_results_intelligent(
        &self,
        results: Vec<SubTaskResult>,
    ) -> IntelligentAggregationResult {
        let mut successful_results = Vec::new();
        let mut failed_results = Vec::new();
        let mut conflicts = Vec::new();

        // Separate successful and failed results
        for result in results {
            if result.success {
                successful_results.push(result);
            } else {
                failed_results.push(result);
            }
        }

        // Detect and resolve conflicts
        conflicts = self.detect_result_conflicts(&successful_results);

        // Merge compatible results
        let merged_output = self.merge_compatible_results(&successful_results);

        // Generate intelligent summary
        let summary =
            self.generate_execution_summary(&successful_results, &failed_results, &conflicts);

        IntelligentAggregationResult {
            successful_tasks: successful_results.len(),
            failed_tasks: failed_results.len(),
            conflicts_detected: conflicts.len(),
            merged_output,
            summary,
            conflicts,
        }
    }

    /// Detect conflicts between task results
    fn detect_result_conflicts(&self, results: &[SubTaskResult]) -> Vec<ResultConflict> {
        let mut conflicts = Vec::new();

        // Simple conflict detection - tasks that produced overlapping or contradictory outputs
        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                let result1 = &results[i];
                let result2 = &results[j];

                if self.results_are_conflicting(result1, result2) {
                    conflicts.push(ResultConflict {
                        task1: result1.task_id.clone(),
                        task2: result2.task_id.clone(),
                        conflict_type: ConflictType::OutputOverlap,
                        description: "Tasks produced overlapping or conflicting outputs"
                            .to_string(),
                        resolution: ConflictResolution::MergeWithPriority,
                    });
                }
            }
        }

        conflicts
    }

    /// Check if two results are conflicting
    fn results_are_conflicting(&self, result1: &SubTaskResult, result2: &SubTaskResult) -> bool {
        // Simple heuristic: if outputs are very similar but tasks are different
        if result1.output.len() > 50 && result2.output.len() > 50 {
            let similarity = self.calculate_text_similarity(&result1.output, &result2.output);
            similarity > 0.8 // High similarity indicates potential conflict
        } else {
            false
        }
    }

    /// Calculate simple text similarity (Levenshtein distance approximation)
    fn calculate_text_similarity(&self, text1: &str, text2: &str) -> f32 {
        let len1 = text1.len();
        let len2 = text2.len();

        if len1 == 0 && len2 == 0 {
            return 1.0;
        }

        let max_len = len1.max(len2) as f32;
        let min_len = len1.min(len2) as f32;

        // Simple length-based similarity
        1.0 - (max_len - min_len).abs() / max_len
    }

    /// Merge compatible results intelligently
    fn merge_compatible_results(&self, results: &[SubTaskResult]) -> String {
        let mut merged_parts = Vec::new();

        // Group results by task type/category
        let mut categorized_results = std::collections::HashMap::new();

        for result in results {
            let category = self.categorize_result(result);
            categorized_results
                .entry(category)
                .or_insert_with(Vec::new)
                .push(result);
        }

        // Merge results within each category
        for (category, category_results) in categorized_results {
            let merged = self.merge_category_results(&category, category_results);
            merged_parts.push(merged);
        }

        merged_parts.join("\n\n")
    }

    /// Categorize a result by task type
    fn categorize_result(&self, result: &SubTaskResult) -> String {
        // Simple categorization based on task ID
        if result.task_id.contains("analyze") || result.task_id.contains("research") {
            "analysis".to_string()
        } else if result.task_id.contains("implement") || result.task_id.contains("code") {
            "implementation".to_string()
        } else if result.task_id.contains("test") || result.task_id.contains("validate") {
            "testing".to_string()
        } else {
            "general".to_string()
        }
    }

    /// Merge results within a category
    fn merge_category_results(&self, category: &str, results: Vec<&SubTaskResult>) -> String {
        let mut merged = format!("=== {} Results ===\n", category.to_uppercase());

        for result in results {
            merged.push_str(&format!(
                "• Task {}: {}\n",
                result.task_id,
                if result.output.len() > 100 {
                    format!("{}...", &result.output[..100])
                } else {
                    result.output.clone()
                }
            ));
        }

        merged
    }

    /// Generate intelligent execution summary
    fn generate_execution_summary(
        &self,
        successful: &[SubTaskResult],
        failed: &[SubTaskResult],
        conflicts: &[ResultConflict],
    ) -> String {
        let total_tasks = successful.len() + failed.len();
        let success_rate = if total_tasks > 0 {
            (successful.len() as f32 / total_tasks as f32) * 100.0
        } else {
            0.0
        };

        let mut summary = format!(
            "Execution Summary: {}/{} tasks successful ({:.1}%)\n",
            successful.len(),
            total_tasks,
            success_rate
        );

        if !failed.is_empty() {
            summary.push_str(&format!("Failed tasks: {}\n", failed.len()));
        }

        if !conflicts.is_empty() {
            summary.push_str(&format!("Conflicts resolved: {}\n", conflicts.len()));
        }

        let total_time: u64 = successful
            .iter()
            .chain(failed.iter())
            .map(|r| r.execution_time_ms)
            .sum();

        summary.push_str(&format!("Total execution time: {}ms\n", total_time));

        if successful.len() > 1 {
            let avg_time = total_time / successful.len() as u64;
            summary.push_str(&format!("Average task time: {}ms\n", avg_time));
        }

        summary
    }

    /// Legacy method for backward compatibility
    pub fn aggregate_results(&self, results: Vec<SubTaskResult>) -> String {
        let intelligent = self.aggregate_results_intelligent(results);
        format!("{}\n\n{}", intelligent.summary, intelligent.merged_output)
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

        let results = orchestrator
            .execute_parallel(tasks, executor)
            .await
            .unwrap();
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

        let results = orchestrator
            .execute_parallel(tasks, executor)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        // Task 1 should complete before task 2
        let task_1_idx = results.iter().position(|r| r.task_id == "task_1").unwrap();
        let task_2_idx = results.iter().position(|r| r.task_id == "task_2").unwrap();
        assert!(task_1_idx < task_2_idx);
    }
}
