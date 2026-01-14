use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tokio::task;

/// Background shell activity monitoring and pattern detection
pub struct ShellMonitor {
    activity_buffer: RwLock<VecDeque<ShellActivity>>,
    pattern_detector: PatternDetector,
    expert_predictor: ExpertPredictor,
    max_buffer_size: usize,
    cleanup_interval: Duration,
    last_cleanup: RwLock<Instant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellActivity {
    pub command: String,
    pub working_directory: String,
    pub exit_code: Option<i32>,
    pub execution_time_ms: Option<u64>,
    pub timestamp: SystemTime,
    pub user: String,
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct ActivityPattern {
    pub pattern_type: PatternType,
    pub confidence: f32,
    pub description: String,
    pub suggested_actions: Vec<String>,
    pub related_experts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PatternType {
    GitWorkflow,
    RustDevelopment,
    JavaScriptDevelopment,
    BuildProcess,
    TestExecution,
    Deployment,
    Debugging,
    Documentation,
    CodeReview,
    DatabaseOperations,
    Unknown,
}

#[derive(Debug)]
struct PatternDetector {
    git_patterns: Vec<String>,
    rust_patterns: Vec<String>,
    js_patterns: Vec<String>,
    build_patterns: Vec<String>,
    test_patterns: Vec<String>,
}

#[derive(Debug)]
struct ExpertPredictor {
    pattern_expert_map: HashMap<PatternType, Vec<String>>,
    recent_patterns: RwLock<VecDeque<(PatternType, Instant)>>,
}

impl ShellMonitor {
    /// Create new shell monitor
    pub fn new() -> Self {
        let pattern_detector = PatternDetector::new();
        let expert_predictor = ExpertPredictor::new();

        Self {
            activity_buffer: RwLock::new(VecDeque::with_capacity(1000)),
            pattern_detector,
            expert_predictor,
            max_buffer_size: 1000,
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            last_cleanup: RwLock::new(Instant::now()),
        }
    }

    /// Record shell activity
    pub async fn record_activity(&self, activity: ShellActivity) -> Result<()> {
        let mut buffer = self.activity_buffer.write().await;

        // Maintain buffer size
        if buffer.len() >= self.max_buffer_size {
            buffer.pop_front();
        }

        buffer.push_back(activity);

        // Trigger pattern analysis
        if let Some(pattern) = self.analyze_recent_activity().await {
            self.expert_predictor
                .record_pattern(pattern.pattern_type)
                .await;
        }

        // Periodic cleanup
        self.periodic_cleanup().await?;

        Ok(())
    }

    /// Analyze recent activity for patterns
    async fn analyze_recent_activity(&self) -> Option<ActivityPattern> {
        let buffer = self.activity_buffer.read().await;
        let recent_activities: Vec<&ShellActivity> = buffer
            .iter()
            .rev()
            .take(10) // Last 10 activities
            .collect();

        self.pattern_detector
            .detect_pattern(&recent_activities)
            .await
    }

    /// Get current activity patterns
    pub async fn get_current_patterns(&self) -> Result<Vec<ActivityPattern>> {
        let buffer = self.activity_buffer.read().await;
        let recent_activities: Vec<&ShellActivity> = buffer
            .iter()
            .rev()
            .take(20) // Last 20 activities for pattern analysis
            .collect();

        let mut patterns = Vec::new();

        for chunk in recent_activities.chunks(5) {
            if let Some(pattern) = self.pattern_detector.detect_pattern(chunk).await {
                patterns.push(pattern);
            }
        }

        Ok(patterns)
    }

    /// Get proactive suggestions based on activity patterns
    pub async fn get_proactive_suggestions(&self) -> Result<Vec<String>> {
        let patterns = self.get_current_patterns().await?;
        let mut suggestions = Vec::new();

        for pattern in patterns {
            if pattern.confidence > 0.7 {
                suggestions.extend(pattern.suggested_actions);
            }
        }

        // Limit suggestions
        suggestions.truncate(5);
        Ok(suggestions)
    }

    /// Predict needed experts based on activity
    pub async fn predict_experts(&self) -> Result<Vec<String>> {
        let patterns = self.expert_predictor.get_recent_patterns().await;
        let mut experts = Vec::new();

        for pattern in patterns {
            if let Some(pattern_experts) = self.expert_predictor.pattern_expert_map.get(&pattern.0)
            {
                for expert in pattern_experts {
                    if !experts.contains(expert) {
                        experts.push(expert.clone());
                    }
                }
            }
        }

        Ok(experts)
    }

    /// Get activity statistics
    pub async fn get_activity_stats(&self) -> HashMap<String, String> {
        let buffer = self.activity_buffer.read().await;
        let mut stats = HashMap::new();

        let total_activities = buffer.len();
        stats.insert("total_activities".to_string(), total_activities.to_string());

        let successful_commands = buffer.iter().filter(|a| a.exit_code == Some(0)).count();
        stats.insert(
            "successful_commands".to_string(),
            successful_commands.to_string(),
        );

        let failed_commands = buffer
            .iter()
            .filter(|a| a.exit_code.is_some() && a.exit_code != Some(0))
            .count();
        stats.insert("failed_commands".to_string(), failed_commands.to_string());

        // Command frequency
        let mut command_counts = HashMap::new();
        for activity in buffer.iter() {
            let cmd_base = activity.command.split_whitespace().next().unwrap_or("");
            *command_counts.entry(cmd_base.to_string()).or_insert(0) += 1;
        }

        let mut top_commands = command_counts.into_iter().collect::<Vec<_>>();
        top_commands.sort_by(|a, b| b.1.cmp(&a.1));
        top_commands.truncate(5);

        for (i, (cmd, count)) in top_commands.into_iter().enumerate() {
            stats.insert(
                format!("top_command_{}", i + 1),
                format!("{}:{}", cmd, count),
            );
        }

        // Time-based stats
        let now = SystemTime::now();
        let last_hour = buffer
            .iter()
            .filter(|a| {
                now.duration_since(a.timestamp)
                    .unwrap_or(Duration::from_secs(0))
                    < Duration::from_secs(3600)
            })
            .count();
        stats.insert("activities_last_hour".to_string(), last_hour.to_string());

        stats
    }

    /// Periodic cleanup of old data
    async fn periodic_cleanup(&self) -> Result<()> {
        let now = Instant::now();
        let mut last_cleanup = self.last_cleanup.write().await;

        if now.duration_since(*last_cleanup) >= self.cleanup_interval {
            let mut buffer = self.activity_buffer.write().await;

            // Remove entries older than 24 hours
            let cutoff = SystemTime::now()
                .checked_sub(Duration::from_secs(86400))
                .ok_or_else(|| anyhow!("Time calculation overflow"))?; // 24 hours
            buffer.retain(|activity| activity.timestamp > cutoff);

            *last_cleanup = now;
        }

        Ok(())
    }

    /// Export activity data for analysis
    pub async fn export_activity_data(&self, hours_back: u64) -> Result<Vec<ShellActivity>> {
        let buffer = self.activity_buffer.read().await;
        let cutoff = SystemTime::now() - Duration::from_secs(hours_back * 3600);

        let activities = buffer
            .iter()
            .filter(|a| a.timestamp > cutoff)
            .cloned()
            .collect();

        Ok(activities)
    }

    /// Search activities by pattern
    pub async fn search_activities(&self, pattern: &str) -> Result<Vec<ShellActivity>> {
        let buffer = self.activity_buffer.read().await;

        let matches = buffer
            .iter()
            .filter(|a| a.command.contains(pattern))
            .cloned()
            .collect();

        Ok(matches)
    }

    /// Get workflow insights
    pub async fn get_workflow_insights(&self) -> Result<HashMap<String, String>> {
        let patterns = self.get_current_patterns().await?;
        let mut insights = HashMap::new();

        for pattern in patterns {
            insights.insert(
                format!("pattern_{:?}", pattern.pattern_type),
                format!(
                    "{} (confidence: {:.2})",
                    pattern.description, pattern.confidence
                ),
            );
        }

        // Add workflow efficiency metrics
        let stats = self.get_activity_stats().await;
        insights.extend(stats);

        Ok(insights)
    }

    /// Clear all activity data
    pub async fn clear_data(&self) {
        let mut buffer = self.activity_buffer.write().await;
        buffer.clear();
    }

    /// Start background monitoring task
    pub fn start_background_monitoring(self: Arc<Self>) -> Result<()> {
        let monitor = Arc::clone(&self);

        task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Check every minute

            loop {
                interval.tick().await;

                // Periodic cleanup and analysis
                if let Err(e) = monitor.periodic_cleanup().await {
                    eprintln!("Background cleanup failed: {}", e);
                }

                // Update pattern predictions
                if let Err(e) = monitor.expert_predictor.update_predictions().await {
                    eprintln!("Pattern prediction update failed: {}", e);
                }
            }
        });

        Ok(())
    }
}

impl PatternDetector {
    /// Create new pattern detector
    fn new() -> Self {
        Self {
            git_patterns: vec![
                "git add",
                "git commit",
                "git push",
                "git pull",
                "git merge",
                "git branch",
                "git checkout",
                "git status",
                "git log",
                "git diff",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            rust_patterns: vec![
                "cargo build",
                "cargo run",
                "cargo test",
                "cargo check",
                "cargo clippy",
                "rustc",
                "rustup",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            js_patterns: vec![
                "npm install",
                "npm run",
                "yarn install",
                "yarn build",
                "node",
                "npm test",
                "webpack",
                "babel",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            build_patterns: vec![
                "make",
                "cmake",
                "configure",
                "build",
                "compile",
                "gradle",
                "maven",
                "ant",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            test_patterns: vec![
                "test", "spec", "pytest", "jest", "mocha", "rspec", "phpunit", "junit", "testng",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        }
    }

    /// Detect patterns in recent activities
    async fn detect_pattern(&self, activities: &[&ShellActivity]) -> Option<ActivityPattern> {
        if activities.is_empty() {
            return None;
        }

        let commands: Vec<&str> = activities.iter().map(|a| a.command.as_str()).collect();

        // Check for Git workflow
        if self.matches_patterns(&commands, &self.git_patterns, 0.4) {
            return Some(ActivityPattern {
                pattern_type: PatternType::GitWorkflow,
                confidence: 0.85,
                description: "Active Git version control workflow".to_string(),
                suggested_actions: vec![
                    "Consider committing your changes".to_string(),
                    "Check git status for untracked files".to_string(),
                    "Review recent commits with git log".to_string(),
                ],
                related_experts: vec!["git".to_string(), "version-control".to_string()],
            });
        }

        // Check for Rust development
        if self.matches_patterns(&commands, &self.rust_patterns, 0.3) {
            return Some(ActivityPattern {
                pattern_type: PatternType::RustDevelopment,
                confidence: 0.9,
                description: "Rust development and compilation workflow".to_string(),
                suggested_actions: vec![
                    "Run cargo clippy for linting".to_string(),
                    "Execute cargo test to run tests".to_string(),
                    "Check for cargo outdated dependencies".to_string(),
                ],
                related_experts: vec!["rust".to_string(), "systems-programming".to_string()],
            });
        }

        // Check for JavaScript development
        if self.matches_patterns(&commands, &self.js_patterns, 0.3) {
            return Some(ActivityPattern {
                pattern_type: PatternType::JavaScriptDevelopment,
                confidence: 0.85,
                description: "JavaScript/TypeScript development workflow".to_string(),
                suggested_actions: vec![
                    "Run npm audit for security issues".to_string(),
                    "Check package.json for outdated dependencies".to_string(),
                    "Consider running build process".to_string(),
                ],
                related_experts: vec!["javascript".to_string(), "web-development".to_string()],
            });
        }

        // Check for build processes
        if self.matches_patterns(&commands, &self.build_patterns, 0.2) {
            return Some(ActivityPattern {
                pattern_type: PatternType::BuildProcess,
                confidence: 0.8,
                description: "Software build and compilation process".to_string(),
                suggested_actions: vec![
                    "Check build output for warnings".to_string(),
                    "Consider parallel build options".to_string(),
                    "Verify build artifacts".to_string(),
                ],
                related_experts: vec!["build-systems".to_string(), "compilation".to_string()],
            });
        }

        // Check for testing
        if self.matches_patterns(&commands, &self.test_patterns, 0.25) {
            return Some(ActivityPattern {
                pattern_type: PatternType::TestExecution,
                confidence: 0.8,
                description: "Test execution and validation workflow".to_string(),
                suggested_actions: vec![
                    "Review test failures and fix issues".to_string(),
                    "Check test coverage metrics".to_string(),
                    "Consider adding more test cases".to_string(),
                ],
                related_experts: vec!["testing".to_string(), "quality-assurance".to_string()],
            });
        }

        None
    }

    /// Check if commands match patterns above threshold
    fn matches_patterns(&self, commands: &[&str], patterns: &[String], threshold: f32) -> bool {
        let mut matches = 0;

        for command in commands {
            for pattern in patterns {
                if command.contains(pattern) {
                    matches += 1;
                    break; // Count each command only once
                }
            }
        }

        let match_ratio = matches as f32 / commands.len() as f32;
        match_ratio >= threshold
    }
}

impl ExpertPredictor {
    /// Create new expert predictor
    fn new() -> Self {
        let mut pattern_expert_map = HashMap::new();

        pattern_expert_map.insert(
            PatternType::GitWorkflow,
            vec![
                "git".to_string(),
                "version-control".to_string(),
                "collaboration".to_string(),
            ],
        );

        pattern_expert_map.insert(
            PatternType::RustDevelopment,
            vec![
                "rust".to_string(),
                "systems-programming".to_string(),
                "performance".to_string(),
            ],
        );

        pattern_expert_map.insert(
            PatternType::JavaScriptDevelopment,
            vec![
                "javascript".to_string(),
                "web-development".to_string(),
                "frontend".to_string(),
            ],
        );

        pattern_expert_map.insert(
            PatternType::BuildProcess,
            vec![
                "build-systems".to_string(),
                "compilation".to_string(),
                "ci-cd".to_string(),
            ],
        );

        pattern_expert_map.insert(
            PatternType::TestExecution,
            vec![
                "testing".to_string(),
                "quality-assurance".to_string(),
                "tdd".to_string(),
            ],
        );

        Self {
            pattern_expert_map,
            recent_patterns: RwLock::new(VecDeque::with_capacity(50)),
        }
    }

    /// Record detected pattern
    async fn record_pattern(&self, pattern_type: PatternType) {
        let mut patterns = self.recent_patterns.write().await;

        // Maintain pattern history
        if patterns.len() >= 50 {
            patterns.pop_front();
        }

        patterns.push_back((pattern_type, Instant::now()));
    }

    /// Get recent patterns
    async fn get_recent_patterns(&self) -> Vec<(PatternType, Instant)> {
        let patterns = self.recent_patterns.read().await;
        patterns.iter().cloned().collect()
    }

    /// Update predictions based on pattern history
    async fn update_predictions(&self) -> Result<()> {
        // Clean old patterns (older than 1 hour)
        let mut patterns_write = self.recent_patterns.write().await;
        let cutoff = Instant::now() - Duration::from_secs(3600);
        patterns_write.retain(|(_, timestamp)| *timestamp > cutoff);

        Ok(())
    }
}
