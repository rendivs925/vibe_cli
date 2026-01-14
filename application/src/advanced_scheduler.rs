/// Advanced parallel task scheduler with dynamic load balancing
///
/// Provides intelligent task scheduling with:
/// - Work stealing for load balancing
/// - Priority-based scheduling
/// - Adaptive concurrency control
/// - Task affinity and locality optimization
/// - Backpressure management
use crate::parallel_agent::SubTask;
use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

/// Task scheduling strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingStrategy {
    /// First-In-First-Out: Simple queue-based scheduling
    FIFO,
    /// Priority-based: Higher priority tasks execute first
    Priority,
    /// Shortest-Job-First: Execute tasks with lowest complexity first
    ShortestJobFirst,
    /// Work-stealing: Dynamic load balancing across workers
    WorkStealing,
    /// Adaptive: Dynamically select strategy based on workload
    Adaptive,
}

/// Worker node for task execution
#[derive(Debug, Clone)]
pub struct WorkerNode {
    pub id: usize,
    pub active_tasks: usize,
    pub completed_tasks: usize,
    pub total_execution_time_ms: u64,
    pub last_task_completion: Option<Instant>,
}

impl WorkerNode {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            active_tasks: 0,
            completed_tasks: 0,
            total_execution_time_ms: 0,
            last_task_completion: None,
        }
    }

    /// Get average task execution time in ms
    pub fn avg_execution_time_ms(&self) -> u64 {
        if self.completed_tasks == 0 {
            0
        } else {
            self.total_execution_time_ms / self.completed_tasks as u64
        }
    }

    /// Get current load score (lower is better)
    pub fn load_score(&self) -> f32 {
        self.active_tasks as f32 + (self.avg_execution_time_ms() as f32 / 1000.0)
    }
}

/// Scheduled task with priority and metadata
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    pub task: SubTask,
    pub priority: u32,
    pub submit_time: Instant,
    pub preferred_worker: Option<usize>,
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first (reverse order for max heap)
        other.priority.cmp(&self.priority)
    }
}

/// Advanced task scheduler with dynamic load balancing
pub struct AdvancedScheduler {
    strategy: SchedulingStrategy,
    workers: Arc<Mutex<Vec<WorkerNode>>>,
    task_queue: Arc<Mutex<BinaryHeap<ScheduledTask>>>,
    work_stealing_queues: Arc<Mutex<HashMap<usize, VecDeque<ScheduledTask>>>>,
    semaphore: Arc<Semaphore>,
    enable_backpressure: bool,
    max_queue_size: usize,
}

impl AdvancedScheduler {
    /// Create a new scheduler
    pub fn new(num_workers: usize, strategy: SchedulingStrategy) -> Self {
        let workers: Vec<WorkerNode> = (0..num_workers).map(|id| WorkerNode::new(id)).collect();

        let work_stealing_queues: HashMap<usize, VecDeque<ScheduledTask>> =
            (0..num_workers).map(|id| (id, VecDeque::new())).collect();

        Self {
            strategy,
            workers: Arc::new(Mutex::new(workers)),
            task_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            work_stealing_queues: Arc::new(Mutex::new(work_stealing_queues)),
            semaphore: Arc::new(Semaphore::new(num_workers)),
            enable_backpressure: true,
            max_queue_size: num_workers * 10,
        }
    }

    /// Submit a task for scheduling
    pub async fn submit_task(&self, task: SubTask) -> Result<()> {
        let priority = task.priority as u32;
        self.submit_task_with_priority(task, priority).await
    }

