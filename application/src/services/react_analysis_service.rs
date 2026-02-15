use domain::entities::{
    Constraint, Fact, Hypothesis, Insight, QueryIntent, TaskType, ToolCategory,
};

pub struct AnalysisService;

impl AnalysisService {
    pub fn new() -> Self {
        Self
    }

    pub fn infer_intent(&self, query: &str) -> QueryIntent {
        let lower = query.to_ascii_lowercase();
        let task_type = if contains_any(&lower, &["explain", "document", "describe", "overview"]) {
            TaskType::Explain
        } else if contains_any(&lower, &["monitor", "watch", "status", "health", "check"])
        {
            TaskType::Monitor
        } else if contains_any(&lower, &["configure", "setup", "enable", "install"]) {
            TaskType::Configure
        } else if contains_any(&lower, &["fix", "repair", "resolve", "remediate"]) {
            TaskType::Fix
        } else if contains_any(&lower, &["debug", "issue", "error", "slow", "failed", "not working"])
        {
            TaskType::Debug
        } else {
            TaskType::Explore
        };

        let target = detect_target(&lower);
        let constraints = self
            .extract_constraints(query)
            .into_iter()
            .map(|c| c.value)
            .collect::<Vec<_>>();
        let tool_categories = detect_tool_categories(&lower);

        let confidence = if target.is_some() {
            0.8
        } else if !tool_categories.is_empty() {
            0.6
        } else {
            0.5
        };

        QueryIntent::new(task_type, target, constraints, tool_categories, confidence)
    }

    pub fn extract_constraints(&self, text: &str) -> Vec<Constraint> {
        let lower = text.to_ascii_lowercase();
        let mut constraints = Vec::new();

        if lower.contains("production") || lower.contains("prod") {
            constraints.push(Constraint::new(
                "environment".to_string(),
                "production".to_string(),
                "user input".to_string(),
            ));
        }
        if lower.contains("staging") {
            constraints.push(Constraint::new(
                "environment".to_string(),
                "staging".to_string(),
                "user input".to_string(),
            ));
        }
        if lower.contains("development") || lower.contains("dev") {
            constraints.push(Constraint::new(
                "environment".to_string(),
                "development".to_string(),
                "user input".to_string(),
            ));
        }
        if lower.contains("read-only")
            || lower.contains("readonly")
            || lower.contains("no changes")
        {
            constraints.push(Constraint::new(
                "mode".to_string(),
                "read-only".to_string(),
                "user input".to_string(),
            ));
        }
        if lower.contains("no restart") {
            constraints.push(Constraint::new(
                "restriction".to_string(),
                "no_restart".to_string(),
                "user input".to_string(),
            ));
        }
        if lower.contains("no sudo") {
            constraints.push(Constraint::new(
                "restriction".to_string(),
                "no_sudo".to_string(),
                "user input".to_string(),
            ));
        }

        constraints
    }

    pub fn extract_facts_from_output(
        &self,
        output: &str,
        source_command: &str,
        source_step: usize,
    ) -> Vec<Fact> {
        let mut facts = Vec::new();

        for line in output.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let lower = line.to_ascii_lowercase();

            if lower.contains("mem") || lower.contains("memory") {
                if let Some(percent) = extract_percentage(line) {
                    facts.push(Fact::new(
                        "memory_usage".to_string(),
                        format!("{}%", percent),
                        source_command.to_string(),
                        source_step,
                        true,
                    ));
                }
            }

            if lower.contains("disk") || lower.contains("filesystem") || lower.contains("/" ) {
                if let Some(percent) = extract_percentage(line) {
                    facts.push(Fact::new(
                        "disk_usage".to_string(),
                        format!("{}%", percent),
                        source_command.to_string(),
                        source_step,
                        true,
                    ));
                }
            }

            if lower.contains("cpu") {
                if let Some(percent) = extract_percentage(line) {
                    facts.push(Fact::new(
                        "cpu_usage".to_string(),
                        format!("{}%", percent),
                        source_command.to_string(),
                        source_step,
                        true,
                    ));
                }
            }

            if lower.contains("load average") {
                if let Some(load) = extract_load_average(line) {
                    facts.push(Fact::new(
                        "load_avg_1".to_string(),
                        load,
                        source_command.to_string(),
                        source_step,
                        true,
                    ));
                }
            }

            if lower.contains("response") || lower.contains("latency") || lower.contains("time") {
                if let Some(ms) = extract_response_time_ms(line) {
                    facts.push(Fact::new(
                        "response_time_ms".to_string(),
                        ms,
                        source_command.to_string(),
                        source_step,
                        true,
                    ));
                }
            }
        }

