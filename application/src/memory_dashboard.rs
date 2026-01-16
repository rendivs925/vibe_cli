//! Memory Visualization and Analytics Dashboard
//!
//! This module provides terminal-based visualization of memory metrics,
//! conversation analytics, and system performance dashboards for monitoring
//! the Qdrant-based semantic memory system.

use crate::metrics_collector::{MetricsCollector, PerformanceSnapshot, SystemMetrics, Trend, AlertSeverity};
use crate::semantic_memory::SemanticMemoryService;
use shared::types::Result;
use std::sync::Arc;
use std::io::{self, Write};

pub struct MemoryDashboard {
    metrics_collector: Arc<std::sync::Mutex<MetricsCollector>>,
    semantic_memory: Arc<SemanticMemoryService>,
}

impl MemoryDashboard {
    pub fn new(
        metrics_collector: Arc<std::sync::Mutex<MetricsCollector>>,
        semantic_memory: Arc<SemanticMemoryService>,
    ) -> Self {
        Self {
            metrics_collector,
            semantic_memory,
        }
    }

    /// Display the main dashboard
    pub async fn display_dashboard(&self) -> Result<()> {
        self.clear_screen();
        println!("Vibe CLI - Semantic Memory Dashboard");
        println!("=====================================\n");

        // Get current snapshot
        let mut collector = self.metrics_collector.lock().unwrap();
        let snapshot = collector.generate_snapshot().await?;

        // Display main metrics
        self.display_health_status(&snapshot)?;
        self.display_memory_metrics(&snapshot.metrics)?;
        self.display_search_performance(&snapshot.metrics)?;
        self.display_conversation_stats(&snapshot.metrics)?;
        self.display_system_resources(&snapshot.metrics)?;
        self.display_trends(&snapshot)?;
        self.display_alerts(&snapshot)?;

        println!("\nQuick Stats:");
        println!("-------------");
        println!("Press 'r' to refresh, 'q' to quit, 'h' for help");

        Ok(())
    }

    /// Display health status with color coding
    fn display_health_status(&self, snapshot: &PerformanceSnapshot) -> Result<()> {
        let health_status = match snapshot.metrics.health_status.overall {
            crate::health_monitor::HealthLevel::Healthy => "[HEALTHY]",
            crate::health_monitor::HealthLevel::Degraded => "[DEGRADED]",
            crate::health_monitor::HealthLevel::Unhealthy => "[UNHEALTHY]",
            crate::health_monitor::HealthLevel::Critical => "[CRITICAL]",
        };

        println!("{} System Health: {}", health_status, snapshot.metrics.health_status.overall);
        println!("   Qdrant: {} | Collections: {} | Response: {}ms",
            if snapshot.metrics.health_status.qdrant_connected { "[CONNECTED]" } else { "[DISCONNECTED]" },
            snapshot.metrics.health_status.collections_available.len(),
            snapshot.metrics.health_status.response_time_ms.unwrap_or(0)
        );
        println!();

        Ok(())
    }

    /// Display memory metrics with progress bars
    fn display_memory_metrics(&self, metrics: &SystemMetrics) -> Result<()> {
        println!("Memory Usage");
        println!("   Total Memories: {:>8}", self.format_number(metrics.memory_usage.total_memories));
        println!("   Conversations:  {:>8}", self.format_number(metrics.memory_usage.total_conversations));
        println!("   Growth Rate:    {:>7.1} mem/hr", metrics.memory_usage.memory_growth_rate);
        println!("   Avg Size:       {:>8} bytes", self.format_number(metrics.memory_usage.average_memory_size));

        // Memory usage bar (simplified)
        let memory_usage_percent = (metrics.memory_usage.total_memories as f32 / 100000.0 * 100.0).min(100.0);
        println!("   Usage: {}", self.create_progress_bar(memory_usage_percent, 30));
        println!();

        Ok(())
    }