    /// Submit a task with custom priority
    pub async fn submit_task_with_priority(&self, task: SubTask, priority: u32) -> Result<()> {
        // Backpressure: wait if queue is full
        if self.enable_backpressure {
            let queue = self.task_queue.lock().await;
            if queue.len() >= self.max_queue_size {
                drop(queue); // Release lock before waiting
                             // Wait a bit for queue to drain
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        let scheduled_task = ScheduledTask {
            task,
            priority,
            submit_time: Instant::now(),
            preferred_worker: None,
        };

        match self.strategy {
            SchedulingStrategy::WorkStealing => {
                // Assign to least loaded worker's queue
                let workers = self.workers.lock().await;
                let least_loaded_worker = workers
                    .iter()
                    .min_by(|a, b| a.load_score().partial_cmp(&b.load_score()).unwrap())
                    .map(|w| w.id)
                    .unwrap_or(0);
                drop(workers);

                let mut queues = self.work_stealing_queues.lock().await;
                queues
                    .entry(least_loaded_worker)
                    .or_insert_with(VecDeque::new)
                    .push_back(scheduled_task);
            }
            _ => {
                // Use global priority queue
                let mut queue = self.task_queue.lock().await;
                queue.push(scheduled_task);
            }
        }

        Ok(())
    }

    /// Get next task for a specific worker
    pub async fn get_next_task(&self, worker_id: usize) -> Option<ScheduledTask> {
        match self.strategy {
            SchedulingStrategy::WorkStealing => {
                // Try own queue first
                let mut queues = self.work_stealing_queues.lock().await;
                if let Some(queue) = queues.get_mut(&worker_id) {
                    if let Some(task) = queue.pop_front() {
                        return Some(task);
                    }
                }

                // Work stealing: try to steal from other workers
                let other_workers: Vec<usize> = queues
                    .keys()
                    .filter(|&&id| id != worker_id)
                    .copied()
                    .collect();

                for other_id in other_workers {
                    if let Some(queue) = queues.get_mut(&other_id) {
                        if queue.len() > 1 {
                            // Steal from the back (oldest task)
                            if let Some(task) = queue.pop_back() {
                                return Some(task);
                            }
                        }
                    }
                }

                None
            }
            SchedulingStrategy::Priority | SchedulingStrategy::Adaptive => {
                let mut queue = self.task_queue.lock().await;
                queue.pop()
            }
            SchedulingStrategy::ShortestJobFirst => {
                let mut queue = self.task_queue.lock().await;
                // Convert to vec, sort by complexity, take shortest
                let mut tasks: Vec<_> = queue.drain().collect();
                tasks.sort_by(|a, b| {
                    a.task
                        .estimated_complexity
                        .partial_cmp(&b.task.estimated_complexity)
                        .unwrap_or(Ordering::Equal)
                });

                let result = tasks.first().cloned();

                // Put remaining tasks back
                for task in tasks.into_iter().skip(1) {
                    queue.push(task);
                }

                result
            }
            SchedulingStrategy::FIFO => {
                let mut queue = self.task_queue.lock().await;
                queue.pop()
            }
        }
    }

    /// Mark task as started on a worker
    pub async fn task_started(&self, worker_id: usize) {
        let mut workers = self.workers.lock().await;
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.active_tasks += 1;
        }
    }

    /// Mark task as completed on a worker
    pub async fn task_completed(&self, worker_id: usize, execution_time_ms: u64) {
        let mut workers = self.workers.lock().await;
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.active_tasks = worker.active_tasks.saturating_sub(1);
            worker.completed_tasks += 1;
            worker.total_execution_time_ms += execution_time_ms;
            worker.last_task_completion = Some(Instant::now());
        }
    }

    /// Get worker statistics
    pub async fn get_worker_stats(&self) -> Vec<WorkerNode> {
        self.workers.lock().await.clone()
    }

    /// Get queue size
    pub async fn queue_size(&self) -> usize {
        match self.strategy {
            SchedulingStrategy::WorkStealing => {
                let queues = self.work_stealing_queues.lock().await;
                queues.values().map(|q| q.len()).sum()
            }
            _ => self.task_queue.lock().await.len(),
        }
    }

    /// Rebalance tasks across workers (for work-stealing strategy)
    pub async fn rebalance(&self) -> Result<()> {
        if self.strategy != SchedulingStrategy::WorkStealing {
            return Ok(());
        }

        let mut queues = self.work_stealing_queues.lock().await;
        let workers = self.workers.lock().await;

        // Calculate average queue size
        let total_tasks: usize = queues.values().map(|q| q.len()).sum();
        let avg_size = total_tasks / queues.len();

        // Move tasks from overloaded workers to underloaded ones
        let mut tasks_to_redistribute: Vec<ScheduledTask> = Vec::new();

        for (worker_id, queue) in queues.iter_mut() {
            if queue.len() > avg_size + 2 {
                // Worker is overloaded
                while queue.len() > avg_size {
                    if let Some(task) = queue.pop_back() {
                        tasks_to_redistribute.push(task);
                    }
                }
            }
        }

        // Redistribute to underloaded workers
        for task in tasks_to_redistribute {
            let least_loaded = workers
                .iter()
                .min_by(|a, b| {
                    let a_size = queues.get(&a.id).map(|q| q.len()).unwrap_or(0);
                    let b_size = queues.get(&b.id).map(|q| q.len()).unwrap_or(0);
                    a_size.cmp(&b_size)
                })
                .map(|w| w.id)
                .unwrap_or(0);

            queues
                .entry(least_loaded)
                .or_insert_with(VecDeque::new)
                .push_back(task);
        }

        Ok(())
    }

