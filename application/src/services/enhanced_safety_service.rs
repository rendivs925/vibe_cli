//! Enhanced Safety Service
//!
//! Application service that integrates the enhanced safety kernel
//! with the existing safety infrastructure.

use domain::safety::{SafetyEngine, SafetyReport};
use infrastructure::storage::SafetyViolationStorage;
use shared::types::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Enhanced safety service with hard rules and logging
pub struct EnhancedSafetyService {
    /// Safety engine for rule evaluation
    engine: Arc<Mutex<SafetyEngine>>,
    /// Optional storage for violations
    storage: Option<SafetyViolationStorage>,
    /// Current session ID
    session_id: String,
}

impl EnhancedSafetyService {
    /// Create a new enhanced safety service
    pub fn new() -> Self {
        let session_id = format!(
            "session_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        Self {
            engine: Arc::new(Mutex::new(SafetyEngine::new())),
            storage: None,
            session_id,
        }
    }

    /// Create with storage for violations
    pub fn with_storage(storage_path: PathBuf) -> Result<Self> {
        let storage = SafetyViolationStorage::new(&storage_path)?;
        let mut service = Self::new();
        service.storage = Some(storage);
        Ok(service)
    }

    /// Analyze a command and return a detailed safety report
    pub fn analyze(&self, command: &str) -> SafetyReport {
        let mut engine = self.engine.lock().unwrap();
        engine.analyze(command)
    }

    /// Check if a command is safe to execute
    pub fn is_safe(&self, command: &str) -> bool {
        let engine = self.engine.lock().unwrap();
        engine.is_safe(command)
    }

    /// Check if a command is blocked
    pub fn is_blocked(&self, command: &str) -> bool {
        let engine = self.engine.lock().unwrap();
        engine.is_blocked(command)
    }

    /// Validate a command and optionally log the result
    pub fn validate(&self, command: &str, query: Option<&str>) -> Result<SafetyReport> {
        let report = self.analyze(command);

        // Log violations if storage is available
        if let Some(ref storage) = self.storage {
            for violation in &report.violations {
                storage.log_violation(
                    Some(&self.session_id),
                    command,
                    query,
                    violation,
                    None, // user decision not yet made
                )?;
            }
        }

        Ok(report)
    }

    /// Validate with user decision
    pub fn validate_with_decision(
        &self,
        command: &str,
        query: Option<&str>,
        user_confirmed: bool,
    ) -> Result<SafetyReport> {
        let report = self.analyze(command);

        // Log with user decision
        if let Some(ref storage) = self.storage {
            for violation in &report.violations {
                storage.log_violation(
                    Some(&self.session_id),
                    command,
                    query,
                    violation,
                    Some(user_confirmed),
                )?;
            }
        }

        Ok(report)
    }

    /// Get recent violations from storage
    pub fn get_recent_violations(&self, limit: i64) -> Result<Vec<String>> {
        if let Some(ref storage) = self.storage {
            let records = storage.get_recent_violations(limit)?;
            let formatted: Vec<String> = records
                .into_iter()
                .map(|r| {
                    format!(
                        "[{}] {} - {} ({})",
                        r.timestamp, r.rule_id, r.rule_name, r.command
                    )
                })
                .collect();
            Ok(formatted)
        } else {
            Ok(vec![])
        }
    }

    /// Get violation statistics
    pub fn get_statistics(&self) -> Result<Vec<String>> {
        if let Some(ref storage) = self.storage {
            let stats = storage.get_all_stats()?;
            let formatted: Vec<String> = stats
                .into_iter()
                .map(|s| {
                    format!(
                        "{}: {} matches ({} blocked, {} warnings) - Last: {:?}",
                        s.rule_name,
                        s.match_count,
                        s.block_count,
                        s.warning_count,
                        s.last_triggered
                    )
                })
                .collect();
            Ok(formatted)
        } else {
            Ok(vec![])
        }
    }

    /// Get formatted safety report for display
    pub fn format_report(&self, report: &SafetyReport) -> String {
        report.format_display()
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Reset violation statistics
    pub fn reset_statistics(&self) -> Result<()> {
        if let Some(ref storage) = self.storage {
            storage.clear_all_violations()?;
        }
        Ok(())
    }
}

impl Default for EnhancedSafetyService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command() {
        let service = EnhancedSafetyService::new();
        let report = service.analyze("ls -la");
        assert!(report.is_safe());
    }

    #[test]
    fn test_dangerous_command_blocked() {
        let service = EnhancedSafetyService::new();
        let report = service.analyze("rm -rf /");
        assert!(report.is_blocked());
    }

    #[test]
    fn test_warning_command() {
        let service = EnhancedSafetyService::new();
        let report = service.analyze("git push --force");
        assert!(!report.is_safe());
        assert!(!report.is_blocked()); // Warnings don't block
    }
}
