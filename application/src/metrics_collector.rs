//! Real-time Monitoring and Metrics Collection Service
//!
//! This module provides comprehensive monitoring capabilities for tracking
//! system performance, memory usage, search latency, and other key metrics
//! in real-time for production deployments.

use crate::semantic_memory::SemanticMemoryService;
use crate::health_monitor::{HealthMonitor, HealthStatus};
use shared::types::Result;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub timestamp: i64,
    pub memory_usage: MemoryMetrics,
    pub search_performance: SearchMetrics,
    pub health_status: HealthStatus,
    pub conversation_stats: ConversationStats,
    pub system_resources: SystemResourceMetrics,
}

#[derive(Debug, Clone)]
pub struct MemoryMetrics {
    pub total_memories: usize,
    pub total_conversations: usize,
    pub memory_growth_rate: f64, // memories per hour
    pub average_memory_size: usize, // bytes per memory
    pub compression_ratio: f32,
    pub cleanup_operations_today: usize,
}

#[derive(Debug, Clone)]
pub struct SearchMetrics {
    pub total_searches: u64,
    pub average_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub cache_hit_rate: f32,
    pub search_errors: u64,
    pub searches_per_second: f64,
}

#[derive(Debug, Clone)]
pub struct ConversationStats {
    pub active_conversations: usize,
    pub total_conversations: usize,
    pub average_conversation_length: f64,
    pub conversations_created_today: usize,
    pub average_session_duration_minutes: f64,
}

#[derive(Debug, Clone)]
pub struct SystemResourceMetrics {
    pub cpu_usage_percent: f32,
    pub memory_usage_mb: f64,
    pub disk_usage_percent: f32,
    pub network_requests_per_second: f64,
    pub active_connections: usize,
}

#[derive(Debug)]
pub struct PerformanceSnapshot {
    pub metrics: SystemMetrics,
    pub trends: MetricTrends,
    pub alerts: Vec<SystemAlert>,
}

#[derive(Debug, Clone)]
pub struct MetricTrends {
    pub memory_growth_trend: Trend,
    pub search_latency_trend: Trend,
    pub health_score_trend: Trend,
    pub conversation_activity_trend: Trend,
}

#[derive(Debug, Clone)]
pub enum Trend {
    Improving,
    Stable,
    Declining,
    Critical,
}

#[derive(Debug, Clone)]
pub struct SystemAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: i64,
    pub suggested_action: String,
}

#[derive(Debug, Clone)]
pub enum AlertType {
    HighMemoryUsage,
    SlowSearchLatency,
    HealthDegradation,
    HighErrorRate,
    ResourceExhaustion,
    MemoryGrowthSpike,
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

pub struct MetricsCollector {
    semantic_memory: Arc<SemanticMemoryService>,
    health_monitor: Arc<std::sync::Mutex<HealthMonitor>>,
    collection_interval: Duration,
    max_history_size: usize,
    metrics_history: Vec<SystemMetrics>,
    search_latencies: Vec<f64>,
    last_collection: Option<Instant>,
}

impl MetricsCollector {
    pub fn new(
        semantic_memory: Arc<SemanticMemoryService>,
        health_monitor: Arc<std::sync::Mutex<HealthMonitor>>,
    ) -> Self {
        Self {
            semantic_memory,
            health_monitor,
            collection_interval: Duration::from_secs(60), // Collect every minute
            max_history_size: 1000, // Keep last 1000 data points
            metrics_history: Vec::new(),
            search_latencies: Vec::new(),
            last_collection: None,
        }
    }

    /// Collect current system metrics
    pub async fn collect_metrics(&mut self) -> Result<SystemMetrics> {
        let now = Instant::now();

        // Only collect if enough time has passed
        if let Some(last) = self.last_collection {
            if now.duration_since(last) < self.collection_interval {
                // Return most recent metrics if collection too frequent
                if let Some(latest) = self.metrics_history.last() {
                    return Ok(latest.clone());
                }
            }
        }

        let timestamp = chrono::Utc::now().timestamp();

        // Collect all metric categories
        let memory_metrics = self.collect_memory_metrics().await?;
        let search_metrics = self.collect_search_metrics().await?;
        let health_status = self.health_monitor.lock().unwrap().check_health().await?;
        let conversation_stats = self.collect_conversation_stats().await?;
        let system_resources = self.collect_system_resources()?;

        let metrics = SystemMetrics {
            timestamp,
            memory_usage: memory_metrics,
            search_performance: search_metrics,
            health_status,
            conversation_stats,
            system_resources,
        };

        // Store in history
        self.metrics_history.push(metrics.clone());
        if self.metrics_history.len() > self.max_history_size {
            self.metrics_history.remove(0);
        }

        self.last_collection = Some(now);

        Ok(metrics)
    }

