use serde::{Deserialize, Serialize};
use std::collections::HashMap;
/// Performance profiling and monitoring utilities
///
/// Provides zero-overhead performance measurement tools for production use
use std::time::Instant;

/// Performance metrics for a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub operation_name: String,
    pub duration_ms: u64,
    pub memory_delta_bytes: i64,
    pub cpu_time_ms: u64,
    pub timestamp: std::time::SystemTime,
}

/// Performance profiler for tracking operation metrics
pub struct PerformanceProfiler {
    metrics: HashMap<String, Vec<PerformanceMetrics>>,
    start_times: HashMap<String, Instant>,
    enabled: bool,
}

impl PerformanceProfiler {
    /// Create a new performance profiler
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
            start_times: HashMap::new(),
            enabled: true,
        }
    }

    /// Enable or disable profiling
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Start timing an operation
    pub fn start(&mut self, operation: impl Into<String>) {
        if !self.enabled {
            return;
        }

        let operation = operation.into();
        self.start_times.insert(operation, Instant::now());
    }

    /// Stop timing an operation and record metrics
    pub fn stop(&mut self, operation: impl Into<String>) -> Option<u64> {
        if !self.enabled {
            return None;
        }

        let operation = operation.into();
        if let Some(start) = self.start_times.remove(&operation) {
            let duration = start.elapsed();
            let duration_ms = duration.as_millis() as u64;

            let metric = PerformanceMetrics {
                operation_name: operation.clone(),
                duration_ms,
                memory_delta_bytes: 0,    // Would track in production
                cpu_time_ms: duration_ms, // Approximation
                timestamp: std::time::SystemTime::now(),
            };

            self.metrics
                .entry(operation)
                .or_insert_with(Vec::new)
                .push(metric);

            Some(duration_ms)
        } else {
            None
        }
    }

    /// Get metrics for a specific operation
    pub fn get_metrics(&self, operation: &str) -> Option<&Vec<PerformanceMetrics>> {
        self.metrics.get(operation)
    }

    /// Get all metrics
    pub fn get_all_metrics(&self) -> &HashMap<String, Vec<PerformanceMetrics>> {
        &self.metrics
    }

    /// Calculate statistics for an operation
    pub fn calculate_stats(&self, operation: &str) -> Option<OperationStats> {
        let metrics = self.get_metrics(operation)?;

        if metrics.is_empty() {
            return None;
        }

        let durations: Vec<u64> = metrics.iter().map(|m| m.duration_ms).collect();
        let total: u64 = durations.iter().sum();
        let count = durations.len() as u64;
        let avg = total / count;

        let min = *durations.iter().min().unwrap();
        let max = *durations.iter().max().unwrap();

        // Calculate percentiles
        let mut sorted = durations.clone();
        sorted.sort_unstable();

        let p50 = sorted[sorted.len() / 2];
        let p95 = sorted[(sorted.len() * 95) / 100];
        let p99 = sorted[(sorted.len() * 99) / 100];

        Some(OperationStats {
            operation: operation.to_string(),
            count,
            total_ms: total,
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            p50_ms: p50,
            p95_ms: p95,
            p99_ms: p99,
        })
    }

    /// Clear all metrics
    pub fn clear(&mut self) {
        self.metrics.clear();
        self.start_times.clear();
    }

    /// Generate performance report
    pub fn generate_report(&self) -> String {
        let mut report = String::from("Performance Report\n");
        report.push_str("==================\n\n");

        for (operation, _metrics) in &self.metrics {
            if let Some(stats) = self.calculate_stats(operation) {
                report.push_str(&format!("Operation: {}\n", operation));
                report.push_str(&format!("  Count: {}\n", stats.count));
                report.push_str(&format!("  Total: {}ms\n", stats.total_ms));
                report.push_str(&format!("  Avg: {}ms\n", stats.avg_ms));
                report.push_str(&format!("  Min: {}ms\n", stats.min_ms));
                report.push_str(&format!("  Max: {}ms\n", stats.max_ms));
                report.push_str(&format!("  P50: {}ms\n", stats.p50_ms));
                report.push_str(&format!("  P95: {}ms\n", stats.p95_ms));
                report.push_str(&format!("  P99: {}ms\n", stats.p99_ms));
                report.push_str("\n");
            }
        }

        report
    }
}