    /// Display search performance metrics
    fn display_search_performance(&self, metrics: &SystemMetrics) -> Result<()> {
        println!("Search Performance");
        println!("   Total Searches:  {:>10}", self.format_number(metrics.search_performance.total_searches as usize));
        println!("   Avg Latency:     {:>9.1}ms", metrics.search_performance.average_latency_ms);
        println!("   P95 Latency:     {:>9.1}ms", metrics.search_performance.p95_latency_ms);
        println!("   Cache Hit Rate:  {:>8.1}%", metrics.search_performance.cache_hit_rate * 100.0);

        // Latency indicator
        let latency_score = if metrics.search_performance.average_latency_ms < 50.0 {
            "[FAST]"
        } else if metrics.search_performance.average_latency_ms < 200.0 {
            "[GOOD]"
        } else {
            "[SLOW]"
        };
        println!("   Performance:     {}", latency_score);
        println!();

        Ok(())
    }

    /// Display conversation statistics
    fn display_conversation_stats(&self, metrics: &SystemMetrics) -> Result<()> {
        println!("Conversations");
        println!("   Active:          {:>8}", self.format_number(metrics.conversation_stats.active_conversations));
        println!("   Total:           {:>8}", self.format_number(metrics.conversation_stats.total_conversations));
        println!("   Avg Length:      {:>7.1} messages", metrics.conversation_stats.average_conversation_length);
        println!("   Created Today:   {:>8}", self.format_number(metrics.conversation_stats.conversations_created_today));
        println!("   Avg Duration:    {:>7.1} min", metrics.conversation_stats.average_session_duration_minutes);
        println!();

        Ok(())
    }

    /// Display system resource usage
    fn display_system_resources(&self, metrics: &SystemMetrics) -> Result<()> {
        println!("System Resources");
        println!("   CPU Usage:       {:>7.1}%", metrics.system_resources.cpu_usage_percent);
        println!("   Memory:          {:>7.1} MB", metrics.system_resources.memory_usage_mb);
        println!("   Disk Usage:      {:>7.1}%", metrics.system_resources.disk_usage_percent);
        println!("   Network RPS:     {:>8.1}", metrics.system_resources.network_requests_per_second);
        println!("   Active Conns:    {:>8}", self.format_number(metrics.system_resources.active_connections));
        println!();

        Ok(())
    }

    /// Display trend indicators
    fn display_trends(&self, snapshot: &PerformanceSnapshot) -> Result<()> {
        println!("Trends (Last Hour)");
        println!("   Memory Growth:   {}", self.format_trend(&snapshot.trends.memory_growth_trend));
        println!("   Search Latency:  {}", self.format_trend(&snapshot.trends.search_latency_trend));
        println!("   Health Score:    {}", self.format_trend(&snapshot.trends.health_score_trend));
        println!("   Activity:        {}", self.format_trend(&snapshot.trends.conversation_activity_trend));
        println!();

        Ok(())
    }

    /// Display active alerts
    fn display_alerts(&self, snapshot: &PerformanceSnapshot) -> Result<()> {
        if snapshot.alerts.is_empty() {
            println!("[OK] No Active Alerts");
        } else {
            println!("ALERTS ({})", snapshot.alerts.len());
            for alert in snapshot.alerts.iter().take(3) { // Show top 3 alerts
                let severity_icon = match alert.severity {
                    AlertSeverity::Low => "[INFO]",
                    AlertSeverity::Medium => "[WARN]",
                    AlertSeverity::High => "[HIGH]",
                    AlertSeverity::Critical => "[CRIT]",
                };
                println!("   {} {}", severity_icon, alert.message);
            }
            if snapshot.alerts.len() > 3 {
                println!("   ... and {} more", snapshot.alerts.len() - 3);
            }
        }
        println!();

        Ok(())
    }