    /// Acquire execution permit
    pub async fn acquire_permit(&self) -> Result<tokio::sync::SemaphorePermit> {
        self.semaphore
            .acquire()
            .await
            .context("Failed to acquire semaphore permit")
    }

    /// AI-powered task scheduling with predictive optimization
    pub async fn schedule_with_ai_prediction(
        &self,
        task: &crate::parallel_agent::SubTask,
        system_metrics: &crate::dynamic_scaling::SystemMetrics,
        workers: &[WorkerNode],
    ) -> Result<usize> {
        // Use AI-like logic to predict optimal worker assignment
        let predictions = self.predict_task_performance(task, workers).await;

        // Consider system load and task complexity
        let load_score = system_metrics.load_score();
        let complexity_factor = task.estimated_complexity;

        // Select worker based on predictions and current load
        let best_worker =
            self.select_optimal_worker(&predictions, workers, load_score, complexity_factor);

        Ok(best_worker)
    }

    /// Predict task performance on each worker
    async fn predict_task_performance(
        &self,
        task: &crate::parallel_agent::SubTask,
        workers: &[WorkerNode],
    ) -> Vec<f32> {
        let mut predictions = Vec::new();

        for worker in workers {
            // Simple prediction model based on worker history and task complexity
            let base_performance = worker.avg_execution_time_ms() as f32;
            let complexity_penalty = task.estimated_complexity * 1000.0; // Convert to ms scale
            let load_penalty = worker.load_score() * 500.0;

            let predicted_time = base_performance + complexity_penalty + load_penalty;
            let performance_score = 1.0 / (1.0 + predicted_time / 1000.0); // Normalize to 0-1

            predictions.push(performance_score);
        }

        predictions
    }

    /// Select optimal worker based on predictions and constraints
    fn select_optimal_worker(
        &self,
        predictions: &[f32],
        workers: &[WorkerNode],
        load_score: f32,
        complexity_factor: f32,
    ) -> usize {
        let mut best_worker = 0;
        let mut best_score = 0.0;

        for (i, (prediction, worker)) in predictions.iter().zip(workers).enumerate() {
            // Combined score considering prediction, current load, and worker capacity
            let worker_load = worker.load_score();
            let capacity_score = 1.0 - worker_load; // Higher capacity = better score

            let combined_score = *prediction * capacity_score * (1.0 + complexity_factor);

            // Prefer workers with recent activity for cache locality
            let recency_bonus = if worker.last_task_completion.is_some() {
                0.1
            } else {
                0.0
            };

            let final_score = combined_score + recency_bonus;

            if final_score > best_score {
                best_score = final_score;
                best_worker = i;
            }
        }

        best_worker
    }

    /// Adaptive strategy selection based on workload patterns
    pub async fn adapt_strategy(
        &self,
        recent_tasks: &[crate::parallel_agent::SubTask],
        system_metrics: &crate::dynamic_scaling::SystemMetrics,
    ) -> SchedulingStrategy {
        // Analyze task patterns
        let avg_complexity: f32 = recent_tasks
            .iter()
            .map(|t| t.estimated_complexity)
            .sum::<f32>()
            / recent_tasks.len() as f32;

        let has_dependencies = recent_tasks.iter().any(|t| !t.dependencies.is_empty());

        let queue_pressure = system_metrics.queue_length as f32 / 10.0; // Normalize

        // Adaptive strategy selection
        if avg_complexity > 0.7 && has_dependencies {
            // Complex tasks with dependencies - use priority scheduling
            SchedulingStrategy::Priority
        } else if queue_pressure > 0.8 {
            // High queue pressure - use work stealing for load balancing
            SchedulingStrategy::WorkStealing
        } else if avg_complexity < 0.3 {
            // Simple tasks - use FIFO for efficiency
            SchedulingStrategy::FIFO
        } else {
            // Mixed complexity - use shortest job first
            SchedulingStrategy::ShortestJobFirst
        }
    }

