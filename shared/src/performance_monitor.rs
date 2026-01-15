use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Ultra-high performance metrics collector
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    inner: Arc<RwLock<MetricsInner>>,
}

#[derive(Debug, Default)]
struct MetricsInner {
    operation_times: HashMap<String, Vec<Duration>>,
    operation_counts: HashMap<String, u64>,
    current_operations: HashMap<String, Instant>,
    total_operations: u64,
    total_latency: Duration,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsInner::default())),
        }
    }

    /// Start timing an operation
    pub async fn start_operation(&self, operation: &str) {
        let mut inner = self.inner.write().await;
        inner.current_operations.insert(operation.to_string(), Instant::now());
        inner.total_operations += 1;
    }

    /// End timing an operation and record the duration
    pub async fn end_operation(&self, operation: &str) {
        let end_time = Instant::now();
        let mut inner = self.inner.write().await;

        if let Some(start_time) = inner.current_operations.remove(operation) {
            let duration = end_time.duration_since(start_time);
            inner.operation_times.entry(operation.to_string())
                .or_insert_with(Vec::new)
                .push(duration);
            inner.total_latency += duration;

            let count = inner.operation_counts.entry(operation.to_string())
                .or_insert(0);
            *count += 1;
        }
    }

    /// Get average latency for an operation
    pub async fn average_latency(&self, operation: &str) -> Option<Duration> {
        let inner = self.inner.read().await;
        inner.operation_times.get(operation)
            .and_then(|times| {
                if times.is_empty() {
                    None
                } else {
                    let total: Duration = times.iter().sum();
                    Some(total / times.len() as u32)
                }
            })
    }

    /// Get throughput (operations per second) for an operation
    pub async fn throughput(&self, operation: &str) -> Option<f64> {
        let inner = self.inner.read().await;
        inner.operation_counts.get(operation)
            .and_then(|&count| {
                let total_time = inner.operation_times.get(operation)?
                    .iter()
                    .sum::<Duration>();
                if total_time.as_secs_f64() > 0.0 {
                    Some(count as f64 / total_time.as_secs_f64())
                } else {
                    None
                }
            })
    }

    /// Get overall system performance stats
    pub async fn system_stats(&self) -> SystemStats {
        let inner = self.inner.read().await;
        let avg_latency = if inner.total_operations > 0 {
            inner.total_latency / inner.total_operations as u32
        } else {
            Duration::from_secs(0)
        };

        SystemStats {
            total_operations: inner.total_operations,
            average_latency: avg_latency,
            total_latency: inner.total_latency,
            active_operations: inner.current_operations.len(),
        }
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        let mut inner = self.inner.write().await;
        *inner = MetricsInner::default();
    }
}

#[derive(Debug, Clone)]
pub struct SystemStats {
    pub total_operations: u64,
    pub average_latency: Duration,
    pub total_latency: Duration,
    pub active_operations: usize,
}

/// Global performance metrics instance
lazy_static::lazy_static! {
    pub static ref GLOBAL_METRICS: PerformanceMetrics = PerformanceMetrics::new();
}



impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_performance_metrics() {
        let metrics = PerformanceMetrics::new();

        // Test timing an operation
        metrics.start_operation("test_op").await;
        sleep(Duration::from_millis(10)).await;
        metrics.end_operation("test_op").await;

        let avg_latency = metrics.average_latency("test_op").await;
        assert!(avg_latency.unwrap() >= Duration::from_millis(10));

        let throughput = metrics.throughput("test_op").await;
        assert!(throughput.unwrap() > 0.0);

        // Test system stats
        let stats = metrics.system_stats().await;
        assert_eq!(stats.total_operations, 1);
        assert!(stats.average_latency >= Duration::from_millis(10));
    }
}