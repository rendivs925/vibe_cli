use crate::parallel_agent::SubTask;
use serde::{Deserialize, Serialize};
use shared::types::Result;

/// Strategy for decomposing tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecompositionStrategy {
    /// Split by file/module boundaries
    ByFile,
    /// Split by functional requirements
    ByFeature,
    /// Split by dependency layers
    ByLayer,
    /// AI-powered intelligent decomposition
    Intelligent,
    /// Hybrid approach combining multiple strategies
    Hybrid,
}

/// Task complexity analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskComplexity {
    pub estimated_lines_of_code: usize,
    pub file_count: usize,
    pub dependency_depth: usize,
    pub risk_level: f32,        // 0.0 - 1.0
    pub parallelizability: f32, // 0.0 - 1.0 (1.0 = highly parallelizable)
}

/// Intelligent task decomposer using AI and heuristics
pub struct TaskDecomposer {
    strategy: DecompositionStrategy,
    max_subtasks: usize,
    min_task_complexity: f32,
}

impl TaskDecomposer {
    /// Create a new task decomposer
    pub fn new(strategy: DecompositionStrategy) -> Self {
        Self {
            strategy,
            max_subtasks: 10,
            min_task_complexity: 0.1,
        }
    }

    /// Set maximum number of subtasks
    pub fn with_max_subtasks(mut self, max: usize) -> Self {
        self.max_subtasks = max;
        self
    }

    /// Set minimum task complexity threshold
    pub fn with_min_complexity(mut self, min: f32) -> Self {
        self.min_task_complexity = min;
        self
    }

    /// Analyze task complexity
    pub fn analyze_complexity(&self, goal: &str) -> TaskComplexity {
        // Heuristic-based complexity analysis
        let word_count = goal.split_whitespace().count();
        let has_multiple_actions = goal.contains(" and ") || goal.contains(",");
        let has_file_operations = goal.to_lowercase().contains("file")
            || goal.to_lowercase().contains("create")
            || goal.to_lowercase().contains("modify");

        let estimated_lines = if word_count < 10 {
            50
        } else if word_count < 20 {
            150
        } else {
            300
        };

        let file_count = if has_multiple_actions { 3 } else { 1 };

        let parallelizability = if has_multiple_actions { 0.8 } else { 0.3 };

        TaskComplexity {
            estimated_lines_of_code: estimated_lines,
            file_count,
            dependency_depth: if has_file_operations { 2 } else { 1 },
            risk_level: if has_file_operations { 0.6 } else { 0.3 },
            parallelizability,
        }
    }

    /// Decompose a complex goal into parallel subtasks
    pub fn decompose(&self, goal: &str) -> Result<Vec<SubTask>> {
        let complexity = self.analyze_complexity(goal);

        match self.strategy {
            DecompositionStrategy::ByFile => self.decompose_by_file(goal, &complexity),
            DecompositionStrategy::ByFeature => self.decompose_by_feature(goal, &complexity),
            DecompositionStrategy::ByLayer => self.decompose_by_layer(goal, &complexity),
            DecompositionStrategy::Intelligent => self.decompose_intelligent(goal, &complexity),
            DecompositionStrategy::Hybrid => self.decompose_hybrid(goal, &complexity),
        }
    }

    /// Decompose by file boundaries
    fn decompose_by_file(&self, goal: &str, complexity: &TaskComplexity) -> Result<Vec<SubTask>> {
        let mut tasks = Vec::new();

        // Analysis phase
        tasks.push(SubTask {
            id: "file_analysis".to_string(),
            description: format!("Analyze files affected by: {}", goal),
            priority: 10,
            dependencies: vec![],
            estimated_complexity: 0.2,
        });

        // File-specific modifications
        for i in 0..complexity.file_count.min(self.max_subtasks - 2) {
            tasks.push(SubTask {
                id: format!("modify_file_{}", i + 1),
                description: format!("Modify file {} for: {}", i + 1, goal),
                priority: 8 - i as u8,
                dependencies: vec!["file_analysis".to_string()],
                estimated_complexity: 0.5,
            });
        }

        // Integration phase
        tasks.push(SubTask {
            id: "integration".to_string(),
            description: "Integrate all file changes".to_string(),
            priority: 5,
            dependencies: tasks
                .iter()
                .filter(|t| t.id.starts_with("modify_file_"))
                .map(|t| t.id.clone())
                .collect(),
            estimated_complexity: 0.3,
        });

        Ok(tasks)
    }