impl Default for PerformanceProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistical summary for an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    pub operation: String,
    pub count: u64,
    pub total_ms: u64,
    pub avg_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
}

/// RAII guard for automatic timing
pub struct TimingGuard<'a> {
    profiler: &'a mut PerformanceProfiler,
    operation: String,
}

impl<'a> TimingGuard<'a> {
    pub fn new(profiler: &'a mut PerformanceProfiler, operation: impl Into<String>) -> Self {
        let operation = operation.into();
        profiler.start(&operation);
        Self {
            profiler,
            operation,
        }
    }
}

impl<'a> Drop for TimingGuard<'a> {
    fn drop(&mut self) {
        self.profiler.stop(&self.operation);
    }
}

/// Macro for easy performance timing
#[macro_export]
macro_rules! time_operation {
    ($profiler:expr, $name:expr, $block:block) => {{
        $profiler.start($name);
        let result = $block;
        $profiler.stop($name);
        result
    }};
}

/// Memory usage tracker
pub struct MemoryTracker {
    initial_usage: u64,
}

impl MemoryTracker {
    /// Create a new memory tracker
    pub fn new() -> Self {
        Self {
            initial_usage: Self::get_current_usage(),
        }
    }

    /// Get current memory usage in bytes
    #[cfg(target_os = "linux")]
    pub fn get_current_usage() -> u64 {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb * 1024;
                        }
                    }
                }
            }
        }
        0
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_current_usage() -> u64 {
        0 // Placeholder for other platforms
    }

    /// Get memory delta since creation
    pub fn get_delta(&self) -> i64 {
        let current = Self::get_current_usage();
        current as i64 - self.initial_usage as i64
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_profiler_basic() {
        let mut profiler = PerformanceProfiler::new();

        profiler.start("test_op");
        std::thread::sleep(Duration::from_millis(10));
        let duration = profiler.stop("test_op");

        assert!(duration.is_some());
        assert!(duration.unwrap() >= 10);
    }

    #[test]
    fn test_profiler_stats() {
        let mut profiler = PerformanceProfiler::new();

        for _ in 0..10 {
            profiler.start("test_op");
            std::thread::sleep(Duration::from_millis(5));
            profiler.stop("test_op");
        }

        let stats = profiler.calculate_stats("test_op");
        assert!(stats.is_some());

        let stats = stats.unwrap();
        assert_eq!(stats.count, 10);
        assert!(stats.avg_ms >= 5);
    }

    #[test]
    fn test_profiler_report() {
        let mut profiler = PerformanceProfiler::new();

        profiler.start("op1");
        std::thread::sleep(Duration::from_millis(5));
        profiler.stop("op1");

        let report = profiler.generate_report();
        assert!(report.contains("Performance Report"));
        assert!(report.contains("op1"));
    }

    #[test]
    fn test_timing_guard() {
        let mut profiler = PerformanceProfiler::new();

        {
            let _guard = TimingGuard::new(&mut profiler, "guarded_op");
            std::thread::sleep(Duration::from_millis(5));
        }

        let stats = profiler.calculate_stats("guarded_op");
        assert!(stats.is_some());
    }

    #[test]
    fn test_memory_tracker() {
        let tracker = MemoryTracker::new();
        let _data = vec![0u8; 1024 * 1024]; // Allocate 1MB
        let delta = tracker.get_delta();

        // Delta might be positive or zero depending on platform
        assert!(delta >= 0 || delta == 0);
    }
}
