//! Induction Engine - Pattern detection from failures
//!
//! Mines the experience buffer to discover patterns in failures
//! and automatically generates rules to prevent them.
//!
//! Example patterns:
//! - "Every time I touch /opt/, I get Permission Denied"
//! - "Commands without sudo fail on system directories"
//! - "Docker commands fail when service is not running"

use crate::storage::experience_buffer::{ExperienceBuffer, ExperienceEntry, FailureType};
use crate::storage::knowledge_graph::{EntityType, KnowledgeGraph};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A discovered pattern from failure analysis
#[derive(Debug, Clone)]
pub struct InducedPattern {
    pub id: i64,
    pub pattern_type: PatternType,
    pub description: String,
    pub confidence: f32,
    pub occurrences: i32,
    pub example_queries: Vec<String>,
    pub example_commands: Vec<String>,
    pub induced_rule: Option<InducedRule>,
    pub discovered_at: String,
}

/// Types of patterns that can be discovered
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternType {
    PermissionRequired,
    PathPattern,
    MissingDependency,
    WrongUser,
    ServiceNotRunning,
    InvalidSyntax,
    ResourceUnavailable,
    TimingIssue,
    Other,
}

impl PatternType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PatternType::PermissionRequired => "permission_required",
            PatternType::PathPattern => "path_pattern",
            PatternType::MissingDependency => "missing_dependency",
            PatternType::WrongUser => "wrong_user",
            PatternType::ServiceNotRunning => "service_not_running",
            PatternType::InvalidSyntax => "invalid_syntax",
            PatternType::ResourceUnavailable => "resource_unavailable",
            PatternType::TimingIssue => "timing_issue",
            PatternType::Other => "other",
        }
    }
}

/// An automatically induced rule
#[derive(Debug, Clone)]
pub struct InducedRule {
    pub id: i64,
    pub pattern_id: i64,
    pub rule_name: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub confidence: f32,
    pub enabled: bool,
}

/// Condition for an induced rule
#[derive(Debug, Clone)]
pub enum RuleCondition {
    PathMatches(String),
    CommandContains(String),
    FailureType(FailureType),
    ErrorMessageContains(String),
    And(Box<RuleCondition>, Box<RuleCondition>),
    Or(Box<RuleCondition>, Box<RuleCondition>),
}

/// Action to take when rule matches
#[derive(Debug, Clone)]
pub enum RuleAction {
    AddPrefix(String),
    AddFlag(String),
    CheckService(String),
    Warn(String),
    Block(String),
    SuggestAlternative(String),
}

/// Induction engine for pattern mining
pub struct InductionEngine {
    conn: Connection,
    min_occurrences: i32,
    min_confidence: f32,
}

