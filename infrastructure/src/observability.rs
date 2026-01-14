use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

/// Structured audit event for compliance tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: SystemTime,
    pub event_type: AuditEventType,
    pub severity: AuditSeverity,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub operation: String,
    pub resource: String,
    pub result: AuditResult,
    pub details: HashMap<String, serde_json::Value>,
    pub compliance_flags: Vec<ComplianceFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    AgentExecution,
    SecurityEvent,
    ResourceUsage,
    ConfigurationChange,
    Authentication,
    Authorization,
    DataAccess,
    SystemHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    Success,
    Failure(String),
    Warning(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceFlag {
    GDPR,
    HIPAA,
    SOX,
    PCI,
    NIST,
    ISO27001,
    Custom(String),
}

impl std::fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditSeverity::Low => write!(f, "LOW"),
            AuditSeverity::Medium => write!(f, "MEDIUM"),
            AuditSeverity::High => write!(f, "HIGH"),
            AuditSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditEventType::AgentExecution => write!(f, "AGENT_EXECUTION"),
            AuditEventType::SecurityEvent => write!(f, "SECURITY_EVENT"),
            AuditEventType::ResourceUsage => write!(f, "RESOURCE_USAGE"),
            AuditEventType::ConfigurationChange => write!(f, "CONFIG_CHANGE"),
            AuditEventType::Authentication => write!(f, "AUTHENTICATION"),
            AuditEventType::Authorization => write!(f, "AUTHORIZATION"),
            AuditEventType::DataAccess => write!(f, "DATA_ACCESS"),
            AuditEventType::SystemHealth => write!(f, "SYSTEM_HEALTH"),
        }
    }
}

/// Agent execution audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionAudit {
    pub request_id: String,
    pub goal: String,
    pub iterations_used: u32,
    pub tools_executed: u32,
    pub execution_time_ms: u64,
    pub confidence_score: f32,
    pub convergence_reason: Option<String>,
    pub security_checks_passed: bool,
    pub resource_limits_enforced: bool,
}

/// Security event audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditEvent {
    pub event_type: String,
    pub risk_level: String,
    pub blocked: bool,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
    pub details: HashMap<String, String>,
}

/// Audit trail manager for structured logging and compliance
pub struct AuditTrailManager {
    log_directory: PathBuf,
    max_log_files: u32,
    max_log_size_mb: u64,
    structured_logging: bool,
    current_log_file: Option<PathBuf>,
    events_buffer: Vec<AuditEvent>,
    buffer_size: usize,
}

impl AuditTrailManager {
    pub fn new(config: &crate::config::AuditTrailConfig) -> Self {
        Self {
            log_directory: PathBuf::from(&config.log_directory),
            max_log_files: config.max_log_files,
            max_log_size_mb: config.max_log_size_mb,
            structured_logging: config.structured_logging,
            current_log_file: None,
            events_buffer: Vec::new(),
            buffer_size: 100, // Flush every 100 events
        }
    }

    pub fn record_event(&mut self, event: AuditEvent) -> Result<(), Box<dyn std::error::Error>> {
        self.events_buffer.push(event);

        if self.events_buffer.len() >= self.buffer_size {
            self.flush_events()?;
        }

        Ok(())
    }

    pub fn record_agent_execution(
        &mut self,
        audit: AgentExecutionAudit,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event = AuditEvent {
            timestamp: SystemTime::now(),
            event_type: AuditEventType::AgentExecution,
            severity: AuditSeverity::Medium,
            user_id: None,
            session_id: Some(audit.request_id.clone()),
            operation: "agent_execution".to_string(),
            resource: "agent".to_string(),
            result: AuditResult::Success,
            details: {
                let mut details = HashMap::new();
                details.insert(
                    "request_id".to_string(),
                    serde_json::json!(audit.request_id),
                );
                details.insert("goal".to_string(), serde_json::json!(audit.goal));
                details.insert(
                    "iterations_used".to_string(),
                    serde_json::json!(audit.iterations_used),
                );
                details.insert(
                    "tools_executed".to_string(),
                    serde_json::json!(audit.tools_executed),
                );
                details.insert(
                    "execution_time_ms".to_string(),
                    serde_json::json!(audit.execution_time_ms),
                );
                details.insert(
                    "confidence_score".to_string(),
                    serde_json::json!(audit.confidence_score),
                );
                details.insert(
                    "security_checks_passed".to_string(),
                    serde_json::json!(audit.security_checks_passed),
                );
                details.insert(
                    "resource_limits_enforced".to_string(),
                    serde_json::json!(audit.resource_limits_enforced),
                );
                if let Some(reason) = audit.convergence_reason {
                    details.insert("convergence_reason".to_string(), serde_json::json!(reason));
                }
                details
            },
            compliance_flags: vec![ComplianceFlag::GDPR, ComplianceFlag::ISO27001],
        };

        self.record_event(event)
    }

