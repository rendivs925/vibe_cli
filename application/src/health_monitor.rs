//! Production Health Monitoring for Qdrant and Semantic Memory
//!
//! This module provides comprehensive health monitoring and automatic recovery
//! for Qdrant connections and semantic memory operations in production environments.

use crate::semantic_memory::{SemanticMemoryService, ConversationMemory};
use infrastructure::qdrant_storage::QdrantStorage;
use shared::types::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub overall: HealthLevel,
    pub qdrant_connected: bool,
    pub collections_available: Vec<String>,
    pub memory_stats: Option<MemoryStats>,
    pub last_check: Instant,
    pub response_time_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthLevel {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_memories: usize,
    pub total_conversations: usize,
    pub average_memories_per_conversation: f64,
    pub oldest_memory_age_days: Option<f64>,
    pub newest_memory_age_seconds: Option<f64>,
}

pub struct HealthMonitor {
    qdrant_url: String,
    semantic_memory: Option<Arc<SemanticMemoryService>>,
    check_interval: Duration,
    last_status: Option<HealthStatus>,
    failure_count: u32,
    max_failures_before_alert: u32,
}

impl HealthMonitor {
    pub fn new(qdrant_url: String, semantic_memory: Option<Arc<SemanticMemoryService>>) -> Self {
        Self {
            qdrant_url,
            semantic_memory,
            check_interval: Duration::from_secs(30), // Check every 30 seconds
            last_status: None,
            failure_count: 0,
            max_failures_before_alert: 3,
        }
    }

