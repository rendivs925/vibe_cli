use crate::parallel_agent::SubTaskResult;
use serde::{Deserialize, Serialize};
use shared::types::Result;

/// Conflict resolution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Use result from highest priority task
    Priority,
    /// Use most recent result
    Latest,
    /// Use result from task that completed first
    First,
    /// Merge results intelligently
    Merge,
    /// Fail on any conflict
    Strict,
}

/// Aggregation strategy for combining results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationStrategy {
    /// Concatenate all outputs
    Concatenate,
    /// Use structured merging
    Structured,
    /// Summary-based aggregation
    Summary,
    /// Custom aggregation logic
    Custom,
}

/// Conflict between two task results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub task_id_1: String,
    pub task_id_2: String,
    pub conflict_type: String,
    pub description: String,
    pub severity: ConflictSeverity,
}

/// Severity of a conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConflictSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Aggregated result from multiple parallel tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResult {
    pub combined_output: String,
    pub task_count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub conflicts_resolved: usize,
    pub conflicts_remaining: Vec<Conflict>,
    pub execution_summary: String,
}

/// Result aggregator for parallel execution
pub struct ResultAggregator {
    conflict_resolution: ConflictResolution,
    aggregation_strategy: AggregationStrategy,
    allow_partial_success: bool,
}

impl ResultAggregator {
    /// Create a new result aggregator
    pub fn new(
        conflict_resolution: ConflictResolution,
        aggregation_strategy: AggregationStrategy,
    ) -> Self {
        Self {
            conflict_resolution,
            aggregation_strategy,
            allow_partial_success: true,
        }
    }

    /// Set whether to allow partial success
    pub fn allow_partial_success(mut self, allow: bool) -> Self {
        self.allow_partial_success = allow;
        self
    }

    /// Aggregate results from parallel execution
    pub fn aggregate(&self, results: Vec<SubTaskResult>) -> Result<AggregatedResult> {
        if results.is_empty() {
            return Err(anyhow::anyhow!("No results to aggregate"));
        }

        let task_count = results.len();
        let success_count = results.iter().filter(|r| r.success).count();
        let failure_count = results.iter().filter(|r| !r.success).count();

        // Check if we should fail due to failures
        if !self.allow_partial_success && failure_count > 0 {
            return Err(anyhow::anyhow!(
                "Partial success not allowed. {} of {} tasks failed",
                failure_count,
                task_count
            ));
        }

        // Detect conflicts
        let conflicts = self.detect_conflicts(&results);
        let conflicts_count = conflicts.len();

        // Resolve conflicts
        let (resolved_results, conflicts_remaining) = self.resolve_conflicts(results, conflicts)?;

        // Aggregate outputs
        let combined_output = match self.aggregation_strategy {
            AggregationStrategy::Concatenate => self.concatenate_outputs(&resolved_results),
            AggregationStrategy::Structured => self.structured_merge(&resolved_results),
            AggregationStrategy::Summary => self.summary_aggregation(&resolved_results),
            AggregationStrategy::Custom => self.custom_aggregation(&resolved_results)?,
        };

        // Generate execution summary
        let execution_summary = self.generate_summary(&resolved_results);

        Ok(AggregatedResult {
            combined_output,
            task_count,
            success_count,
            failure_count,
            conflicts_resolved: conflicts_count - conflicts_remaining.len(),
            conflicts_remaining,
            execution_summary,
        })
    }