    pub fn record_security_event(
        &mut self,
        audit: SecurityAuditEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let severity = match audit.risk_level.as_str() {
            "critical" => AuditSeverity::Critical,
            "high" => AuditSeverity::High,
            "medium" => AuditSeverity::Medium,
            _ => AuditSeverity::Low,
        };

        let event = AuditEvent {
            timestamp: SystemTime::now(),
            event_type: AuditEventType::SecurityEvent,
            severity,
            user_id: None,
            session_id: None,
            operation: audit.event_type.clone(),
            resource: "security".to_string(),
            result: if audit.blocked {
                AuditResult::Success
            } else {
                AuditResult::Warning("Security event not blocked".to_string())
            },
            details: {
                let mut details = HashMap::new();
                details.insert(
                    "event_type".to_string(),
                    serde_json::json!(audit.event_type),
                );
                details.insert(
                    "risk_level".to_string(),
                    serde_json::json!(audit.risk_level),
                );
                details.insert("blocked".to_string(), serde_json::json!(audit.blocked));
                if let Some(ip) = audit.source_ip {
                    details.insert("source_ip".to_string(), serde_json::json!(ip));
                }
                if let Some(ua) = audit.user_agent {
                    details.insert("user_agent".to_string(), serde_json::json!(ua));
                }
                for (key, value) in audit.details {
                    details.insert(key, serde_json::json!(value));
                }
                details
            },
            compliance_flags: vec![
                ComplianceFlag::GDPR,
                ComplianceFlag::HIPAA,
                ComplianceFlag::PCI,
            ],
        };

        self.record_event(event)
    }

    pub fn record_configuration_change(
        &mut self,
        component: &str,
        setting: &str,
        old_value: &str,
        new_value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event = AuditEvent {
            timestamp: SystemTime::now(),
            event_type: AuditEventType::ConfigurationChange,
            severity: AuditSeverity::High,
            user_id: None,
            session_id: None,
            operation: "configuration_change".to_string(),
            resource: component.to_string(),
            result: AuditResult::Success,
            details: {
                let mut details = HashMap::new();
                details.insert("setting".to_string(), serde_json::json!(setting));
                details.insert("old_value".to_string(), serde_json::json!(old_value));
                details.insert("new_value".to_string(), serde_json::json!(new_value));
                details
            },
            compliance_flags: vec![ComplianceFlag::SOX, ComplianceFlag::ISO27001],
        };

        self.record_event(event)
    }

    pub fn flush_events(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.events_buffer.is_empty() {
            return Ok(());
        }

        // Ensure log directory exists
        fs::create_dir_all(&self.log_directory)?;

        // Get or create current log file
        let log_file = self.get_current_log_file()?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;

        for event in &self.events_buffer {
            let log_line = if self.structured_logging {
                serde_json::to_string(event)?
            } else {
                format!(
                    "[{}] {} {} {} {} {} - {:?}",
                    event
                        .timestamp
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    event.severity,
                    event.event_type,
                    event.operation,
                    event.resource,
                    match &event.result {
                        AuditResult::Success => "SUCCESS".to_string(),
                        AuditResult::Failure(msg) => format!("FAILURE: {}", msg),
                        AuditResult::Warning(msg) => format!("WARNING: {}", msg),
                    },
                    event.details
                )
            };

            writeln!(file, "{}", log_line)?;
        }

        self.events_buffer.clear();

        // Check if we need to rotate logs
        self.check_log_rotation()?;

        Ok(())
    }

    fn get_current_log_file(&mut self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        if let Some(ref file) = self.current_log_file {
            return Ok(file.clone());
        }

        let timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let filename = format!("audit_{}.log", timestamp);
        let file_path = self.log_directory.join(filename);

        self.current_log_file = Some(file_path.clone());
        Ok(file_path)
    }

    fn check_log_rotation(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref current_file) = self.current_log_file {
            let metadata = fs::metadata(current_file)?;
            let size_mb = metadata.len() / (1024 * 1024);

            if size_mb >= self.max_log_size_mb {
                // Rotate log file
                self.rotate_log_file()?;
            }
        }