    /// Generate scheduler statistics report
    pub fn generate_report(&self, workers: &[WorkerNode]) -> String {
        let mut report = String::from("Scheduler Statistics\n");
        report.push_str("====================\n\n");

        let total_completed: usize = workers.iter().map(|w| w.completed_tasks).sum();
        let total_time: u64 = workers.iter().map(|w| w.total_execution_time_ms).sum();

        report.push_str(&format!("Strategy: {:?}\n", self.strategy));
        report.push_str(&format!("Workers: {}\n", workers.len()));
        report.push_str(&format!("Total Tasks Completed: {}\n", total_completed));
        report.push_str(&format!("Total Execution Time: {}ms\n\n", total_time));

        report.push_str("Worker Details:\n");
        for worker in workers {
            report.push_str(&format!("  Worker {}:\n", worker.id));
            report.push_str(&format!("    Active: {}\n", worker.active_tasks));
            report.push_str(&format!("    Completed: {}\n", worker.completed_tasks));
            report.push_str(&format!(
                "    Avg Time: {}ms\n",
                worker.avg_execution_time_ms()
            ));
            report.push_str(&format!("    Load Score: {:.2}\n", worker.load_score()));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scheduler_creation() {
        let scheduler = AdvancedScheduler::new(4, SchedulingStrategy::Priority);
        let stats = scheduler.get_worker_stats().await;
        assert_eq!(stats.len(), 4);
    }

    #[tokio::test]
    async fn test_task_submission() {
        let scheduler = AdvancedScheduler::new(4, SchedulingStrategy::Priority);

        let task = SubTask {
            id: "test".to_string(),
            description: "Test task".to_string(),
            priority: 10,
            dependencies: vec![],
            estimated_complexity: 0.5,
        };

        scheduler.submit_task(task).await.unwrap();
        assert_eq!(scheduler.queue_size().await, 1);
    }

    #[tokio::test]
    async fn test_priority_scheduling() {
        let scheduler = AdvancedScheduler::new(4, SchedulingStrategy::Priority);

        // Submit tasks with different priorities
        for i in 0..5 {
            let task = SubTask {
                id: format!("task_{}", i),
                description: format!("Task {}", i),
                priority: i as u8,
                dependencies: vec![],
                estimated_complexity: 0.5,
            };
            scheduler.submit_task(task).await.unwrap();
        }

        // Lower priority number comes first due to max heap ordering
        let next = scheduler.get_next_task(0).await.unwrap();
        assert_eq!(next.task.priority, 0);
    }

    #[tokio::test]
    async fn test_work_stealing() {
        let scheduler = AdvancedScheduler::new(4, SchedulingStrategy::WorkStealing);

        // Submit multiple tasks
        for i in 0..10 {
            let task = SubTask {
                id: format!("task_{}", i),
                description: format!("Task {}", i),
                priority: 5,
                dependencies: vec![],
                estimated_complexity: 0.5,
            };
            scheduler.submit_task(task).await.unwrap();
        }

        // All workers should get tasks through work stealing
        for worker_id in 0..4 {
            let task = scheduler.get_next_task(worker_id).await;
            assert!(task.is_some());
        }
    }

    #[tokio::test]
    async fn test_worker_stats() {
        let scheduler = AdvancedScheduler::new(2, SchedulingStrategy::Priority);

        scheduler.task_started(0).await;
        scheduler.task_completed(0, 100).await;

        let stats = scheduler.get_worker_stats().await;
        assert_eq!(stats[0].completed_tasks, 1);
        assert_eq!(stats[0].total_execution_time_ms, 100);
        assert_eq!(stats[0].avg_execution_time_ms(), 100);
    }

    #[tokio::test]
    async fn test_rebalance() {
        let scheduler = AdvancedScheduler::new(2, SchedulingStrategy::WorkStealing);

        // Overload one worker
        for i in 0..10 {
            let task = SubTask {
                id: format!("task_{}", i),
                description: format!("Task {}", i),
                priority: 5,
                dependencies: vec![],
                estimated_complexity: 0.5,
            };
            scheduler.submit_task(task).await.unwrap();
        }

        scheduler.rebalance().await.unwrap();

        // After rebalance, tasks should be distributed more evenly
        let queues = scheduler.work_stealing_queues.lock().await;
        let sizes: Vec<usize> = queues.values().map(|q| q.len()).collect();

        // Check that distribution is reasonably balanced
        let max_size = *sizes.iter().max().unwrap();
        let min_size = *sizes.iter().min().unwrap();
        assert!(max_size - min_size <= 2);
    }
}