        facts
    }

    pub fn extract_hypotheses_from_reasoning(&self, reasoning: &str) -> Vec<Hypothesis> {
        let lower = reasoning.to_ascii_lowercase();
        if contains_any(&lower, &["likely", "suspect", "probably", "seems"]) {
            return vec![Hypothesis::new(
                reasoning.trim().to_string(),
                0.6,
                Vec::new(),
            )];
        }
        Vec::new()
    }

    pub fn extract_insights_from_reasoning(&self, reasoning: &str) -> Vec<Insight> {
        let lower = reasoning.to_ascii_lowercase();
        if contains_any(&lower, &["next", "recommend", "suggest", "should"])
            && reasoning.trim().len() >= 8
        {
            return vec![Insight::new(reasoning.trim().to_string(), 0.5)];
        }
        Vec::new()
    }
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

fn detect_target(lower: &str) -> Option<String> {
    let services = [
        "nginx", "apache", "httpd", "mysql", "postgres", "postgresql", "redis", "docker",
        "ssh", "systemd",
    ];
    services
        .iter()
        .find(|svc| lower.contains(*svc))
        .map(|svc| svc.to_string())
}

fn detect_tool_categories(lower: &str) -> Vec<ToolCategory> {
    let mut categories = Vec::new();
    if contains_any(lower, &["process", "cpu", "pid", "top", "ps"]) {
        categories.push(ToolCategory::Process);
    }
    if contains_any(lower, &["network", "port", "http", "curl", "dns", "tcp", "udp"]) {
        categories.push(ToolCategory::Network);
    }
    if contains_any(lower, &["file", "disk", "filesystem", "mount", "inode"]) {
        categories.push(ToolCategory::Filesystem);
    }
    if contains_any(lower, &["service", "systemd", "daemon", "nginx", "redis", "mysql"]) {
        categories.push(ToolCategory::Service);
    }
    if contains_any(lower, &["log", "journal", "trace", "stack"]) {
        categories.push(ToolCategory::Logs);
    }
    if contains_any(lower, &["package", "install", "apt", "yum", "dnf", "apk"]) {
        categories.push(ToolCategory::Package);
    }
    if contains_any(lower, &["git", "branch", "commit", "merge"]) {
        categories.push(ToolCategory::Git);
    }
    if contains_any(lower, &["build", "compile", "cargo", "make", "ci"]) {
        categories.push(ToolCategory::Build);
    }
    if categories.is_empty() {
        categories.push(ToolCategory::Shell);
    }
    categories
}

fn extract_percentage(line: &str) -> Option<u32> {
    let bytes = line.as_bytes();
    for (idx, &b) in bytes.iter().enumerate() {
        if b == b'%' {
            let mut start = idx;
            while start > 0 && bytes[start - 1].is_ascii_digit() {
                start -= 1;
            }
            if start < idx {
                if let Ok(value) = line[start..idx].trim().parse::<u32>() {
                    return Some(value.min(100));
                }
            }
        }
    }
    None
}

fn extract_load_average(line: &str) -> Option<String> {
    if let Some(pos) = line.to_ascii_lowercase().find("load average") {
        let tail = &line[pos..];
        if let Some(colon) = tail.find(':') {
            let values = tail[colon + 1..].trim();
            let first = values.split(',').next()?.trim();
            if !first.is_empty() {
                return Some(first.to_string());
            }
        }
    }
    None
}

fn extract_response_time_ms(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    if let Some(pos) = lower.find("ms") {
        let value = extract_number_before(line, pos)?;
        return Some(value);
    }
    if let Some(pos) = lower.find(" s") {
        let value = extract_number_before(line, pos)?;
        if let Ok(seconds) = value.parse::<f32>() {
            return Some(format!("{:.0}", seconds * 1000.0));
        }
    }
    None
}

fn extract_number_before(line: &str, pos: usize) -> Option<String> {
    let bytes = line.as_bytes();
    let mut start = pos;
    while start > 0 {
        let ch = bytes[start - 1];
        if ch.is_ascii_digit() || ch == b'.' {
            start -= 1;
        } else if ch == b' ' {
            start -= 1;
            break;
        } else {
            break;
        }
    }
    if start < pos {
        let value = line[start..pos].trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}