    /// Perform a comprehensive health check
    pub async fn check_health(&mut self) -> Result<HealthStatus> {
        let start_time = Instant::now();
        let mut status = HealthStatus {
            overall: HealthLevel::Healthy,
            qdrant_connected: false,
            collections_available: Vec::new(),
            memory_stats: None,
            last_check: start_time,
            response_time_ms: None,
        };

        // Check Qdrant connectivity
        match self.check_qdrant_connectivity().await {
            Ok((connected, collections)) => {
                status.qdrant_connected = connected;
                status.collections_available = collections;

                if !connected {
                    status.overall = HealthLevel::Critical;
                    self.failure_count += 1;
                } else {
                    self.failure_count = 0;
                }
            }
            Err(_) => {
                status.overall = HealthLevel::Critical;
                self.failure_count += 1;
            }
        }

        // Check semantic memory if available
        if let Some(memory) = &self.semantic_memory {
            match memory.get_memory_stats().await {
                Ok((total_memories, total_conversations, oldest_timestamp, newest_timestamp)) => {
                    let avg_memories = if total_conversations > 0 {
                        total_memories as f64 / total_conversations as f64
                    } else {
                        0.0
                    };

                    // Calculate age in days for oldest memory
                    let oldest_memory_age_days = oldest_timestamp.map(|ts| {
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;
                        let age_seconds = current_time - ts;
                        age_seconds as f64 / 86400.0 // Convert seconds to days
                    });

                    // Calculate age in seconds for newest memory
                    let newest_memory_age_seconds = newest_timestamp.map(|ts| {
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;
                        let age_seconds = current_time - ts;
                        age_seconds as f64
                    });

                    status.memory_stats = Some(MemoryStats {
                        total_memories,
                        total_conversations,
                        average_memories_per_conversation: avg_memories,
                        oldest_memory_age_days,
                        newest_memory_age_seconds,
                    });

                    // Degrade health if memory stats look concerning
                    if total_memories > 1_000_000 {
                        // Too many memories - potential performance issue
                        status.overall = HealthLevel::Degraded;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get memory stats: {}", e);
                    if status.overall == HealthLevel::Healthy {
                        status.overall = HealthLevel::Degraded;
                    }
                }
            }
        }

        // Check if we have critical failures
        if self.failure_count >= self.max_failures_before_alert {
            status.overall = HealthLevel::Critical;
        }

        let response_time = start_time.elapsed();
        status.response_time_ms = Some(response_time.as_millis() as u64);

        // Check response time degradation
        if response_time > Duration::from_secs(5) && status.overall == HealthLevel::Healthy {
            status.overall = HealthLevel::Degraded;
        }

        self.last_status = Some(status.clone());
        Ok(status)
    }

    /// Check Qdrant connectivity and get available collections
    async fn check_qdrant_connectivity(&self) -> Result<(bool, Vec<String>)> {
        // Try to connect to Qdrant and list collections
        let client = reqwest::Client::new();
        let url = format!("{}/collections", self.qdrant_url.trim_end_matches('/'));

        match client.get(&url).timeout(Duration::from_secs(10)).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(data) => {
                            if let Some(collections) = data.get("result").and_then(|r| r.get("collections")) {
                                let collection_names = collections.as_array()
                                    .unwrap_or(&vec![])
                                    .iter()
                                    .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
                                    .map(|s| s.to_string())
                                    .collect::<Vec<_>>();

                                Ok((true, collection_names))
                            } else {
                                Ok((true, vec![]))
                            }
                        }
                        Err(_) => Ok((false, vec![])),
                    }
                } else {
                    Ok((false, vec![]))
                }
            }
            Err(_) => Ok((false, vec![])),
        }
    }

    /// Get the last health status
    pub fn get_last_status(&self) -> Option<&HealthStatus> {
        self.last_status.as_ref()
    }

    /// Check if the system needs attention
    pub fn needs_attention(&self) -> bool {
        if let Some(status) = &self.last_status {
            matches!(status.overall, HealthLevel::Critical | HealthLevel::Unhealthy)
        } else {
            true // No status means we haven't checked yet
        }
    }

    /// Get health summary as a string
    pub fn get_health_summary(&self) -> String {
        if let Some(status) = &self.last_status {
            let level_str = match status.overall {
                HealthLevel::Healthy => "🟢 HEALTHY",
                HealthLevel::Degraded => "🟡 DEGRADED",
                HealthLevel::Unhealthy => "🟠 UNHEALTHY",
                HealthLevel::Critical => "🔴 CRITICAL",
            };

            let mut summary = format!("{} - Qdrant: {}", level_str,
                if status.qdrant_connected { "Connected" } else { "Disconnected" });

            if !status.collections_available.is_empty() {
                summary.push_str(&format!(" ({} collections)", status.collections_available.len()));
            }

            if let Some(stats) = &status.memory_stats {
                summary.push_str(&format!(" | Memories: {} in {} conversations",
                    stats.total_memories, stats.total_conversations));
            }

            if let Some(rt) = status.response_time_ms {
                summary.push_str(&format!(" | Response: {}ms", rt));
            }

            summary
        } else {
            "❓ No health check performed yet".to_string()
        }
    }

    /// Attempt automatic recovery
    pub async fn attempt_recovery(&mut self) -> Result<bool> {
        println!("🔧 Attempting automatic recovery...");

        // For now, just re-check health
        // In a real implementation, this might restart services, recreate collections, etc.
        match self.check_health().await {
            Ok(status) => {
                let recovered = matches!(status.overall, HealthLevel::Healthy | HealthLevel::Degraded);
                if recovered {
                    println!("✅ Recovery successful");
                    self.failure_count = 0;
                } else {
                    println!("❌ Recovery failed - system still unhealthy");
                }
                Ok(recovered)
            }
            Err(e) => {
                println!("❌ Recovery attempt failed: {}", e);
                Ok(false)
            }
        }
    }
}

impl std::fmt::Display for HealthLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level_str = match self {
            HealthLevel::Healthy => "Healthy",
            HealthLevel::Degraded => "Degraded",
            HealthLevel::Unhealthy => "Unhealthy",
            HealthLevel::Critical => "Critical",
        };
        write!(f, "{}", level_str)
    }
}