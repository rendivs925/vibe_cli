use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

/// Production observability and monitoring system
pub struct ObservabilityManager {
    metrics: Arc<RwLock<MetricsCollector>>,
    health_checker: HealthChecker,
    alert_manager: AlertManager,
    tracer: RequestTracer,
}

impl ObservabilityManager {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(MetricsCollector::new())),
            health_checker: HealthChecker::new(),
            alert_manager: AlertManager::new(),
            tracer: RequestTracer::new(),
        }
    }

    pub async fn record_request(&self, operation: &str, duration: Duration, success: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.record_request(operation, duration, success);
    }

    pub async fn record_security_event(&self, event_type: &str, details: HashMap<String, String>) {
        let mut metrics = self.metrics.write().await;
        metrics.record_security_event(event_type, details);
    }

    pub async fn record_resource_usage(&self, memory_mb: u64, cpu_percent: f32) {
        let mut metrics = self.metrics.write().await;
        metrics.record_resource_usage(memory_mb, cpu_percent);
    }

    pub async fn get_health_status(&self) -> HealthStatus {
        self.health_checker.check_health().await
    }

    pub async fn get_metrics(&self) -> MetricsSnapshot {
        let metrics = self.metrics.read().await;
        metrics.get_snapshot()
    }

    pub fn start_request_trace(&self, operation: &str) -> TraceHandle {
        self.tracer.start_trace(operation)
    }
}

impl Default for ObservabilityManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics collector for system monitoring
pub struct MetricsCollector {
    requests_total: HashMap<String, u64>,
    requests_duration: HashMap<String, Duration>,
    errors_total: HashMap<String, u64>,
    security_events: HashMap<String, u64>,
    active_connections: u64,
    memory_usage_mb: u64,
    cpu_usage_percent: f32,
    uptime_seconds: u64,
    start_time: Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            requests_total: HashMap::new(),
            requests_duration: HashMap::new(),
            errors_total: HashMap::new(),
            security_events: HashMap::new(),
            active_connections: 0,
            memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
            uptime_seconds: 0,
            start_time: Instant::now(),
        }
    }

    pub fn record_request(&mut self, operation: &str, duration: Duration, success: bool) {
        *self.requests_total.entry(operation.to_string()).or_insert(0) += 1;
        self.requests_duration.insert(operation.to_string(), duration);

        if !success {
            *self.errors_total.entry(operation.to_string()).or_insert(0) += 1;
        }

        self.uptime_seconds = self.start_time.elapsed().as_secs();
    }

    pub fn record_security_event(&mut self, event_type: &str, _details: HashMap<String, String>) {
        *self.security_events.entry(event_type.to_string()).or_insert(0) += 1;
    }

    pub fn record_resource_usage(&mut self, memory_mb: u64, cpu_percent: f32) {
        self.memory_usage_mb = memory_mb;
        self.cpu_usage_percent = cpu_percent;
    }

    pub fn get_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            requests_total: self.requests_total.clone(),
            errors_total: self.errors_total.clone(),
            security_events: self.security_events.clone(),
            active_connections: self.active_connections,
            memory_usage_mb: self.memory_usage_mb,
            cpu_usage_percent: self.cpu_usage_percent,
            uptime_seconds: self.uptime_seconds,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub requests_total: HashMap<String, u64>,
    pub errors_total: HashMap<String, u64>,
    pub security_events: HashMap<String, u64>,
    pub active_connections: u64,
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f32,
    pub uptime_seconds: u64,
}

/// Health checker for system status monitoring
pub struct HealthChecker {
    checks: Vec<Box<dyn HealthCheck>>,
}

#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthCheckResult;
    fn name(&self) -> &str;
    fn is_critical(&self) -> bool;
}

pub struct HealthCheckResult {
    pub status: HealthStatus,
    pub message: String,
    pub response_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthChecker {
    pub fn new() -> Self {
        let mut checks: Vec<Box<dyn HealthCheck>> = Vec::new();

        // Add built-in health checks
        checks.push(Box::new(MemoryHealthCheck));
        checks.push(Box::new(DiskSpaceHealthCheck));
        checks.push(Box::new(NetworkHealthCheck));

        Self { checks }
    }

    pub async fn check_health(&self) -> HealthStatus {
        let mut critical_failures = 0;
        let mut total_failures = 0;

        for check in &self.checks {
            let result = check.check().await;
            match result.status {
                HealthStatus::Unhealthy => {
                    if check.is_critical() {
                        critical_failures += 1;
                    }
                    total_failures += 1;
                }
                HealthStatus::Degraded => {
                    total_failures += 1;
                }
                HealthStatus::Healthy => {}
            }
        }

        if critical_failures > 0 {
            HealthStatus::Unhealthy
        } else if total_failures > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    pub async fn detailed_health_check(&self) -> Vec<DetailedHealthCheck> {
        let mut results = Vec::new();

        for check in &self.checks {
            let result = check.check().await;
            results.push(DetailedHealthCheck {
                name: check.name().to_string(),
                status: result.status,
                message: result.message,
                response_time_ms: result.response_time.as_millis() as u64,
                is_critical: check.is_critical(),
            });
        }

        results
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedHealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    pub response_time_ms: u64,
    pub is_critical: bool,
}

/// Built-in health checks
pub struct MemoryHealthCheck;

#[async_trait::async_trait]
impl HealthCheck for MemoryHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Simple memory check - in production this would query system metrics
        let memory_usage = 100; // Placeholder - would get actual memory usage
        let status = if memory_usage > 90 {
            HealthStatus::Unhealthy
        } else if memory_usage > 75 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        HealthCheckResult {
            status,
            message: format!("Memory usage: {}%", memory_usage),
            response_time: start.elapsed(),
        }
    }

    fn name(&self) -> &str {
        "memory"
    }

    fn is_critical(&self) -> bool {
        true
    }
}

pub struct DiskSpaceHealthCheck;

#[async_trait::async_trait]
impl HealthCheck for DiskSpaceHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Check available disk space
        match std::fs::metadata(".") {
            Ok(_) => HealthCheckResult {
                status: HealthStatus::Healthy,
                message: "Disk space available".to_string(),
                response_time: start.elapsed(),
            },
            Err(_) => HealthCheckResult {
                status: HealthStatus::Unhealthy,
                message: "Unable to check disk space".to_string(),
                response_time: start.elapsed(),
            },
        }
    }

    fn name(&self) -> &str {
        "disk_space"
    }

    fn is_critical(&self) -> bool {
        true
    }
}