    /// Decompose by functional features
    fn decompose_by_feature(
        &self,
        goal: &str,
        _complexity: &TaskComplexity,
    ) -> Result<Vec<SubTask>> {
        let mut tasks = Vec::new();

        // Requirements analysis
        tasks.push(SubTask {
            id: "requirements".to_string(),
            description: format!("Analyze requirements for: {}", goal),
            priority: 10,
            dependencies: vec![],
            estimated_complexity: 0.2,
        });

        // Core functionality
        tasks.push(SubTask {
            id: "core_logic".to_string(),
            description: "Implement core business logic".to_string(),
            priority: 9,
            dependencies: vec!["requirements".to_string()],
            estimated_complexity: 0.6,
        });

        // User interface
        tasks.push(SubTask {
            id: "ui_layer".to_string(),
            description: "Implement user interface layer".to_string(),
            priority: 7,
            dependencies: vec!["requirements".to_string()],
            estimated_complexity: 0.4,
        });

        // Integration
        tasks.push(SubTask {
            id: "integration".to_string(),
            description: "Integrate UI with core logic".to_string(),
            priority: 6,
            dependencies: vec!["core_logic".to_string(), "ui_layer".to_string()],
            estimated_complexity: 0.3,
        });

        // Testing
        tasks.push(SubTask {
            id: "testing".to_string(),
            description: "Write tests for new functionality".to_string(),
            priority: 8,
            dependencies: vec!["integration".to_string()],
            estimated_complexity: 0.4,
        });

        Ok(tasks)
    }

    /// Decompose by architectural layers
    fn decompose_by_layer(&self, goal: &str, _complexity: &TaskComplexity) -> Result<Vec<SubTask>> {
        let mut tasks = Vec::new();

        tasks.push(SubTask {
            id: "domain_layer".to_string(),
            description: format!("Implement domain models for: {}", goal),
            priority: 10,
            dependencies: vec![],
            estimated_complexity: 0.4,
        });

        tasks.push(SubTask {
            id: "application_layer".to_string(),
            description: "Implement application services".to_string(),
            priority: 9,
            dependencies: vec!["domain_layer".to_string()],
            estimated_complexity: 0.5,
        });

        tasks.push(SubTask {
            id: "infrastructure_layer".to_string(),
            description: "Implement infrastructure components".to_string(),
            priority: 8,
            dependencies: vec!["domain_layer".to_string()],
            estimated_complexity: 0.5,
        });

        tasks.push(SubTask {
            id: "presentation_layer".to_string(),
            description: "Implement presentation/CLI layer".to_string(),
            priority: 7,
            dependencies: vec!["application_layer".to_string()],
            estimated_complexity: 0.4,
        });

        Ok(tasks)
    }

    /// Intelligent AI-powered decomposition
    fn decompose_intelligent(
        &self,
        goal: &str,
        complexity: &TaskComplexity,
    ) -> Result<Vec<SubTask>> {
        // Placeholder for AI-powered decomposition
        // In production, this would call an LLM to intelligently decompose the task

        println!("Using intelligent decomposition for: {}", goal);
        println!("  Estimated complexity: {:.2}", complexity.risk_level);
        println!("  Parallelizability: {:.2}", complexity.parallelizability);

        // For now, use a smart heuristic-based approach
        if complexity.parallelizability > 0.7 {
            // Highly parallelizable - use feature-based decomposition
            self.decompose_by_feature(goal, complexity)
        } else if complexity.file_count > 2 {
            // Multiple files - use file-based decomposition
            self.decompose_by_file(goal, complexity)
        } else {
            // Simple task - use layer-based decomposition
            self.decompose_by_layer(goal, complexity)
        }
    }

    /// Hybrid decomposition combining multiple strategies
    fn decompose_hybrid(&self, goal: &str, complexity: &TaskComplexity) -> Result<Vec<SubTask>> {
        let mut all_tasks = Vec::new();

        // Phase 1: Analysis (always needed)
        all_tasks.push(SubTask {
            id: "analysis".to_string(),
            description: format!("Comprehensive analysis of: {}", goal),
            priority: 10,
            dependencies: vec![],
            estimated_complexity: 0.2,
        });

        // Phase 2: Parallel implementation based on complexity
        if complexity.file_count > 1 {
            // File-based tasks
            for i in 0..complexity.file_count.min(3) {
                all_tasks.push(SubTask {
                    id: format!("impl_file_{}", i + 1),
                    description: format!("Implement changes in file group {}", i + 1),
                    priority: 9 - i as u8,
                    dependencies: vec!["analysis".to_string()],
                    estimated_complexity: 0.5,
                });
            }
        } else {
            // Layer-based tasks for single file
            all_tasks.push(SubTask {
                id: "impl_core".to_string(),
                description: "Implement core functionality".to_string(),
                priority: 9,
                dependencies: vec!["analysis".to_string()],
                estimated_complexity: 0.6,
            });
        }

        // Phase 3: Integration and testing
        let impl_tasks: Vec<String> = all_tasks
            .iter()
            .filter(|t| t.id.starts_with("impl_"))
            .map(|t| t.id.clone())
            .collect();

        all_tasks.push(SubTask {
            id: "integration".to_string(),
            description: "Integrate all components".to_string(),
            priority: 7,
            dependencies: impl_tasks.clone(),
            estimated_complexity: 0.3,
        });

        all_tasks.push(SubTask {
            id: "testing".to_string(),
            description: "Comprehensive testing".to_string(),
            priority: 8,
            dependencies: vec!["integration".to_string()],
            estimated_complexity: 0.4,
        });

        // Limit to max_subtasks
        if all_tasks.len() > self.max_subtasks {
            all_tasks.truncate(self.max_subtasks);
        }

        Ok(all_tasks)
    }

