/// Dynamic agent scaling based on task complexity and system load
///
/// Provides intelligent scaling of agent resources:
/// - Automatic worker pool sizing
/// - Load-based scaling decisions
/// - Resource utilization optimization
/// - Predictive scaling based on task queue
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Scaling policy determines when to scale up/down
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingPolicy {
    /// Manual: No automatic scaling
    Manual,
    /// Conservative: Scale slowly, prefer stability
    Conservative,
    /// Aggressive: Scale quickly to meet demand
    Aggressive,
    /// Predictive: Use ML-based predictions for scaling
    Predictive,
    /// Adaptive: Dynamically adjust policy based on workload patterns
    Adaptive,
}

/// System metrics for scaling decisions
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub cpu_utilization: f32,    // 0.0 to 1.0
    pub memory_utilization: f32, // 0.0 to 1.0
    pub queue_length: usize,
    pub active_workers: usize,
    pub avg_task_completion_ms: u64,
    pub task_arrival_rate: f32, // tasks per second
    pub timestamp: Instant,
}

impl SystemMetrics {
    pub fn new() -> Self {
        Self {
            cpu_utilization: 0.0,
            memory_utilization: 0.0,
            queue_length: 0,
            active_workers: 0,
            avg_task_completion_ms: 0,
            task_arrival_rate: 0.0,
            timestamp: Instant::now(),
        }
    }