    /// Record a search operation for latency tracking
    pub fn record_search_latency(&mut self, latency_ms: f64) {
        self.search_latencies.push(latency_ms);

        // Keep only recent latencies (last 1000)
        if self.search_latencies.len() > 1000 {
            self.search_latencies.remove(0);
        }
    }

    /// Generate performance snapshot with trends and alerts
    pub async fn generate_snapshot(&mut self) -> Result<PerformanceSnapshot> {
        let metrics = self.collect_metrics().await?;
        let trends = self.calculate_trends()?;
        let alerts = self.generate_alerts(&metrics, &trends);

        Ok(PerformanceSnapshot {
            metrics,
            trends,
            alerts,
        })
    }

    /// Get historical metrics for trend analysis
    pub fn get_metrics_history(&self, hours: u32) -> Vec<&SystemMetrics> {
        let cutoff_timestamp = chrono::Utc::now().timestamp() - (hours as i64 * 3600);

        self.metrics_history.iter()
            .filter(|m| m.timestamp >= cutoff_timestamp)
            .collect()
    }

    /// Calculate metric trends based on historical data
    fn calculate_trends(&self) -> Result<MetricTrends> {
        if self.metrics_history.len() < 2 {
            return Ok(MetricTrends {
                memory_growth_trend: Trend::Stable,
                search_latency_trend: Trend::Stable,
                health_score_trend: Trend::Stable,
                conversation_activity_trend: Trend::Stable,
            });
        }

        // Analyze trends over last 10 data points
        let recent = &self.metrics_history[self.metrics_history.len().saturating_sub(10)..];

        let memory_trend = self.calculate_memory_trend(recent);
        let latency_trend = self.calculate_latency_trend(recent);
        let health_trend = self.calculate_health_trend(recent);
        let activity_trend = self.calculate_activity_trend(recent);

        Ok(MetricTrends {
            memory_growth_trend: memory_trend,
            search_latency_trend: latency_trend,
            health_score_trend: health_trend,
            conversation_activity_trend: activity_trend,
        })
    }

    /// Generate alerts based on current metrics and trends
    fn generate_alerts(&self, metrics: &SystemMetrics, trends: &MetricTrends) -> Vec<SystemAlert> {
        let mut alerts = Vec::new();
        let now = chrono::Utc::now().timestamp();

        // Memory usage alerts
        if metrics.memory_usage.total_memories > 100_000 {
            alerts.push(SystemAlert {
                alert_type: AlertType::HighMemoryUsage,
                severity: AlertSeverity::High,
                message: format!("High memory usage: {} memories stored", metrics.memory_usage.total_memories),
                timestamp: now,
                suggested_action: "Consider running memory cleanup or increasing storage capacity".to_string(),
            });
        }

        // Search performance alerts
        if metrics.search_performance.average_latency_ms > 500.0 {
            alerts.push(SystemAlert {
                alert_type: AlertType::SlowSearchLatency,
                severity: AlertSeverity::Medium,
                message: format!("Slow search latency: {:.1}ms average", metrics.search_performance.average_latency_ms),
                timestamp: now,
                suggested_action: "Check Qdrant performance, consider index optimization".to_string(),
            });
        }

        // Health status alerts
        match metrics.health_status.overall {
            crate::health_monitor::HealthLevel::Critical => {
                alerts.push(SystemAlert {
                    alert_type: AlertType::HealthDegradation,
                    severity: AlertSeverity::Critical,
                    message: "System health is critical".to_string(),
                    timestamp: now,
                    suggested_action: "Immediate attention required - check Qdrant connection and services".to_string(),
                });
            }
            crate::health_monitor::HealthLevel::Unhealthy => {
                alerts.push(SystemAlert {
                    alert_type: AlertType::HealthDegradation,
                    severity: AlertSeverity::High,
                    message: "System health is degraded".to_string(),
                    timestamp: now,
                    suggested_action: "Investigate health issues and consider restart".to_string(),
                });
            }
            _ => {}
        }

        // Trend-based alerts
        match trends.memory_growth_trend {
            Trend::Critical => {
                alerts.push(SystemAlert {
                    alert_type: AlertType::MemoryGrowthSpike,
                    severity: AlertSeverity::Medium,
                    message: "Memory growth rate is critically high".to_string(),
                    timestamp: now,
                    suggested_action: "Monitor memory usage closely, consider enabling auto-cleanup".to_string(),
                });
            }
            _ => {}
        }

        alerts
    }

    // Helper methods for collecting specific metrics
    async fn collect_memory_metrics(&self) -> Result<MemoryMetrics> {
        let (total_memories, total_conversations, _, _) = self.semantic_memory.get_memory_stats().await?;

        // Calculate growth rate (simplified)
        let growth_rate = if self.metrics_history.len() >= 2 {
            let recent = &self.metrics_history[self.metrics_history.len() - 2..];
            if let [prev, _] = recent {
                let time_diff_hours = 1.0; // Assuming 1 hour between collections
                (total_memories as f64 - prev.memory_usage.total_memories as f64) / time_diff_hours
            } else {
                0.0
            }
        } else {
            0.0
        };

        Ok(MemoryMetrics {
            total_memories,
            total_conversations,
            memory_growth_rate: growth_rate,
            average_memory_size: 1024, // Placeholder - would calculate actual average
            compression_ratio: 1.0,     // Placeholder - would track actual compression
            cleanup_operations_today: 0, // Placeholder - would track actual cleanups
        })
    }