    /// Optimize task dependencies to maximize parallelism
    pub fn optimize_dependencies(&self, tasks: &mut [SubTask]) {
        // Build a map of task IDs to their dependencies first
        let dep_map: std::collections::HashMap<String, Vec<String>> = tasks
            .iter()
            .map(|t| (t.id.clone(), t.dependencies.clone()))
            .collect();

        // Remove redundant dependencies
        for task in tasks.iter_mut() {
            let mut optimized_deps = task.dependencies.clone();

            // Remove transitive dependencies
            for dep in &task.dependencies {
                if let Some(dep_dependencies) = dep_map.get(dep) {
                    for transitive_dep in dep_dependencies {
                        optimized_deps.retain(|d| d != transitive_dep);
                    }
                }
            }

            task.dependencies = optimized_deps;
        }
    }

    /// Calculate critical path through task graph
    pub fn calculate_critical_path(&self, tasks: &[SubTask]) -> Vec<String> {
        let mut critical_path = Vec::new();
        let mut current_tasks: Vec<&SubTask> =
            tasks.iter().filter(|t| t.dependencies.is_empty()).collect();

        while !current_tasks.is_empty() {
            // Find highest priority task
            if let Some(task) = current_tasks.iter().max_by_key(|t| t.priority) {
                critical_path.push(task.id.clone());

                // Find next tasks that depend on this one
                let task_id = task.id.clone();
                current_tasks = tasks
                    .iter()
                    .filter(|t| t.dependencies.contains(&task_id))
                    .collect();
            } else {
                break;
            }
        }

        critical_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_analysis() {
        let decomposer = TaskDecomposer::new(DecompositionStrategy::Intelligent);

        let simple = decomposer.analyze_complexity("Add a function");
        assert!(simple.estimated_lines_of_code < 100);
        assert!(simple.parallelizability < 0.5);

        let complex = decomposer
            .analyze_complexity("Create a new module with multiple files and database integration");
        assert!(complex.estimated_lines_of_code > 100);
        assert!(complex.file_count > 1);
    }

    #[test]
    fn test_decompose_by_file() {
        let decomposer = TaskDecomposer::new(DecompositionStrategy::ByFile);
        let tasks = decomposer
            .decompose("Implement user authentication")
            .unwrap();

        assert!(!tasks.is_empty());
        assert!(tasks.iter().any(|t| t.id.contains("analysis")));
        assert!(tasks.iter().any(|t| t.id.contains("integration")));
    }

    #[test]
    fn test_decompose_by_feature() {
        let decomposer = TaskDecomposer::new(DecompositionStrategy::ByFeature);
        let tasks = decomposer.decompose("Add user dashboard").unwrap();

        assert!(!tasks.is_empty());
        assert!(tasks.iter().any(|t| t.description.contains("requirements")));
        assert!(tasks.iter().any(|t| t.id == "testing"));
    }

    #[test]
    fn test_optimize_dependencies() {
        let decomposer = TaskDecomposer::new(DecompositionStrategy::Hybrid);
        let mut tasks = vec![
            SubTask {
                id: "a".to_string(),
                description: "Task A".to_string(),
                priority: 10,
                dependencies: vec![],
                estimated_complexity: 0.5,
            },
            SubTask {
                id: "b".to_string(),
                description: "Task B".to_string(),
                priority: 9,
                dependencies: vec!["a".to_string()],
                estimated_complexity: 0.5,
            },
            SubTask {
                id: "c".to_string(),
                description: "Task C".to_string(),
                priority: 8,
                dependencies: vec!["a".to_string(), "b".to_string()], // 'a' is redundant
                estimated_complexity: 0.5,
            },
        ];

        decomposer.optimize_dependencies(&mut tasks);

        // Task C should only depend on B (A is transitive through B)
        assert_eq!(tasks[2].dependencies.len(), 1);
        assert_eq!(tasks[2].dependencies[0], "b");
    }

    #[test]
    fn test_critical_path() {
        let decomposer = TaskDecomposer::new(DecompositionStrategy::Hybrid);
        let tasks = vec![
            SubTask {
                id: "a".to_string(),
                description: "Task A".to_string(),
                priority: 10,
                dependencies: vec![],
                estimated_complexity: 0.5,
            },
            SubTask {
                id: "b".to_string(),
                description: "Task B".to_string(),
                priority: 9,
                dependencies: vec!["a".to_string()],
                estimated_complexity: 0.5,
            },
            SubTask {
                id: "c".to_string(),
                description: "Task C".to_string(),
                priority: 8,
                dependencies: vec!["b".to_string()],
                estimated_complexity: 0.5,
            },
        ];

        let path = decomposer.calculate_critical_path(&tasks);
        assert_eq!(path, vec!["a", "b", "c"]);
    }
}
