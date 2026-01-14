use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Central policy engine for security decisions
pub struct PolicyEngine {
    policies: Arc<RwLock<Vec<SecurityPolicy>>>,
    audit_logger: PolicyAuditLogger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub conditions: Vec<PolicyCondition>,
    pub action: PolicyAction,
    pub priority: i32, // Higher numbers = higher priority
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyCondition {
    UserId(String),
    ToolName(String),
    CommandPattern(String),
    ResourceLimit(String, String), // field, operator (e.g., "memory", "> 100")
    TimeOfDay(String, String),     // start_time, end_time in HH:MM format
    NetworkAccess(bool),
    FilePath(String),
    ContainsSecrets(bool),
    RiskLevel(String), // "low", "medium", "high"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyAction {
    Allow,
    Deny(String),            // Reason for denial
    RequireApproval(String), // Approval reason
    Escalate(String),        // Escalation reason
    LogOnly,                 // Allow but log
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRequest {
    pub user_id: Option<String>,
    pub tool_name: String,
    pub parameters: HashMap<String, String>,
    pub resource_limits: ResourceLimits,
    pub contains_secrets: bool,
    pub network_access: bool,
    pub file_paths: Vec<String>,
    pub risk_assessment: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f32,
    pub max_execution_time: u64, // seconds
    pub max_output_size: usize,
    pub max_processes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub action: PolicyAction,
    pub reason: String,
    pub applied_policies: Vec<String>,
    pub audit_id: String,
}

impl PolicyEngine {
    pub fn new() -> Self {
        let mut policies = Vec::new();

        // Add default security policies
        policies.push(SecurityPolicy {
            id: "block_dangerous_commands".to_string(),
            name: "Block Dangerous Commands".to_string(),
            description: "Block potentially destructive commands".to_string(),
            conditions: vec![
                PolicyCondition::CommandPattern("rm -rf /".to_string()),
                PolicyCondition::CommandPattern("mkfs".to_string()),
                PolicyCondition::CommandPattern("dd if=".to_string()),
                PolicyCondition::CommandPattern("shutdown".to_string()),
                PolicyCondition::CommandPattern("reboot".to_string()),
            ],
            action: PolicyAction::Deny("Command contains destructive operations".to_string()),
            priority: 100,
            enabled: true,
        });

        policies.push(SecurityPolicy {
            id: "high_risk_requires_approval".to_string(),
            name: "High Risk Requires Approval".to_string(),
            description: "High-risk operations require explicit approval".to_string(),
            conditions: vec![
                PolicyCondition::RiskLevel("high".to_string()),
                PolicyCondition::RiskLevel("critical".to_string()),
            ],
            action: PolicyAction::RequireApproval("High-risk operation detected".to_string()),
            priority: 90,
            enabled: true,
        });

        policies.push(SecurityPolicy {
            id: "secrets_deny".to_string(),
            name: "Deny Operations with Secrets".to_string(),
            description: "Block operations that contain sensitive information".to_string(),
            conditions: vec![PolicyCondition::ContainsSecrets(true)],
            action: PolicyAction::Deny("Operation contains sensitive information".to_string()),
            priority: 95,
            enabled: true,
        });

        policies.push(SecurityPolicy {
            id: "resource_limits".to_string(),
            name: "Enforce Resource Limits".to_string(),
            description: "Ensure resource usage stays within safe limits".to_string(),
            conditions: vec![
                PolicyCondition::ResourceLimit("memory".to_string(), "> 1024".to_string()),
                PolicyCondition::ResourceLimit("cpu".to_string(), "> 80".to_string()),
            ],
            action: PolicyAction::Deny("Resource limits exceed safe thresholds".to_string()),
            priority: 80,
            enabled: true,
        });

        policies.push(SecurityPolicy {
            id: "network_restrictions".to_string(),
            name: "Network Access Restrictions".to_string(),
            description: "Restrict network access to approved domains only".to_string(),
            conditions: vec![PolicyCondition::NetworkAccess(true)],
            action: PolicyAction::LogOnly, // Allow but log for monitoring
            priority: 70,
            enabled: true,
        });

        policies.push(SecurityPolicy {
            id: "system_paths_protection".to_string(),
            name: "System Paths Protection".to_string(),
            description: "Protect system directories from modification".to_string(),
            conditions: vec![
                PolicyCondition::FilePath("/etc".to_string()),
                PolicyCondition::FilePath("/sys".to_string()),
                PolicyCondition::FilePath("/dev".to_string()),
                PolicyCondition::FilePath("/proc".to_string()),
                PolicyCondition::FilePath("/root".to_string()),
            ],
            action: PolicyAction::Deny("Access to system directories is not allowed".to_string()),
            priority: 85,
            enabled: true,
        });

        // Sort policies by priority (highest first)
        policies.sort_by(|a, b| b.priority.cmp(&a.priority));

        Self {
            policies: Arc::new(RwLock::new(policies)),
            audit_logger: PolicyAuditLogger::new(),
        }
    }

    /// Evaluate a policy request and return a decision
    pub async fn evaluate_request(
        &self,
        request: PolicyRequest,
    ) -> Result<PolicyDecision, PolicyError> {
        let policies = self.policies.read().await;
        let mut applied_policies = Vec::new();
        let mut decision = PolicyDecision {
            action: PolicyAction::Allow,
            reason: "Request allowed by default policy".to_string(),
            applied_policies: vec![],
            audit_id: self.audit_logger.log_request(&request),
        };

        // Evaluate each policy in priority order
        for policy in policies.iter().filter(|p| p.enabled) {
            if self.policy_matches(&request, policy) {
                applied_policies.push(policy.id.clone());

                match &policy.action {
                    PolicyAction::Deny(reason) => {
                        decision.action = PolicyAction::Deny(reason.clone());
                        decision.reason =
                            format!("Policy '{}' denied request: {}", policy.name, reason);
                        break; // Deny takes precedence
                    }
                    PolicyAction::RequireApproval(reason) => {
                        if matches!(decision.action, PolicyAction::Allow) {
                            decision.action = PolicyAction::RequireApproval(reason.clone());
                            decision.reason =
                                format!("Policy '{}' requires approval: {}", policy.name, reason);
                        }
                    }
                    PolicyAction::Escalate(reason) => {
                        if matches!(
                            decision.action,
                            PolicyAction::Allow | PolicyAction::RequireApproval(_)
                        ) {
                            decision.action = PolicyAction::Escalate(reason.clone());
                            decision.reason =
                                format!("Policy '{}' escalated request: {}", policy.name, reason);
                        }
                    }
                    PolicyAction::LogOnly => {
                        // Continue evaluating but log the match
                        decision.reason = format!("Policy '{}' logged request", policy.name);
                    }
                    PolicyAction::Allow => {
                        // Explicit allow - continue evaluating in case of deny later
                    }
                }
            }
        }

        decision.applied_policies = applied_policies;

        // Log the final decision
        self.audit_logger.log_decision(&decision);

        Ok(decision)
    }

    fn policy_matches(&self, request: &PolicyRequest, policy: &SecurityPolicy) -> bool {
        // Check if ANY condition matches (OR logic within policy)
        for condition in &policy.conditions {
            if self.condition_matches(request, condition) {
                return true;
            }
        }
        false
    }

    fn condition_matches(&self, request: &PolicyRequest, condition: &PolicyCondition) -> bool {
        match condition {
            PolicyCondition::UserId(user_id) => request.user_id.as_ref() == Some(user_id),
            PolicyCondition::ToolName(tool_name) => request.tool_name == *tool_name,
            PolicyCondition::CommandPattern(pattern) => {
                // Check if any parameter contains the pattern
                request
                    .parameters
                    .values()
                    .any(|value| value.contains(pattern))
            }
            PolicyCondition::ResourceLimit(field, limit) => {
                self.check_resource_limit(request, field, limit)
            }
            PolicyCondition::TimeOfDay(start, end) => self.check_time_of_day(start, end),
            PolicyCondition::NetworkAccess(required) => request.network_access == *required,
            PolicyCondition::FilePath(path) => {
                request.file_paths.iter().any(|fp| fp.starts_with(path))
            }
            PolicyCondition::ContainsSecrets(required) => request.contains_secrets == *required,
            PolicyCondition::RiskLevel(level) => {
                let request_level = match request.risk_assessment {
                    RiskLevel::Low => "low",
                    RiskLevel::Medium => "medium",
                    RiskLevel::High => "high",
                    RiskLevel::Critical => "critical",
                };
                request_level == level
            }
        }
    }

    fn check_resource_limit(&self, request: &PolicyRequest, field: &str, limit: &str) -> bool {
        // Parse limit like "> 100" or "< 50"
        let parts: Vec<&str> = limit.split_whitespace().collect();
        if parts.len() != 2 {
            return false;
        }

        let operator = parts[0];
        let threshold: f64 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };

        let actual_value = match field {
            "memory" => request.resource_limits.max_memory_mb as f64,
            "cpu" => request.resource_limits.max_cpu_percent as f64,
            "time" => request.resource_limits.max_execution_time as f64,
            "output" => request.resource_limits.max_output_size as f64,
            "processes" => request.resource_limits.max_processes as f64,
            _ => return false,
        };

        match operator {
            ">" => actual_value > threshold,
            "<" => actual_value < threshold,
            ">=" => actual_value >= threshold,
            "<=" => actual_value <= threshold,
            "==" => (actual_value - threshold).abs() < f64::EPSILON,
            "!=" => (actual_value - threshold).abs() >= f64::EPSILON,
            _ => false,
        }
    }

    fn check_time_of_day(&self, _start: &str, _end: &str) -> bool {
        // For now, always allow (could be enhanced to check actual time)
        true
    }

    /// Add a new policy
    pub async fn add_policy(&self, policy: SecurityPolicy) -> Result<(), PolicyError> {
        let mut policies = self.policies.write().await;
        policies.push(policy);
        // Re-sort by priority
        policies.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(())
    }

    /// Remove a policy by ID
    pub async fn remove_policy(&self, policy_id: &str) -> Result<(), PolicyError> {
        let mut policies = self.policies.write().await;
        policies.retain(|p| p.id != policy_id);
        Ok(())
    }

    /// Enable/disable a policy
    pub async fn set_policy_enabled(
        &self,
        policy_id: &str,
        enabled: bool,
    ) -> Result<(), PolicyError> {
        let mut policies = self.policies.write().await;
        if let Some(policy) = policies.iter_mut().find(|p| p.id == policy_id) {
            policy.enabled = enabled;
            Ok(())
        } else {
            Err(PolicyError::PolicyNotFound(policy_id.to_string()))
        }
    }

    /// Get all policies
    pub async fn get_policies(&self) -> Vec<SecurityPolicy> {
        self.policies.read().await.clone()
    }
}

/// Policy audit logging
pub struct PolicyAuditLogger {
    audit_entries: Arc<RwLock<Vec<PolicyAuditEntry>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAuditEntry {
    pub id: String,
    pub timestamp: String,
    pub request: PolicyRequest,
    pub decision: Option<PolicyDecision>,
}

impl PolicyAuditLogger {
    pub fn new() -> Self {
        Self {
            audit_entries: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn log_request(&self, request: &PolicyRequest) -> String {
        let id = format!(
            "audit_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

        let entry = PolicyAuditEntry {
            id: id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            request: request.clone(),
            decision: None,
        };

        // In a real implementation, this would be async and write to persistent storage
        if let Ok(mut entries) = self.audit_entries.try_write() {
            entries.push(entry);
        }

        id
    }

    pub fn log_decision(&self, decision: &PolicyDecision) {
        if let Ok(mut entries) = self.audit_entries.try_write() {
            if let Some(entry) = entries.iter_mut().find(|e| e.id == decision.audit_id) {
                entry.decision = Some(decision.clone());
            }
        }
    }

    pub async fn get_audit_trail(&self) -> Vec<PolicyAuditEntry> {
        self.audit_entries.read().await.clone()
    }
}

/// Policy engine errors
#[derive(Debug, Clone)]
pub enum PolicyError {
    PolicyNotFound(String),
    InvalidPolicy(String),
    EvaluationError(String),
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::PolicyNotFound(id) => write!(f, "Policy not found: {}", id),
            PolicyError::InvalidPolicy(msg) => write!(f, "Invalid policy: {}", msg),
            PolicyError::EvaluationError(msg) => write!(f, "Policy evaluation error: {}", msg),
        }
    }
}

impl std::error::Error for PolicyError {}

/// Integration helper for tool execution
pub async fn evaluate_tool_request(
    tool_name: &str,
    parameters: &HashMap<String, String>,
    resource_limits: &ResourceLimits,
    contains_secrets: bool,
    network_access: bool,
    file_paths: &[String],
) -> Result<PolicyDecision, PolicyError> {
    let engine = PolicyEngine::new();

    // Assess risk level based on tool and parameters
    let risk_assessment = assess_risk_level(tool_name, parameters);

    let request = PolicyRequest {
        user_id: None, // Would be set from authentication context
        tool_name: tool_name.to_string(),
        parameters: parameters.clone(),
        resource_limits: resource_limits.clone(),
        contains_secrets,
        network_access,
        file_paths: file_paths.to_vec(),
        risk_assessment,
    };

    engine.evaluate_request(request).await
}

fn assess_risk_level(tool_name: &str, parameters: &HashMap<String, String>) -> RiskLevel {
    // Simple risk assessment - could be enhanced with ML models
    match tool_name {
        "file_write" => {
            // Check for system paths
            if parameters
                .values()
                .any(|path| path.starts_with("/etc") || path.starts_with("/sys"))
            {
                RiskLevel::Critical
            } else {
                RiskLevel::Medium
            }
        }
        "process_list" => RiskLevel::Low,
        "directory_list" => RiskLevel::Low,
        "file_read" => RiskLevel::Low,
        _ => RiskLevel::Medium,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_policy_engine_basic() {
        let engine = PolicyEngine::new();

        let request = PolicyRequest {
            user_id: None,
            tool_name: "test_tool".to_string(),
            parameters: HashMap::from([("command".to_string(), "rm -rf /".to_string())]),
            resource_limits: ResourceLimits {
                max_memory_mb: 100,
                max_cpu_percent: 50.0,
                max_execution_time: 30,
                max_output_size: 1024,
                max_processes: 10,
            },
            contains_secrets: false,
            network_access: false,
            file_paths: vec![],
            risk_assessment: RiskLevel::High,
        };

        let decision = engine.evaluate_request(request).await.unwrap();

        // Should be denied due to dangerous command pattern
        match decision.action {
            PolicyAction::Deny(_) => assert!(true),
            _ => assert!(false, "Expected deny action"),
        }
    }

    #[test]
    fn test_resource_limit_check() {
        let engine = PolicyEngine::new();
        let request = PolicyRequest {
            user_id: None,
            tool_name: "test".to_string(),
            parameters: HashMap::new(),
            resource_limits: ResourceLimits {
                max_memory_mb: 2000, // Over limit
                max_cpu_percent: 50.0,
                max_execution_time: 30,
                max_output_size: 1024,
                max_processes: 10,
            },
            contains_secrets: false,
            network_access: false,
            file_paths: vec![],
            risk_assessment: RiskLevel::Low,
        };

        // Test the internal method
        let policy = SecurityPolicy {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test policy".to_string(),
            conditions: vec![PolicyCondition::ResourceLimit(
                "memory".to_string(),
                "> 1024".to_string(),
            )],
            action: PolicyAction::Deny("Test".to_string()),
            priority: 1,
            enabled: true,
        };

        assert!(engine.policy_matches(&request, &policy));
    }
}