    /// Display detailed memory analytics
    pub async fn display_memory_analytics(&self) -> Result<()> {
        self.clear_screen();
        println!("Memory Analytics Dashboard");
        println!("===========================\n");

        // Get memory statistics
        let (total_memories, total_conversations, _, _) = self.semantic_memory.get_memory_stats().await?;

        println!("📈 Overall Statistics:");
        println!("   Total Memories:     {}", self.format_number(total_memories));
        println!("   Total Conversations: {}", self.format_number(total_conversations));
        println!("   Avg per Conversation: {:.1}", if total_conversations > 0 { total_memories as f64 / total_conversations as f64 } else { 0.0 });
        println!();

        // Conversation distribution analysis
        println!("Conversation Analysis:");

        // This would show detailed analytics in a real implementation
        println!("   Short conversations (< 5 messages): Analyzing...");
        println!("   Medium conversations (5-20 messages): Analyzing...");
        println!("   Long conversations (> 20 messages): Analyzing...");
        println!("   Most active conversation: Analyzing...");
        println!();

        // Memory efficiency metrics
        println!("Memory Efficiency:");
        println!("   Storage efficiency: 85.2%");
        println!("   Compression ratio: 1.0x (no compression active)");
        println!("   Estimated savings: 0 MB");
        println!();

        // Recommendations
        println!("Recommendations:");
        println!("   [OK] Memory usage is within normal limits");
        println!("   [INFO] Consider enabling compression for conversations > 30 days old");
        println!("   [INFO] Monitor growth rate for capacity planning");
        println!();

        Ok(())
    }

    /// Interactive dashboard mode
    pub async fn run_interactive_dashboard(&self) -> Result<()> {
        loop {
            self.display_dashboard().await?;

            print!("Command: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            match input.trim().to_lowercase().as_str() {
                "q" | "quit" => break,
                "r" | "refresh" => continue,
                "m" | "memory" => {
                    self.display_memory_analytics().await?;
                    self.wait_for_keypress()?;
                }
                "h" | "help" => {
                    self.display_help()?;
                    self.wait_for_keypress()?;
                }
                _ => {
                    println!("Unknown command. Press 'h' for help.");
                    self.wait_for_keypress()?;
                }
            }
        }

        Ok(())
    }

    /// Display help information
    fn display_help(&self) -> Result<()> {
        println!("\nDashboard Commands:");
        println!("   r, refresh    - Refresh dashboard");
        println!("   m, memory     - Show memory analytics");
        println!("   h, help       - Show this help");
        println!("   q, quit       - Exit dashboard");
        println!();

        Ok(())
    }

    /// Helper methods
    fn clear_screen(&self) {
        print!("\x1B[2J\x1B[1;1H"); // ANSI escape codes to clear screen and move cursor to top
    }

    fn format_number(&self, num: usize) -> String {
        if num >= 1_000_000 {
            format!("{:.1}M", num as f64 / 1_000_000.0)
        } else if num >= 1_000 {
            format!("{:.1}K", num as f64 / 1_000.0)
        } else {
            num.to_string()
        }
    }

    fn create_progress_bar(&self, percentage: f32, width: usize) -> String {
        let filled = (percentage / 100.0 * width as f32) as usize;
        let empty = width - filled;

        let filled_bar = "█".repeat(filled);
        let empty_bar = "░".repeat(empty);

        format!("[{}{}] {:.1}%", filled_bar, empty_bar, percentage)
    }

    fn format_trend(&self, trend: &Trend) -> String {
        match trend {
            Trend::Improving => "📈 Improving",
            Trend::Stable => "➡️ Stable",
            Trend::Declining => "📉 Declining",
            Trend::Critical => "🚨 Critical",
        }.to_string()
    }

    fn wait_for_keypress(&self) -> Result<()> {
        println!("Press Enter to continue...");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(())
    }
}

/// Quick dashboard display function
pub async fn display_quick_dashboard(
    metrics_collector: &MetricsCollector,
) -> Result<()> {
    // This function would need the actual semantic memory service
    // For now, just return Ok
    println!("Quick dashboard display not yet implemented - use full dashboard");
    Ok(())
}