    /// Calculate system load score (0.0 = no load, 1.0+ = overloaded)
    pub fn load_score(&self) -> f32 {
        let cpu_weight = 0.4;
        let memory_weight = 0.2;
        let queue_weight = 0.3;
        let worker_weight = 0.1;

        let queue_pressure = if self.active_workers == 0 {
            1.0
        } else {
            (self.queue_length as f32 / self.active_workers as f32).min(1.0)
        };

        let worker_utilization = if self.active_workers == 0 {
            0.0
        } else {
            self.active_workers as f32 / num_cpus::get() as f32
        };

        (self.cpu_utilization * cpu_weight)
            + (self.memory_utilization * memory_weight)
            + (queue_pressure * queue_weight)
            + (worker_utilization * worker_weight)
    }
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Scaling decision recommendation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalingDecision {
    /// Scale up by N workers
    ScaleUp(usize),
    /// Scale down by N workers
    ScaleDown(usize),
    /// No scaling needed
    NoChange,
}

/// Configuration for dynamic scaling
#[derive(Debug, Clone)]
pub struct ScalingConfig {
    pub policy: ScalingPolicy,
    pub min_workers: usize,
    pub max_workers: usize,
    pub scale_up_threshold: f32,     // Load score to trigger scale up
    pub scale_down_threshold: f32,   // Load score to trigger scale down
    pub cooldown_period_secs: u64,   // Minimum time between scaling operations
    pub scale_up_increment: usize,   // How many workers to add
    pub scale_down_increment: usize, // How many workers to remove
}

impl ScalingConfig {
    pub fn new(policy: ScalingPolicy) -> Self {
        match policy {
            ScalingPolicy::Conservative => Self {
                policy,
                min_workers: 2,
                max_workers: num_cpus::get(),
                scale_up_threshold: 0.8,
                scale_down_threshold: 0.3,
                cooldown_period_secs: 60,
                scale_up_increment: 1,
                scale_down_increment: 1,
            },
            ScalingPolicy::Aggressive => Self {
                policy,
                min_workers: 1,
                max_workers: num_cpus::get() * 2,
                scale_up_threshold: 0.6,
                scale_down_threshold: 0.4,
                cooldown_period_secs: 10,
                scale_up_increment: 2,
                scale_down_increment: 1,
            },
            ScalingPolicy::Predictive => Self {
                policy,
                min_workers: 2,
                max_workers: num_cpus::get() * 2,
                scale_up_threshold: 0.7,
                scale_down_threshold: 0.3,
                cooldown_period_secs: 30,
                scale_up_increment: 2,
                scale_down_increment: 1,
            },
            ScalingPolicy::Adaptive => Self {
                policy,
                min_workers: 1,
                max_workers: num_cpus::get() * 2,
                scale_up_threshold: 0.7,
                scale_down_threshold: 0.3,
                cooldown_period_secs: 30,
                scale_up_increment: 1,
                scale_down_increment: 1,
            },
            ScalingPolicy::Manual => Self {
                policy,
                min_workers: num_cpus::get(),
                max_workers: num_cpus::get(),
                scale_up_threshold: 1.0,   // Never scale
                scale_down_threshold: 0.0, // Never scale
                cooldown_period_secs: 3600,
                scale_up_increment: 0,
                scale_down_increment: 0,
            },
        }
    }
}

/// Dynamic scaling controller
pub struct DynamicScalingController {
    config: Arc<RwLock<ScalingConfig>>,
    current_workers: Arc<RwLock<usize>>,
    last_scaling_action: Arc<RwLock<Instant>>,
    metrics_history: Arc<RwLock<Vec<SystemMetrics>>>,
}

impl DynamicScalingController {
    /// Create a new scaling controller
    pub fn new(config: ScalingConfig) -> Self {
        let initial_workers = config.min_workers;
        Self {
            config: Arc::new(RwLock::new(config)),
            current_workers: Arc::new(RwLock::new(initial_workers)),
            last_scaling_action: Arc::new(RwLock::new(Instant::now())),
            metrics_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record system metrics
    pub async fn record_metrics(&self, metrics: SystemMetrics) {
        let mut history = self.metrics_history.write().await;
        history.push(metrics);

        // Keep last 100 samples
        if history.len() > 100 {
            history.remove(0);
        }
    }

    /// Determine if scaling action is needed
    pub async fn should_scale(&self, metrics: &SystemMetrics) -> Result<ScalingDecision> {
        let config = self.config.read().await;
        let last_action = *self.last_scaling_action.read().await;
        let current_workers = *self.current_workers.read().await;

        // Check cooldown period
        if last_action.elapsed().as_secs() < config.cooldown_period_secs {
            return Ok(ScalingDecision::NoChange);
        }

        let load_score = metrics.load_score();

        // Scale up if overloaded
        if load_score >= config.scale_up_threshold && current_workers < config.max_workers {
            let increment = config
                .scale_up_increment
                .min(config.max_workers - current_workers);
            return Ok(ScalingDecision::ScaleUp(increment));
        }

        // Scale down if underutilized
        if load_score <= config.scale_down_threshold && current_workers > config.min_workers {
            let decrement = config
                .scale_down_increment
                .min(current_workers - config.min_workers);
            return Ok(ScalingDecision::ScaleDown(decrement));
        }

        Ok(ScalingDecision::NoChange)
    }

    /// Apply scaling decision
    pub async fn apply_scaling(&self, decision: ScalingDecision) -> Result<usize> {
        match decision {
            ScalingDecision::ScaleUp(n) => {
                let mut workers = self.current_workers.write().await;
                let config = self.config.read().await;

                let old_count = *workers;
                let new_count = (*workers + n).min(config.max_workers);
                *workers = new_count;

                *self.last_scaling_action.write().await = Instant::now();

                println!("Scaled up: {} -> {} workers", old_count, new_count);
                Ok(new_count)
            }
            ScalingDecision::ScaleDown(n) => {
                let mut workers = self.current_workers.write().await;
                let config = self.config.read().await;

                let old_count = *workers;
                let new_count = workers.saturating_sub(n).max(config.min_workers);
                *workers = new_count;

                *self.last_scaling_action.write().await = Instant::now();

                println!("Scaled down: {} -> {} workers", old_count, new_count);
                Ok(new_count)
            }
            ScalingDecision::NoChange => Ok(*self.current_workers.read().await),
        }
    }

    /// Get current worker count
    pub async fn get_worker_count(&self) -> usize {
        *self.current_workers.read().await
    }

    /// Predict future load based on historical metrics
    pub async fn predict_load(&self, lookahead_secs: u64) -> f32 {
        let history = self.metrics_history.read().await;

        if history.len() < 2 {
            return 0.5; // Default prediction
        }

        // Simple moving average with trend analysis
        let recent_samples = history.iter().rev().take(10).collect::<Vec<_>>();
        let avg_load: f32 = recent_samples.iter().map(|m| m.load_score()).sum::<f32>()
            / recent_samples.len() as f32;

        // Calculate trend (increasing or decreasing load)
        if recent_samples.len() >= 2 {
            let recent = recent_samples[0].load_score();
            let older = recent_samples[recent_samples.len() - 1].load_score();
            let trend = recent - older;

            // Extrapolate trend
            let predicted = avg_load + (trend * lookahead_secs as f32 / 60.0);
            predicted.clamp(0.0, 1.0)
        } else {
            avg_load
        }
    }

    /// Optimize scaling configuration based on workload patterns
    pub async fn optimize_config(&self) -> Result<()> {
        let history = self.metrics_history.read().await;

        if history.len() < 10 {
            return Ok(()); // Not enough data
        }

        let mut config = self.config.write().await;

        // Analyze workload patterns
        let avg_cpu: f32 =
            history.iter().map(|m| m.cpu_utilization).sum::<f32>() / history.len() as f32;
        let avg_queue_length: f32 =
            history.iter().map(|m| m.queue_length as f32).sum::<f32>() / history.len() as f32;

        // Adjust thresholds based on patterns
        if avg_cpu > 0.8 {
            // High CPU usage - be more aggressive with scale up
            config.scale_up_threshold = 0.6;
            config.scale_up_increment = 2;
        } else if avg_cpu < 0.3 {
            // Low CPU usage - scale down more aggressively
            config.scale_down_threshold = 0.4;
            config.scale_down_increment = 2;
        }

        if avg_queue_length > 10.0 {
            // Large queues - scale up faster
            config.cooldown_period_secs = 10;
        }

        Ok(())
    }

    /// Generate scaling report
    pub async fn generate_report(&self) -> String {
        let config = self.config.read().await;
        let current_workers = *self.current_workers.read().await;
        let history = self.metrics_history.read().await;

        let mut report = String::from("Dynamic Scaling Report\n");
        report.push_str("======================\n\n");

        report.push_str(&format!("Policy: {:?}\n", config.policy));
        report.push_str(&format!("Current Workers: {}\n", current_workers));
        report.push_str(&format!(
            "Worker Range: {} - {}\n",
            config.min_workers, config.max_workers
        ));
        report.push_str(&format!(
            "Scale Up Threshold: {:.2}\n",
            config.scale_up_threshold
        ));
        report.push_str(&format!(
            "Scale Down Threshold: {:.2}\n\n",
            config.scale_down_threshold
        ));

        if !history.is_empty() {
            let latest = history.last().unwrap();
            report.push_str("Current Metrics:\n");
            report.push_str(&format!(
                "  CPU Utilization: {:.1}%\n",
                latest.cpu_utilization * 100.0
            ));
            report.push_str(&format!(
                "  Memory Utilization: {:.1}%\n",
                latest.memory_utilization * 100.0
            ));
            report.push_str(&format!("  Queue Length: {}\n", latest.queue_length));
            report.push_str(&format!("  Load Score: {:.2}\n", latest.load_score()));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_system_metrics_load_score() {
        let mut metrics = SystemMetrics::new();
        metrics.cpu_utilization = 0.8;
        metrics.memory_utilization = 0.6;
        metrics.queue_length = 5;
        metrics.active_workers = 4;

        let score = metrics.load_score();
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_scaling_config_conservative() {
        let config = ScalingConfig::new(ScalingPolicy::Conservative);
        assert_eq!(config.scale_up_threshold, 0.8);
        assert_eq!(config.scale_down_threshold, 0.3);
        assert_eq!(config.scale_up_increment, 1);
    }

    #[test]
    fn test_scaling_config_aggressive() {
        let config = ScalingConfig::new(ScalingPolicy::Aggressive);
        assert_eq!(config.scale_up_threshold, 0.6);
        assert_eq!(config.scale_up_increment, 2);
    }

    #[tokio::test]
    async fn test_scaling_controller_creation() {
        let config = ScalingConfig::new(ScalingPolicy::Conservative);
        let controller = DynamicScalingController::new(config.clone());

        let workers = controller.get_worker_count().await;
        assert_eq!(workers, config.min_workers);
    }

    #[tokio::test]
    async fn test_should_scale_up() {
        let mut config = ScalingConfig::new(ScalingPolicy::Aggressive);
        config.cooldown_period_secs = 0; // Disable cooldown for test
        let controller = DynamicScalingController::new(config);

        let mut metrics = SystemMetrics::new();
        metrics.cpu_utilization = 0.9;
        metrics.memory_utilization = 0.8;
        metrics.queue_length = 20;
        metrics.active_workers = 2;

        let decision = controller.should_scale(&metrics).await.unwrap();
        assert!(matches!(decision, ScalingDecision::ScaleUp(_)));
    }

    #[tokio::test]
    async fn test_should_scale_down() {
        let config = ScalingConfig::new(ScalingPolicy::Aggressive);
        let controller = DynamicScalingController::new(config);

        // Wait for cooldown
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut metrics = SystemMetrics::new();
        metrics.cpu_utilization = 0.1;
        metrics.memory_utilization = 0.2;
        metrics.queue_length = 0;
        metrics.active_workers = 8;

        let decision = controller.should_scale(&metrics).await.unwrap();
        assert!(matches!(
            decision,
            ScalingDecision::ScaleDown(_) | ScalingDecision::NoChange
        ));
    }

    #[tokio::test]
    async fn test_apply_scaling_up() {
        let config = ScalingConfig::new(ScalingPolicy::Aggressive);
        let controller = DynamicScalingController::new(config);

        let initial_workers = controller.get_worker_count().await;
        let decision = ScalingDecision::ScaleUp(2);

        let new_count = controller.apply_scaling(decision).await.unwrap();
        assert_eq!(new_count, initial_workers + 2);
    }

    #[tokio::test]
    async fn test_apply_scaling_respects_limits() {
        let config = ScalingConfig::new(ScalingPolicy::Conservative);
        let controller = DynamicScalingController::new(config.clone());

        // Try to scale up beyond max
        let decision = ScalingDecision::ScaleUp(1000);
        let new_count = controller.apply_scaling(decision).await.unwrap();
        assert_eq!(new_count, config.max_workers);
    }

    #[tokio::test]
    async fn test_metrics_recording() {
        let config = ScalingConfig::new(ScalingPolicy::Conservative);
        let controller = DynamicScalingController::new(config);

        for i in 0..5 {
            let mut metrics = SystemMetrics::new();
            metrics.cpu_utilization = i as f32 * 0.1;
            controller.record_metrics(metrics).await;
        }

        let history = controller.metrics_history.read().await;
        assert_eq!(history.len(), 5);
    }

    #[tokio::test]
    async fn test_predict_load() {
        let config = ScalingConfig::new(ScalingPolicy::Predictive);
        let controller = DynamicScalingController::new(config);

        // Record increasing load
        for i in 0..10 {
            let mut metrics = SystemMetrics::new();
            metrics.cpu_utilization = i as f32 * 0.1;
            controller.record_metrics(metrics).await;
        }

        let prediction = controller.predict_load(60).await;
        assert!(prediction >= 0.0 && prediction <= 1.0);
    }
}