impl InductionEngine {
    /// Initialize engine with database path
    pub fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        let engine = Self {
            conn,
            min_occurrences: 2,
            min_confidence: 0.7,
        };
        engine.init_tables()?;
        Ok(engine)
    }

    /// Set minimum occurrences for pattern detection
    pub fn with_min_occurrences(mut self, min: i32) -> Self {
        self.min_occurrences = min;
        self
    }

    /// Set minimum confidence threshold
    pub fn with_min_confidence(mut self, min: f32) -> Self {
        self.min_confidence = min;
        self
    }

    fn init_tables(&self) -> SqliteResult<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS induced_patterns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern_type TEXT NOT NULL,
                description TEXT NOT NULL,
                confidence REAL NOT NULL,
                occurrences INTEGER NOT NULL,
                example_queries TEXT NOT NULL,
                example_commands TEXT NOT NULL,
                discovered_at TEXT NOT NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS induced_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern_id INTEGER NOT NULL,
                rule_name TEXT NOT NULL,
                condition_type TEXT NOT NULL,
                condition_value TEXT NOT NULL,
                action_type TEXT NOT NULL,
                action_value TEXT NOT NULL,
                confidence REAL NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                FOREIGN KEY (pattern_id) REFERENCES induced_patterns(id) ON DELETE CASCADE
            )",
            [],
        )?;

        Ok(())
    }

    /// Mine patterns from experience buffer
    pub fn mine_patterns(&self, buffer: &ExperienceBuffer) -> SqliteResult<Vec<InducedPattern>> {
        let mut patterns = vec![];

        // Get all failures
        let failures = self.get_all_failures(buffer)?;

        // Mine permission patterns
        if let Some(pattern) = self.mine_permission_patterns(&failures) {
            patterns.push(pattern);
        }

        // Mine path patterns
        if let Some(pattern) = self.mine_path_patterns(&failures) {
            patterns.push(pattern);
        }

        // Mine dependency patterns
        if let Some(pattern) = self.mine_dependency_patterns(&failures) {
            patterns.push(pattern);
        }

        // Mine syntax patterns
        if let Some(pattern) = self.mine_syntax_patterns(&failures) {
            patterns.push(pattern);
        }

        // Store patterns
        for pattern in &patterns {
            self.store_pattern(pattern)?;
        }

        Ok(patterns)
    }

    /// Get all failures from experience buffer
    fn get_all_failures(&self, buffer: &ExperienceBuffer) -> SqliteResult<Vec<ExperienceEntry>> {
        buffer.list_failures(1000)
    }

    /// Mine permission-related patterns
    fn mine_permission_patterns(&self, failures: &[ExperienceEntry]) -> Option<InducedPattern> {
        let permission_failures: Vec<_> = failures
            .iter()
            .filter(|f| f.failure_type == FailureType::PermissionDenied)
            .collect();

        if permission_failures.len() < self.min_occurrences as usize {
            return None;
        }

        // Check for common paths
        let mut path_counts: HashMap<String, i32> = HashMap::new();
        for failure in &permission_failures {
            if let Some(path) = self.extract_path(&failure.attempted_command) {
                *path_counts.entry(path).or_insert(0) += 1;
            }
        }

        // Find most common path
        if let Some((path, count)) = path_counts.iter().max_by_key(|(_, c)| *c) {
            if *count >= self.min_occurrences {
                let confidence = *count as f32 / permission_failures.len() as f32;
                if confidence >= self.min_confidence {
                    return Some(InducedPattern {
                        id: 0,
                        pattern_type: PatternType::PermissionRequired,
                        description: format!(
                            "Commands accessing '{}' require elevated permissions",
                            path
                        ),
                        confidence,
                        occurrences: *count,
                        example_queries: permission_failures
                            .iter()
                            .map(|f| f.query.clone())
                            .collect(),
                        example_commands: permission_failures
                            .iter()
                            .map(|f| f.attempted_command.clone())
                            .collect(),
                        induced_rule: Some(InducedRule {
                            id: 0,
                            pattern_id: 0,
                            rule_name: format!("Require sudo for {}", path),
                            condition: RuleCondition::PathMatches(path.clone()),
                            action: RuleAction::AddPrefix("sudo".to_string()),
                            confidence,
                            enabled: true,
                        }),
                        discovered_at: chrono::Utc::now().to_rfc3339(),
                    });
                }
            }
        }

        None
    }

    /// Mine path-related patterns
    fn mine_path_patterns(&self, failures: &[ExperienceEntry]) -> Option<InducedPattern> {
        let mut path_failure_counts: HashMap<String, i32> = HashMap::new();

        for failure in failures {
            if let Some(path) = self.extract_path(&failure.attempted_command) {
                *path_failure_counts.entry(path).or_insert(0) += 1;
            }
        }

        // Find paths with high failure rates
        for (path, count) in path_failure_counts {
            if count >= self.min_occurrences {
                let relevant_failures: Vec<_> = failures
                    .iter()
                    .filter(|f| {
                        self.extract_path(&f.attempted_command)
                            .map(|p| p == path)
                            .unwrap_or(false)
                    })
                    .collect();

                if relevant_failures.len() >= self.min_occurrences as usize {
                    let confidence = count as f32 / failures.len() as f32;
                    if confidence >= self.min_confidence {
                        return Some(InducedPattern {
                            id: 0,
                            pattern_type: PatternType::PathPattern,
                            description: format!("Operations on '{}' frequently fail", path),
                            confidence,
                            occurrences: count,
                            example_queries: relevant_failures
                                .iter()
                                .map(|f| f.query.clone())
                                .collect(),
                            example_commands: relevant_failures
                                .iter()
                                .map(|f| f.attempted_command.clone())
                                .collect(),
                            induced_rule: Some(InducedRule {
                                id: 0,
                                pattern_id: 0,
                                rule_name: format!("Extra caution for {}", path),
                                condition: RuleCondition::PathMatches(path.clone()),
                                action: RuleAction::Warn(format!(
                                    "Operations on {} have high failure rate",
                                    path
                                )),
                                confidence,
                                enabled: true,
                            }),
                            discovered_at: chrono::Utc::now().to_rfc3339(),
                        });
                    }
                }
            }
        }

        None
    }

    /// Mine dependency patterns
    fn mine_dependency_patterns(&self, failures: &[ExperienceEntry]) -> Option<InducedPattern> {
        // Look for "command not found" patterns
        let not_found_failures: Vec<_> = failures
            .iter()
            .filter(|f| {
                f.failure_type == FailureType::CommandNotFound
                    || f.error_message
                        .as_ref()
                        .map(|e| e.contains("not found") || e.contains("not installed"))
                        .unwrap_or(false)
            })
            .collect();

        if not_found_failures.len() < self.min_occurrences as usize {
            return None;
        }

        // Extract commands that are not found
        let mut missing_commands: HashMap<String, i32> = HashMap::new();
        for failure in &not_found_failures {
            let cmd = failure
                .attempted_command
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if !cmd.is_empty() {
                *missing_commands.entry(cmd).or_insert(0) += 1;
            }
        }

        // Find most common missing command
        if let Some((cmd, count)) = missing_commands.iter().max_by_key(|(_, c)| *c) {
            if *count >= self.min_occurrences {
                let confidence = *count as f32 / not_found_failures.len() as f32;
                return Some(InducedPattern {
                    id: 0,
                    pattern_type: PatternType::MissingDependency,
                    description: format!("'{}' is frequently not installed", cmd),
                    confidence,
                    occurrences: *count,
                    example_queries: not_found_failures.iter().map(|f| f.query.clone()).collect(),
                    example_commands: not_found_failures
                        .iter()
                        .map(|f| f.attempted_command.clone())
                        .collect(),
                    induced_rule: Some(InducedRule {
                        id: 0,
                        pattern_id: 0,
                        rule_name: format!("Check {} availability", cmd),
                        condition: RuleCondition::CommandContains(cmd.clone()),
                        action: RuleAction::SuggestAlternative(format!("Install {} first", cmd)),
                        confidence,
                        enabled: true,
                    }),
                    discovered_at: chrono::Utc::now().to_rfc3339(),
                });
            }
        }

        None
    }

    /// Mine syntax error patterns
    fn mine_syntax_patterns(&self, failures: &[ExperienceEntry]) -> Option<InducedPattern> {
        let syntax_failures: Vec<_> = failures
            .iter()
            .filter(|f| f.failure_type == FailureType::SyntaxError)
            .collect();

        if syntax_failures.len() < self.min_occurrences as usize {
            return None;
        }

        // Look for common command patterns that fail
        let mut command_patterns: HashMap<String, i32> = HashMap::new();
        for failure in &syntax_failures {
            let pattern = self.extract_command_pattern(&failure.attempted_command);
            *command_patterns.entry(pattern).or_insert(0) += 1;
        }

        if let Some((pattern, count)) = command_patterns.iter().max_by_key(|(_, c)| *c) {
            if *count >= self.min_occurrences {
                let confidence = *count as f32 / syntax_failures.len() as f32;
                if confidence >= self.min_confidence {
                    return Some(InducedPattern {
                        id: 0,
                        pattern_type: PatternType::InvalidSyntax,
                        description: format!("Pattern '{}' frequently has syntax errors", pattern),
                        confidence,
                        occurrences: *count,
                        example_queries: syntax_failures.iter().map(|f| f.query.clone()).collect(),
                        example_commands: syntax_failures
                            .iter()
                            .map(|f| f.attempted_command.clone())
                            .collect(),
                        induced_rule: Some(InducedRule {
                            id: 0,
                            pattern_id: 0,
                            rule_name: format!("Validate syntax for {}", pattern),
                            condition: RuleCondition::CommandContains(pattern.clone()),
                            action: RuleAction::Block(
                                "Syntax errors detected in similar commands".to_string(),
                            ),
                            confidence,
                            enabled: true,
                        }),
                        discovered_at: chrono::Utc::now().to_rfc3339(),
                    });
                }
            }
        }

        None
    }

    /// Extract path from command
    fn extract_path(&self, command: &str) -> Option<String> {
        // Simple heuristic: look for absolute paths
        for word in command.split_whitespace() {
            if word.starts_with('/') && word.len() > 1 {
                // Get parent directory
                let path = std::path::Path::new(word);
                if let Some(parent) = path.parent() {
                    return Some(parent.to_string_lossy().to_string());
                }
            }
        }
        None
    }

    /// Extract command pattern (first word)
    fn extract_command_pattern(&self, command: &str) -> String {
        command.split_whitespace().next().unwrap_or("").to_string()
    }

    /// Store pattern in database
    fn store_pattern(&self, pattern: &InducedPattern) -> SqliteResult<i64> {
        let example_queries = pattern.example_queries.join("\n");
        let example_commands = pattern.example_commands.join("\n");

        self.conn.execute(
            "INSERT INTO induced_patterns 
             (pattern_type, description, confidence, occurrences, 
              example_queries, example_commands, discovered_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                pattern.pattern_type.as_str(),
                pattern.description,
                pattern.confidence,
                pattern.occurrences,
                example_queries,
                example_commands,
                pattern.discovered_at,
            ],
        )?;

        let pattern_id = self.conn.last_insert_rowid();

        // Store associated rule if present
        if let Some(ref rule) = pattern.induced_rule {
            self.store_rule(pattern_id, rule)?;
        }

        Ok(pattern_id)
    }

    /// Store rule in database
    fn store_rule(&self, pattern_id: i64, rule: &InducedRule) -> SqliteResult<i64> {
        let (condition_type, condition_value) = self.serialize_condition(&rule.condition);
        let (action_type, action_value) = self.serialize_action(&rule.action);

        self.conn.execute(
            "INSERT INTO induced_rules 
             (pattern_id, rule_name, condition_type, condition_value,
              action_type, action_value, confidence, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                pattern_id,
                rule.rule_name,
                condition_type,
                condition_value,
                action_type,
                action_value,
                rule.confidence,
                rule.enabled,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    fn serialize_condition(&self, condition: &RuleCondition) -> (String, String) {
        match condition {
            RuleCondition::PathMatches(p) => ("path_matches".to_string(), p.clone()),
            RuleCondition::CommandContains(c) => ("command_contains".to_string(), c.clone()),
            RuleCondition::FailureType(f) => ("failure_type".to_string(), format!("{:?}", f)),
            RuleCondition::ErrorMessageContains(e) => ("error_contains".to_string(), e.clone()),
            _ => ("complex".to_string(), "see_rule".to_string()),
        }
    }

    fn serialize_action(&self, action: &RuleAction) -> (String, String) {
        match action {
            RuleAction::AddPrefix(p) => ("add_prefix".to_string(), p.clone()),
            RuleAction::AddFlag(f) => ("add_flag".to_string(), f.clone()),
            RuleAction::CheckService(s) => ("check_service".to_string(), s.clone()),
            RuleAction::Warn(w) => ("warn".to_string(), w.clone()),
            RuleAction::Block(b) => ("block".to_string(), b.clone()),
            RuleAction::SuggestAlternative(a) => ("suggest".to_string(), a.clone()),
        }
    }

    /// Get all discovered patterns
    pub fn get_patterns(&self) -> SqliteResult<Vec<InducedPattern>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, pattern_type, description, confidence, occurrences,
                    example_queries, example_commands, discovered_at
             FROM induced_patterns
             ORDER BY confidence DESC",
        )?;

        let patterns = stmt.query_map([], |row| {
            let type_str: String = row.get(1)?;
            let pattern_type = match type_str.as_str() {
                "permission_required" => PatternType::PermissionRequired,
                "path_pattern" => PatternType::PathPattern,
                "missing_dependency" => PatternType::MissingDependency,
                "wrong_user" => PatternType::WrongUser,
                "service_not_running" => PatternType::ServiceNotRunning,
                "invalid_syntax" => PatternType::InvalidSyntax,
                "resource_unavailable" => PatternType::ResourceUnavailable,
                "timing_issue" => PatternType::TimingIssue,
                _ => PatternType::Other,
            };

            let example_queries: Vec<String> = row
                .get::<_, String>(5)?
                .split('\n')
                .map(|s| s.to_string())
                .collect();
            let example_commands: Vec<String> = row
                .get::<_, String>(6)?
                .split('\n')
                .map(|s| s.to_string())
                .collect();

            Ok(InducedPattern {
                id: row.get(0)?,
                pattern_type,
                description: row.get(2)?,
                confidence: row.get(3)?,
                occurrences: row.get(4)?,
                example_queries,
                example_commands,
                induced_rule: None, // Would need separate query
                discovered_at: row.get(7)?,
            })
        })?;

        patterns.collect()
    }

    /// Apply induced rules to the knowledge graph as configuration entities
    pub fn apply_rules_to_graph(
        &self,
        graph: &KnowledgeGraph,
        patterns: &[InducedPattern],
    ) -> SqliteResult<usize> {
        let mut applied = 0;

        for pattern in patterns {
            if let Some(rule) = &pattern.induced_rule {
                let rule_name = format!("induced_rule:{}", rule.rule_name);
                if graph
                    .find_entity(EntityType::Configuration, &rule_name)?
                    .is_some()
                {
                    continue;
                }

                let (condition_type, condition_value) = self.serialize_condition(&rule.condition);
                let (action_type, action_value) = self.serialize_action(&rule.action);

                let mut attrs = HashMap::new();
                attrs.insert("pattern_type".to_string(), pattern.pattern_type.as_str().to_string());
                attrs.insert("description".to_string(), pattern.description.clone());
                attrs.insert("confidence".to_string(), pattern.confidence.to_string());
                attrs.insert("condition_type".to_string(), condition_type);
                attrs.insert("condition_value".to_string(), condition_value);
                attrs.insert("action_type".to_string(), action_type);
                attrs.insert("action_value".to_string(), action_value);
                attrs.insert("enabled".to_string(), rule.enabled.to_string());

                graph.add_entity(EntityType::Configuration, &rule_name, attrs)?;
                applied += 1;
            }
        }

        Ok(applied)
    }

    /// Get statistics
    pub fn stats(&self) -> SqliteResult<(usize, usize)> {
        let patterns: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM induced_patterns", [], |row| {
                    row.get(0)
                })?;

        let rules: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM induced_rules", [], |row| row.get(0))?;

        Ok((patterns as usize, rules as usize))
    }

    /// Clear all patterns and rules
    pub fn clear_all(&self) -> SqliteResult<()> {
        self.conn.execute("DELETE FROM induced_rules", [])?;
        self.conn.execute("DELETE FROM induced_patterns", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_db_path() -> PathBuf {
        PathBuf::from("/tmp/test_induction_engine.db")
    }

    #[test]
    fn test_init_tables() {
        let _ = std::fs::remove_file(test_db_path());
        let engine = InductionEngine::new(test_db_path()).unwrap();
        let (patterns, rules) = engine.stats().unwrap();
        assert_eq!(patterns, 0);
        assert_eq!(rules, 0);
        let _ = std::fs::remove_file(test_db_path());
    }

    #[test]
    fn test_extract_path() {
        let engine = InductionEngine::new(test_db_path()).unwrap();

        assert_eq!(
            engine.extract_path("ls /opt/myapp"),
            Some("/opt".to_string())
        );
        assert_eq!(
            engine.extract_path("cat /etc/nginx/nginx.conf"),
            Some("/etc/nginx".to_string())
        );
        assert_eq!(engine.extract_path("echo hello"), None);
    }

    #[test]
    fn test_extract_command_pattern() {
        let engine = InductionEngine::new(test_db_path()).unwrap();

        assert_eq!(engine.extract_command_pattern("docker ps -a"), "docker");
        assert_eq!(
            engine.extract_command_pattern("systemctl status nginx"),
            "systemctl"
        );
    }
}