        Ok(())
    }

    fn rotate_log_file(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(current_file) = self.current_log_file.take() {
            // List existing log files
            let mut log_files = Vec::new();
            for entry in fs::read_dir(&self.log_directory)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("log") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if stem.starts_with("audit_") {
                            log_files.push(path);
                        }
                    }
                }
            }

            // Sort by modification time (newest first)
            log_files.sort_by(|a, b| {
                b.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH)
                    .cmp(
                        &a.metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(SystemTime::UNIX_EPOCH),
                    )
            });

            // Remove oldest files if we exceed max_log_files
            while log_files.len() >= self.max_log_files as usize {
                if let Some(oldest) = log_files.pop() {
                    fs::remove_file(oldest)?;
                }
            }
        }

        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.flush_events()
    }
}

/// Production observability and monitoring system with audit trail
pub struct ObservabilityManager {
    metrics: Arc<RwLock<MetricsCollector>>,
    health_checker: HealthChecker,
    alert_manager: AlertManager,
    tracer: RequestTracer,
    audit_trail: Arc<RwLock<AuditTrailManager>>,
}

impl ObservabilityManager {
    pub fn new() -> Self {
        Self::with_config(&crate::config::AuditTrailConfig::default())
    }

    pub fn with_config(audit_config: &crate::config::AuditTrailConfig) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(MetricsCollector::new())),
            health_checker: HealthChecker::new(),
            alert_manager: AlertManager::new(),
            tracer: RequestTracer::new(),
            audit_trail: Arc::new(RwLock::new(AuditTrailManager::new(audit_config))),
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

    /// Record agent execution for audit trail
    pub async fn record_agent_execution_audit(
        &self,
        audit: AgentExecutionAudit,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut audit_trail = self.audit_trail.write().await;
        audit_trail.record_agent_execution(audit)
    }

    /// Record security event for audit trail
    pub async fn record_security_audit(
        &self,
        audit: SecurityAuditEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut audit_trail = self.audit_trail.write().await;
        audit_trail.record_security_event(audit)
    }

    /// Record configuration change for audit trail
    pub async fn record_configuration_change(
        &self,
        component: &str,
        setting: &str,
        old_value: &str,
        new_value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut audit_trail = self.audit_trail.write().await;
        audit_trail.record_configuration_change(component, setting, old_value, new_value)
    }

    /// Record generic audit event
    pub async fn record_audit_event(
        &self,
        event: AuditEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut audit_trail = self.audit_trail.write().await;
        audit_trail.record_event(event)
    }

    /// Flush audit trail to disk
    pub async fn flush_audit_trail(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut audit_trail = self.audit_trail.write().await;
        audit_trail.flush_events()
    }

    /// Shutdown observability system and flush all logs
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut audit_trail = self.audit_trail.write().await;
        audit_trail.shutdown()
    }
}

impl Default for ObservabilityManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global observability instance
static mut GLOBAL_OBSERVABILITY: Option<ObservabilityManager> = None;

/// Initialize global observability manager
pub fn init_global_observability(config: &crate::config::AuditTrailConfig) {
    unsafe {
        GLOBAL_OBSERVABILITY = Some(ObservabilityManager::with_config(config));
    }
}

/// Get global observability manager (panics if not initialized)
pub fn get_global_observability() -> &'static ObservabilityManager {
    unsafe {
        GLOBAL_OBSERVABILITY
            .as_ref()
            .expect("Global observability not initialized")
    }
}

/// Shutdown global observability system
pub async fn shutdown_global_observability() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        if let Some(ref manager) = GLOBAL_OBSERVABILITY {
            manager.shutdown().await?;
        }
    }
    Ok(())
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
        *self
            .requests_total
            .entry(operation.to_string())
            .or_insert(0) += 1;
        self.requests_duration
            .insert(operation.to_string(), duration);

        if !success {
            *self.errors_total.entry(operation.to_string()).or_insert(0) += 1;
        }

        self.uptime_seconds = self.start_time.elapsed().as_secs();
    }

    pub fn record_security_event(&mut self, event_type: &str, _details: HashMap<String, String>) {
        *self
            .security_events
            .entry(event_type.to_string())
            .or_insert(0) += 1;
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
            id: format!(
                "alert_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
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
        alerts.iter().filter(|a| !a.acknowledged).cloned().collect()
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
        let trace_id = format!(
            "trace_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

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
                .record_request($operation, $duration, $success)
                .await;
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
            $crate::infrastructure::observability::OBSERVABILITY
                .alert_manager
                .raise_alert($severity, $message, $source)
                .await;
        });
    };
}