    async fn collect_search_metrics(&self) -> Result<SearchMetrics> {
        let total_searches = self.search_latencies.len() as u64;

        let (average_latency, p95_latency, p99_latency) = if !self.search_latencies.is_empty() {
            let mut sorted = self.search_latencies.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let avg = sorted.iter().sum::<f64>() / sorted.len() as f64;
            let p95_idx = (sorted.len() as f64 * 0.95) as usize;
            let p99_idx = (sorted.len() as f64 * 0.99) as usize;

            (avg, sorted[p95_idx], sorted[p99_idx])
        } else {
            (0.0, 0.0, 0.0)
        };

        Ok(SearchMetrics {
            total_searches,
            average_latency_ms: average_latency,
            p95_latency_ms: p95_latency,
            p99_latency_ms: p99_latency,
            cache_hit_rate: 0.0, // Placeholder
            search_errors: 0,     // Placeholder
            searches_per_second: 0.0, // Placeholder
        })
    }

    async fn collect_conversation_stats(&self) -> Result<ConversationStats> {
        let (total_memories, total_conversations, _, _) = self.semantic_memory.get_memory_stats().await?;

        let average_length = if total_conversations > 0 {
            total_memories as f64 / total_conversations as f64
        } else {
            0.0
        };

        Ok(ConversationStats {
            active_conversations: total_conversations,
            total_conversations,
            average_conversation_length: average_length,
            conversations_created_today: 0, // Placeholder
            average_session_duration_minutes: 0.0, // Placeholder
        })
    }

    fn collect_system_resources(&self) -> Result<SystemResourceMetrics> {
        // Placeholder implementation - in production, would use system monitoring libraries
        Ok(SystemResourceMetrics {
            cpu_usage_percent: 45.0,      // Placeholder
            memory_usage_mb: 1024.0,      // Placeholder
            disk_usage_percent: 65.0,     // Placeholder
            network_requests_per_second: 150.0, // Placeholder
            active_connections: 25,       // Placeholder
        })
    }

    // Helper methods for trend calculation
    fn calculate_memory_trend(&self, recent: &[SystemMetrics]) -> Trend {
        if recent.len() < 2 { return Trend::Stable; }

        let growth_rates: Vec<f64> = recent.windows(2)
            .map(|window| {
                let [prev, curr] = window else { return 0.0 };
                curr.memory_usage.total_memories as f64 - prev.memory_usage.total_memories as f64
            })
            .collect();

        let avg_growth = growth_rates.iter().sum::<f64>() / growth_rates.len() as f64;

        if avg_growth > 100.0 {
            Trend::Critical
        } else if avg_growth > 10.0 {
            Trend::Declining
        } else if avg_growth < -10.0 {
            Trend::Improving
        } else {
            Trend::Stable
        }
    }

    fn calculate_latency_trend(&self, recent: &[SystemMetrics]) -> Trend {
        if recent.len() < 2 { return Trend::Stable; }

        let latencies: Vec<f64> = recent.iter().map(|m| m.search_performance.average_latency_ms).collect();
        let trend = self.calculate_trend_direction(&latencies);

        match trend {
            t if t > 20.0 => Trend::Declining,  // Latency increasing
            t if t < -20.0 => Trend::Improving, // Latency decreasing
            _ => Trend::Stable,
        }
    }

    fn calculate_health_trend(&self, _recent: &[SystemMetrics]) -> Trend {
        // Simplified - would analyze health scores over time
        Trend::Stable
    }

    fn calculate_activity_trend(&self, recent: &[SystemMetrics]) -> Trend {
        if recent.len() < 2 { return Trend::Stable; }

        let activities: Vec<usize> = recent.iter().map(|m| m.conversation_stats.active_conversations).collect();
        let trend = self.calculate_trend_direction(&activities.iter().map(|&x| x as f64).collect::<Vec<_>>());

        match trend {
            t if t > 5.0 => Trend::Improving,   // Activity increasing
            t if t < -5.0 => Trend::Declining, // Activity decreasing
            _ => Trend::Stable,
        }
    }

    fn calculate_trend_direction(&self, values: &[f64]) -> f64 {
        if values.len() < 2 { return 0.0; }

        // Simple linear trend calculation
        let n = values.len() as f64;
        let sum_x: f64 = (0..values.len()).map(|i| i as f64).sum();
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = values.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
        let sum_xx: f64 = (0..values.len()).map(|i| (i as f64).powi(2)).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x.powi(2));
        slope
    }
}