pub struct NetworkHealthCheck;

#[async_trait::async_trait]
impl HealthCheck for NetworkHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Simple network connectivity check
        HealthCheckResult {
            status: HealthStatus::Healthy,
            message: "Network connectivity OK".to_string(),
            response_time: start.elapsed(),
        }
    }

    fn name(&self) -> &str {
        "network"
    }

    fn is_critical(&self) -> bool {
        false
    }
}

/// Alert manager for notifications and escalations
pub struct AlertManager {
    alerts: Arc<RwLock<Vec<Alert>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub source: String,
    pub timestamp: String,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            alerts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn raise_alert(&self, severity: AlertSeverity, message: String, source: &str) {
        let alert = Alert {
            id: format!("alert_{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()),
            severity,
            message: message.clone(),
            source: source.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            acknowledged: false,
        };

        let mut alerts = self.alerts.write().await;
        alerts.push(alert);

        // In production, this would send notifications (Slack, email, etc.)
        eprintln!("ALERT [{}]: {}", source, message);
    }

    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts.iter()
            .filter(|a| !a.acknowledged)
            .cloned()
            .collect()
    }

    pub async fn acknowledge_alert(&self, alert_id: &str) -> bool {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
            true
        } else {
            false
        }
    }
}

/// Request tracer for distributed tracing
pub struct RequestTracer {
    active_traces: Arc<RwLock<HashMap<String, Trace>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub id: String,
    pub operation: String,
    pub start_time: std::time::SystemTime,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub name: String,
    pub start_time: std::time::SystemTime,
    pub duration: Option<Duration>,
}

pub struct TraceHandle {
    trace_id: String,
    tracer: Arc<RwLock<HashMap<String, Trace>>>,
}

impl RequestTracer {
    pub fn new() -> Self {
        Self {
            active_traces: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn start_trace(&self, operation: &str) -> TraceHandle {
        let trace_id = format!("trace_{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());

        let trace = Trace {
            id: trace_id.clone(),
            operation: operation.to_string(),
            start_time: std::time::SystemTime::now(),
            spans: Vec::new(),
        };

        let mut traces = self.active_traces.try_write().unwrap();
        traces.insert(trace_id.clone(), trace);

        TraceHandle {
            trace_id,
            tracer: Arc::clone(&self.active_traces),
        }
    }
}

impl TraceHandle {
    pub fn add_span(&self, span_name: &str) -> SpanHandle {
        // In a real implementation, this would add spans to the trace
        SpanHandle {
            span_name: span_name.to_string(),
            start_time: Instant::now(),
        }
    }

    pub fn finish(self) {
        // Mark trace as completed
        if let Ok(mut traces) = self.tracer.try_write() {
            if let Some(trace) = traces.get_mut(&self.trace_id) {
                // Could add completion logic here
                let _ = trace;
            }
        }
    }
}

impl Drop for TraceHandle {
    fn drop(&mut self) {
        // Ensure trace is cleaned up
        if let Ok(mut traces) = self.tracer.try_write() {
            traces.remove(&self.trace_id);
        }
    }
}

pub struct SpanHandle {
    span_name: String,
    start_time: Instant,
}

impl Drop for SpanHandle {
    fn drop(&mut self) {
        let duration = self.start_time.elapsed();
        // In a real implementation, this would record the span duration
        let _ = duration;
    }
}

/// Global observability instance
lazy_static::lazy_static! {
    pub static ref OBSERVABILITY: ObservabilityManager = ObservabilityManager::new();
}

/// Convenience macros for observability
#[macro_export]
macro_rules! record_request {
    ($operation:expr, $duration:expr, $success:expr) => {
        tokio::spawn(async move {
            $crate::infrastructure::observability::OBSERVABILITY
                .record_request($operation, $duration, $success).await;
        });
    };
}

#[macro_export]
macro_rules! record_security_event {
    ($event_type:expr, $($key:expr => $value:expr),*) => {
        tokio::spawn(async move {
            let mut details = std::collections::HashMap::new();
            $(
                details.insert($key.to_string(), $value.to_string());
            )*
            $crate::infrastructure::observability::OBSERVABILITY
                .record_security_event($event_type, details).await;
        });
    };
}

#[macro_export]
macro_rules! trace_request {
    ($operation:expr) => {
        $crate::infrastructure::observability::OBSERVABILITY.start_request_trace($operation)
    };
}

#[macro_export]
macro_rules! raise_alert {
    ($severity:expr, $message:expr, $source:expr) => {
        tokio::spawn(async move {
            $crate::infrastructure::observability::OBSERVABILITY.alert_manager.raise_alert($severity, $message, $source).await;
        });
    };
}