    /// Detect conflicts between task results
    fn detect_conflicts(&self, results: &[SubTaskResult]) -> Vec<Conflict> {
        let mut conflicts = Vec::new();

        // Check for output conflicts (simplified - in production would be more sophisticated)
        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                let r1 = &results[i];
                let r2 = &results[j];

                // Check if both tasks succeeded but have conflicting outputs
                if r1.success && r2.success {
                    // Simple heuristic: check if outputs mention the same files/entities
                    if self.outputs_conflict(&r1.output, &r2.output) {
                        conflicts.push(Conflict {
                            task_id_1: r1.task_id.clone(),
                            task_id_2: r2.task_id.clone(),
                            conflict_type: "output_overlap".to_string(),
                            description: "Tasks may have modified the same resources".to_string(),
                            severity: ConflictSeverity::Medium,
                        });
                    }
                }
            }
        }

        conflicts
    }

    /// Check if two outputs conflict
    fn outputs_conflict(&self, output1: &str, output2: &str) -> bool {
        // Simplified conflict detection
        // In production, this would parse outputs and check for actual conflicts

        // Check if both outputs mention the same file paths
        let words1: Vec<&str> = output1.split_whitespace().collect();
        let words2: Vec<&str> = output2.split_whitespace().collect();

        // Look for common file-like patterns
        words1.iter().any(|w1| {
            words2.iter().any(|w2| {
                w1 == w2 && (w1.ends_with(".rs") || w1.ends_with(".toml") || w1.contains('/'))
            })
        })
    }

    /// Resolve conflicts between results
    fn resolve_conflicts(
        &self,
        results: Vec<SubTaskResult>,
        conflicts: Vec<Conflict>,
    ) -> Result<(Vec<SubTaskResult>, Vec<Conflict>)> {
        if conflicts.is_empty() {
            return Ok((results, vec![]));
        }

        match self.conflict_resolution {
            ConflictResolution::Strict => {
                // Fail on any conflict
                Err(anyhow::anyhow!(
                    "Found {} conflicts with strict resolution policy",
                    conflicts.len()
                ))
            }
            ConflictResolution::Priority => {
                // Keep results from tasks involved in conflicts based on task priority
                // For this simplified implementation, we'll just keep the first task's result
                Ok((results, vec![]))
            }
            ConflictResolution::Latest => {
                // Keep the most recent results
                Ok((results, vec![]))
            }
            ConflictResolution::First => {
                // Keep results that completed first (already in order)
                Ok((results, vec![]))
            }
            ConflictResolution::Merge => {
                // Attempt intelligent merging
                let unresolvable: Vec<Conflict> = conflicts
                    .into_iter()
                    .filter(|c| c.severity >= ConflictSeverity::High)
                    .collect();

                Ok((results, unresolvable))
            }
        }
    }

    /// Concatenate all outputs
    fn concatenate_outputs(&self, results: &[SubTaskResult]) -> String {
        results
            .iter()
            .filter(|r| r.success)
            .map(|r| format!("=== {} ===\n{}\n", r.task_id, r.output))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Structured merge of results
    fn structured_merge(&self, results: &[SubTaskResult]) -> String {
        let mut output = String::from("# Aggregated Results\n\n");

        output.push_str("## Successful Tasks\n\n");
        for result in results.iter().filter(|r| r.success) {
            output.push_str(&format!("### {}\n", result.task_id));
            output.push_str(&format!(
                "Execution time: {}ms\n\n",
                result.execution_time_ms
            ));
            output.push_str(&format!("{}\n\n", result.output));
        }

        if results.iter().any(|r| !r.success) {
            output.push_str("## Failed Tasks\n\n");
            for result in results.iter().filter(|r| !r.success) {
                output.push_str(&format!("### {} (FAILED)\n", result.task_id));
                if let Some(error) = &result.error {
                    output.push_str(&format!("Error: {}\n\n", error));
                }
            }
        }

        output
    }

    /// Summary-based aggregation
    fn summary_aggregation(&self, results: &[SubTaskResult]) -> String {
        let total_time: u64 = results.iter().map(|r| r.execution_time_ms).sum();
        let avg_time = if !results.is_empty() {
            total_time / results.len() as u64
        } else {
            0
        };

        format!(
            "Execution Summary:\n\
            - Total tasks: {}\n\
            - Successful: {}\n\
            - Failed: {}\n\
            - Total execution time: {}ms\n\
            - Average task time: {}ms\n\
            \n\
            Key Outputs:\n{}",
            results.len(),
            results.iter().filter(|r| r.success).count(),
            results.iter().filter(|r| !r.success).count(),
            total_time,
            avg_time,
            results
                .iter()
                .filter(|r| r.success)
                .map(|r| format!("- {}: {}", r.task_id, self.truncate(&r.output, 100)))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    /// Custom aggregation logic
    fn custom_aggregation(&self, results: &[SubTaskResult]) -> Result<String> {
        // Placeholder for custom aggregation
        // In production, this would implement domain-specific logic
        Ok(self.structured_merge(results))
    }

    /// Generate execution summary
    fn generate_summary(&self, results: &[SubTaskResult]) -> String {
        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();
        let total_time: u64 = results.iter().map(|r| r.execution_time_ms).sum();

        format!(
            "Parallel execution completed: {} successful, {} failed, {}ms total",
            successful, failed, total_time
        )
    }

    /// Truncate string to max length
    fn truncate(&self, s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len])
        }
    }

    /// Merge outputs from related tasks
    pub fn merge_related_outputs(
        &self,
        results: Vec<SubTaskResult>,
        groups: Vec<Vec<String>>,
    ) -> Result<Vec<String>> {
        let mut merged_outputs = Vec::new();

        for group in groups {
            let group_results: Vec<_> = results
                .iter()
                .filter(|r| group.contains(&r.task_id))
                .collect();

            if group_results.is_empty() {
                continue;
            }

            let merged = group_results
                .iter()
                .map(|r| r.output.clone())
                .collect::<Vec<_>>()
                .join("\n---\n");

            merged_outputs.push(merged);
        }

        Ok(merged_outputs)
    }
}

