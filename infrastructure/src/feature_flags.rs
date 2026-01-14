use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Feature flag system for safe deployments and rollbacks
pub struct FeatureFlagManager {
    flags: Arc<RwLock<HashMap<String, FeatureFlag>>>,
    rollout_strategies: Arc<RwLock<HashMap<String, RolloutStrategy>>>,
    emergency_switches: Arc<RwLock<HashMap<String, EmergencySwitch>>>,
    audit_logger: FeatureAuditLogger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub rollout_percentage: f32, // 0.0 to 1.0
    pub user_whitelist: Vec<String>,
    pub user_blacklist: Vec<String>,
    pub conditions: Vec<FeatureCondition>,
    pub metadata: HashMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureCondition {
    UserId(String),
    UserGroup(String),
    Environment(String),
    TimeRange(String, String), // start, end
    Custom(String, String),    // key, value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutStrategy {
    pub feature_name: String,
    pub strategy_type: RolloutType,
    pub current_percentage: f32,
    pub target_percentage: f32,
    pub step_size: f32,
    pub step_interval_seconds: u64,
    pub auto_advance: bool,
    pub success_criteria: Vec<SuccessCriterion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RolloutType {
    PercentageBased,
    UserBased,
    TimeBased,
    Canary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessCriterion {
    pub metric_name: String,
    pub threshold: f64,
    pub operator: ComparisonOperator,
    pub time_window_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencySwitch {
    pub feature_name: String,
    pub triggered: bool,
    pub trigger_reason: Option<String>,
    pub triggered_at: Option<String>,
    pub auto_rollback: bool,
    pub rollback_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FeatureContext {
    pub user_id: Option<String>,
    pub user_groups: Vec<String>,
    pub environment: String,
    pub custom_properties: HashMap<String, String>,
}

impl FeatureFlagManager {
    pub fn new() -> Self {
        let mut flags = HashMap::new();
        let mut emergency_switches = HashMap::new();

        // Initialize default feature flags
        flags.insert(
            "safe_tools".to_string(),
            FeatureFlag {
                name: "safe_tools".to_string(),
                description: "Enable safe tool execution system".to_string(),
                enabled: true,
                rollout_percentage: 1.0,
                user_whitelist: vec![],
                user_blacklist: vec![],
                conditions: vec![],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
        );

        flags.insert(
            "agent_control".to_string(),
            FeatureFlag {
                name: "agent_control".to_string(),
                description: "Enable bounded agent loops and verification".to_string(),
                enabled: false,
                rollout_percentage: 0.1, // 10% rollout
                user_whitelist: vec![],
                user_blacklist: vec![],
                conditions: vec![],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
        );

        flags.insert(
            "observability".to_string(),
            FeatureFlag {
                name: "observability".to_string(),
                description: "Enable production observability and monitoring".to_string(),
                enabled: true,
                rollout_percentage: 1.0,
                user_whitelist: vec![],
                user_blacklist: vec![],
                conditions: vec![],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
        );

        // Initialize emergency switches
        emergency_switches.insert(
            "safe_tools".to_string(),
            EmergencySwitch {
                feature_name: "safe_tools".to_string(),
                triggered: false,
                trigger_reason: None,
                triggered_at: None,
                auto_rollback: true,
                rollback_version: Some("legacy_shell".to_string()),
            },
        );

        Self {
            flags: Arc::new(RwLock::new(flags)),
            rollout_strategies: Arc::new(RwLock::new(HashMap::new())),
            emergency_switches: Arc::new(RwLock::new(emergency_switches)),
            audit_logger: FeatureAuditLogger::new(),
        }
    }

    /// Check if a feature is enabled for a given context
    pub async fn is_feature_enabled(&self, feature_name: &str, context: &FeatureContext) -> bool {
        let flags = self.flags.read().await;
        let emergency_switches = self.emergency_switches.read().await;

        // Check emergency switch first
        if let Some(emergency) = emergency_switches.get(feature_name) {
            if emergency.triggered {
                self.audit_logger
                    .log_emergency_disable(feature_name, "Emergency switch triggered");
                return false;
            }
        }

        // Check feature flag
        if let Some(flag) = flags.get(feature_name) {
            // Check user blacklist first (blacklist overrides everything)
            if let Some(user_id) = &context.user_id {
                if flag.user_blacklist.contains(user_id) {
                    return false;
                }
            }

            // Check user whitelist (whitelist overrides global enabled flag)
            if let Some(user_id) = &context.user_id {
                if flag.user_whitelist.contains(user_id) {
                    self.audit_logger
                        .log_feature_access(feature_name, user_id, true, "whitelist");
                    return true;
                }
            }

            // Check if feature is globally enabled
            if !flag.enabled {
                return false;
            }

            // Check conditions
            for condition in &flag.conditions {
                if !self.evaluate_condition(condition, context) {
                    return false;
                }
            }

            // Check rollout percentage
            if flag.rollout_percentage < 1.0 {
                if let Some(user_id) = &context.user_id {
                    let hash = self.simple_hash(user_id) as f32 / u32::MAX as f32;
                    if hash > flag.rollout_percentage {
                        return false;
                    }
                } else {
                    // No user ID, fall back to random chance
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u32;
                    let hash = self.simple_hash(&seed.to_string()) as f32 / u32::MAX as f32;
                    if hash > flag.rollout_percentage {
                        return false;
                    }
                }
            }

            // Log successful access
            self.audit_logger.log_feature_access(
                feature_name,
                context.user_id.as_deref().unwrap_or("anonymous"),
                true,
                "rollout",
            );
            true
        } else {
            false
        }
    }

    /// Enable a feature flag
    pub async fn enable_feature(
        &self,
        feature_name: &str,
        percentage: Option<f32>,
    ) -> Result<(), FeatureError> {
        let mut flags = self.flags.write().await;
        if let Some(flag) = flags.get_mut(feature_name) {
            flag.enabled = true;
            if let Some(pct) = percentage {
                flag.rollout_percentage = pct.min(1.0).max(0.0);
            }
            flag.updated_at = chrono::Utc::now().to_rfc3339();
            self.audit_logger.log_feature_change(
                feature_name,
                "enabled",
                &format!("percentage: {:?}", percentage),
            );
            Ok(())
        } else {
            Err(FeatureError::FeatureNotFound(feature_name.to_string()))
        }
    }

    /// Disable a feature flag
    pub async fn disable_feature(&self, feature_name: &str) -> Result<(), FeatureError> {
        let mut flags = self.flags.write().await;
        if let Some(flag) = flags.get_mut(feature_name) {
            flag.enabled = false;
            flag.updated_at = chrono::Utc::now().to_rfc3339();
            self.audit_logger
                .log_feature_change(feature_name, "disabled", "");
            Ok(())
        } else {
            Err(FeatureError::FeatureNotFound(feature_name.to_string()))
        }
    }

    /// Trigger emergency disable for a feature
    pub async fn trigger_emergency_disable(
        &self,
        feature_name: &str,
        reason: &str,
    ) -> Result<(), FeatureError> {
        let mut emergency_switches = self.emergency_switches.write().await;
        if let Some(switch) = emergency_switches.get_mut(feature_name) {
            switch.triggered = true;
            switch.trigger_reason = Some(reason.to_string());
            switch.triggered_at = Some(chrono::Utc::now().to_rfc3339());

            // Disable the feature flag
            self.disable_feature(feature_name).await?;

            self.audit_logger
                .log_emergency_disable(feature_name, reason);
            Ok(())
        } else {
            Err(FeatureError::FeatureNotFound(feature_name.to_string()))
        }
    }

    /// Create a rollout strategy for gradual deployment
    pub async fn create_rollout_strategy(
        &self,
        strategy: RolloutStrategy,
    ) -> Result<(), FeatureError> {
        let mut strategies = self.rollout_strategies.write().await;
        strategies.insert(strategy.feature_name.clone(), strategy);
        Ok(())
    }

    /// Advance rollout for a feature
    pub async fn advance_rollout(&self, feature_name: &str) -> Result<f32, FeatureError> {
        let mut strategies = self.rollout_strategies.write().await;
        if let Some(strategy) = strategies.get_mut(feature_name) {
            let new_percentage =
                (strategy.current_percentage + strategy.step_size).min(strategy.target_percentage);
            strategy.current_percentage = new_percentage;

            // Update the feature flag
            let mut flags = self.flags.write().await;
            if let Some(flag) = flags.get_mut(feature_name) {
                flag.rollout_percentage = new_percentage;
                flag.updated_at = chrono::Utc::now().to_rfc3339();
            }

            self.audit_logger
                .log_rollout_advance(feature_name, new_percentage);
            Ok(new_percentage)
        } else {
            Err(FeatureError::StrategyNotFound(feature_name.to_string()))
        }
    }

    /// Add user to whitelist
    pub async fn add_to_whitelist(
        &self,
        feature_name: &str,
        user_id: &str,
    ) -> Result<(), FeatureError> {
        let mut flags = self.flags.write().await;
        if let Some(flag) = flags.get_mut(feature_name) {
            if !flag.user_whitelist.contains(&user_id.to_string()) {
                flag.user_whitelist.push(user_id.to_string());
                flag.updated_at = chrono::Utc::now().to_rfc3339();
                self.audit_logger
                    .log_whitelist_change(feature_name, user_id, "added");
            }
            Ok(())
        } else {
            Err(FeatureError::FeatureNotFound(feature_name.to_string()))
        }
    }

    /// Get all feature flags
    pub async fn get_all_flags(&self) -> HashMap<String, FeatureFlag> {
        self.flags.read().await.clone()
    }

    /// Get rollout strategies
    pub async fn get_rollout_strategies(&self) -> HashMap<String, RolloutStrategy> {
        self.rollout_strategies.read().await.clone()
    }

    fn evaluate_condition(&self, condition: &FeatureCondition, context: &FeatureContext) -> bool {
        match condition {
            FeatureCondition::UserId(user_id) => context.user_id.as_ref() == Some(user_id),
            FeatureCondition::UserGroup(group) => context.user_groups.contains(group),
            FeatureCondition::Environment(env) => context.environment == *env,
            FeatureCondition::TimeRange(start, end) => self.check_time_range(start, end),
            FeatureCondition::Custom(key, value) => {
                context.custom_properties.get(key) == Some(value)
            }
        }
    }

    fn check_time_range(&self, _start: &str, _end: &str) -> bool {
        // Simplified - in production this would parse and check actual time
        true
    }

    fn simple_hash(&self, input: &str) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        hasher.finish() as u32
    }
}

/// Feature flag audit logging
pub struct FeatureAuditLogger {
    audit_entries: Arc<RwLock<Vec<FeatureAuditEntry>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureAuditEntry {
    pub timestamp: String,
    pub event_type: String,
    pub feature_name: String,
    pub details: String,
    pub user_id: Option<String>,
}

impl FeatureAuditLogger {
    pub fn new() -> Self {
        Self {
            audit_entries: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn log_feature_access(
        &self,
        feature_name: &str,
        user_id: &str,
        enabled: bool,
        reason: &str,
    ) {
        self.log_entry(
            "feature_access",
            feature_name,
            &format!(
                "user: {}, enabled: {}, reason: {}",
                user_id, enabled, reason
            ),
            Some(user_id),
        );
    }

    pub fn log_feature_change(&self, feature_name: &str, action: &str, details: &str) {
        self.log_entry(
            "feature_change",
            feature_name,
            &format!("action: {}, details: {}", action, details),
            None,
        );
    }

    pub fn log_emergency_disable(&self, feature_name: &str, reason: &str) {
        self.log_entry(
            "emergency_disable",
            feature_name,
            &format!("reason: {}", reason),
            None,
        );
    }

    pub fn log_rollout_advance(&self, feature_name: &str, new_percentage: f32) {
        self.log_entry(
            "rollout_advance",
            feature_name,
            &format!("percentage: {:.2}", new_percentage),
            None,
        );
    }

    pub fn log_whitelist_change(&self, feature_name: &str, user_id: &str, action: &str) {
        self.log_entry(
            "whitelist_change",
            feature_name,
            &format!("user: {}, action: {}", user_id, action),
            Some(user_id),
        );
    }

    fn log_entry(
        &self,
        event_type: &str,
        feature_name: &str,
        details: &str,
        user_id: Option<&str>,
    ) {
        let entry = FeatureAuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_type: event_type.to_string(),
            feature_name: feature_name.to_string(),
            details: details.to_string(),
            user_id: user_id.map(|s| s.to_string()),
        };

        // In production, this would be written to persistent storage
        if let Ok(mut entries) = self.audit_entries.try_write() {
            entries.push(entry);
        }
    }

    pub async fn get_audit_trail(&self) -> Vec<FeatureAuditEntry> {
        self.audit_entries.read().await.clone()
    }
}

/// Feature flag errors
#[derive(Debug, Clone)]
pub enum FeatureError {
    FeatureNotFound(String),
    StrategyNotFound(String),
    InvalidPercentage(f32),
    RolloutInProgress,
}

impl std::fmt::Display for FeatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeatureError::FeatureNotFound(name) => write!(f, "Feature not found: {}", name),
            FeatureError::StrategyNotFound(name) => write!(f, "Strategy not found: {}", name),
            FeatureError::InvalidPercentage(pct) => write!(f, "Invalid percentage: {}", pct),
            FeatureError::RolloutInProgress => write!(f, "Rollout already in progress"),
        }
    }
}

impl std::error::Error for FeatureError {}

/// Global feature flag manager
lazy_static::lazy_static! {
    pub static ref FEATURE_MANAGER: FeatureFlagManager = FeatureFlagManager::new();
}

/// Convenience macros for feature flags
#[macro_export]
macro_rules! check_feature {
    ($feature:expr) => {
        check_feature!($feature, anonymous)
    };
    ($feature:expr, $user_id:expr) => {{
        let context = $crate::infrastructure::feature_flags::FeatureContext {
            user_id: Some($user_id.to_string()),
            user_groups: vec![],
            environment: "production".to_string(),
            custom_properties: std::collections::HashMap::new(),
        };
        $crate::infrastructure::feature_flags::FEATURE_MANAGER
            .is_feature_enabled($feature, &context)
            .await
    }};
}

#[macro_export]
macro_rules! with_feature {
    ($feature:expr, $code:block) => {
        if check_feature!($feature).await {
            $code
        }
    };
    ($feature:expr, $user_id:expr, $code:block) => {
        if check_feature!($feature, $user_id).await {
            $code
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_feature_flag() {
        let manager = FeatureFlagManager::new();

        let context = FeatureContext {
            user_id: Some("test_user".to_string()),
            user_groups: vec![],
            environment: "test".to_string(),
            custom_properties: HashMap::new(),
        };

        // Safe tools should be enabled by default
        assert!(manager.is_feature_enabled("safe_tools", &context).await);

        // Agent control should be disabled by default
        assert!(!manager.is_feature_enabled("agent_control", &context).await);
    }

    #[tokio::test]
    async fn test_whitelist() {
        let manager = FeatureFlagManager::new();

        // Add user to whitelist for agent_control
        manager
            .add_to_whitelist("agent_control", "test_user")
            .await
            .unwrap();

        let context = FeatureContext {
            user_id: Some("test_user".to_string()),
            user_groups: vec![],
            environment: "test".to_string(),
            custom_properties: HashMap::new(),
        };

        // Should now be enabled due to whitelist
        assert!(manager.is_feature_enabled("agent_control", &context).await);
    }

    #[tokio::test]
    async fn test_emergency_disable() {
        let manager = FeatureFlagManager::new();

        let context = FeatureContext {
            user_id: Some("test_user".to_string()),
            user_groups: vec![],
            environment: "test".to_string(),
            custom_properties: HashMap::new(),
        };

        // Initially enabled
        assert!(manager.is_feature_enabled("safe_tools", &context).await);

        // Trigger emergency disable
        manager
            .trigger_emergency_disable("safe_tools", "Test emergency")
            .await
            .unwrap();

        // Should now be disabled
        assert!(!manager.is_feature_enabled("safe_tools", &context).await);
    }

    #[tokio::test]
    async fn test_rollout_percentage() {
        let manager = FeatureFlagManager::new();

        // Enable agent_control with 50% rollout
        manager
            .enable_feature("agent_control", Some(0.5))
            .await
            .unwrap();

        let context = FeatureContext {
            user_id: Some("consistent_user".to_string()), // Use consistent user for deterministic test
            user_groups: vec![],
            environment: "test".to_string(),
            custom_properties: HashMap::new(),
        };

        // This specific user should either be enabled or disabled based on hash
        let _result = manager.is_feature_enabled("agent_control", &context).await;
        // We can't assert a specific result since it's hash-based, but we can verify it doesn't panic
    }
}