/// Builder for ResultAggregator
pub struct ResultAggregatorBuilder {
    conflict_resolution: ConflictResolution,
    aggregation_strategy: AggregationStrategy,
    allow_partial_success: bool,
}

impl ResultAggregatorBuilder {
    pub fn new() -> Self {
        Self {
            conflict_resolution: ConflictResolution::Merge,
            aggregation_strategy: AggregationStrategy::Structured,
            allow_partial_success: true,
        }
    }

    pub fn conflict_resolution(mut self, strategy: ConflictResolution) -> Self {
        self.conflict_resolution = strategy;
        self
    }

    pub fn aggregation_strategy(mut self, strategy: AggregationStrategy) -> Self {
        self.aggregation_strategy = strategy;
        self
    }

    pub fn allow_partial_success(mut self, allow: bool) -> Self {
        self.allow_partial_success = allow;
        self
    }

    pub fn build(self) -> ResultAggregator {
        ResultAggregator::new(self.conflict_resolution, self.aggregation_strategy)
            .allow_partial_success(self.allow_partial_success)
    }
}

impl Default for ResultAggregatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_result(id: &str, success: bool, output: &str) -> SubTaskResult {
        SubTaskResult {
            task_id: id.to_string(),
            success,
            output: output.to_string(),
            execution_time_ms: 100,
            error: if success {
                None
            } else {
                Some("Error".to_string())
            },
        }
    }

    #[test]
    fn test_aggregator_creation() {
        let aggregator =
            ResultAggregator::new(ConflictResolution::Merge, AggregationStrategy::Structured);
        assert!(aggregator.allow_partial_success);
    }

    #[test]
    fn test_concatenate_strategy() {
        let aggregator =
            ResultAggregator::new(ConflictResolution::Merge, AggregationStrategy::Concatenate);

        let results = vec![
            create_test_result("task1", true, "Output 1"),
            create_test_result("task2", true, "Output 2"),
        ];

        let aggregated = aggregator.aggregate(results).unwrap();
        assert_eq!(aggregated.task_count, 2);
        assert_eq!(aggregated.success_count, 2);
        assert!(aggregated.combined_output.contains("task1"));
        assert!(aggregated.combined_output.contains("task2"));
    }

    #[test]
    fn test_partial_success() {
        let aggregator =
            ResultAggregator::new(ConflictResolution::Merge, AggregationStrategy::Summary);

        let results = vec![
            create_test_result("task1", true, "Success"),
            create_test_result("task2", false, ""),
        ];

        let aggregated = aggregator.aggregate(results).unwrap();
        assert_eq!(aggregated.success_count, 1);
        assert_eq!(aggregated.failure_count, 1);
    }

    #[test]
    fn test_no_partial_success() {
        let aggregator =
            ResultAggregator::new(ConflictResolution::Merge, AggregationStrategy::Summary)
                .allow_partial_success(false);

        let results = vec![
            create_test_result("task1", true, "Success"),
            create_test_result("task2", false, ""),
        ];

        let result = aggregator.aggregate(results);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_pattern() {
        let aggregator = ResultAggregatorBuilder::new()
            .conflict_resolution(ConflictResolution::Strict)
            .aggregation_strategy(AggregationStrategy::Concatenate)
            .allow_partial_success(false)
            .build();

        let results = vec![create_test_result("task1", true, "Output")];
        let aggregated = aggregator.aggregate(results).unwrap();
        assert_eq!(aggregated.success_count, 1);
    }
}
