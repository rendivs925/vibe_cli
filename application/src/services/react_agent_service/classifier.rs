#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    Coding,
    Research,
    FileOps,
    SystemAdmin,
    General,
}

pub struct TaskClassifier;

impl TaskClassifier {
    pub fn new() -> Self {
        Self
    }

    pub fn classify(&self, query: &str) -> TaskClass {
        let lower = query.to_lowercase();
        if contains_any(&lower, &["bug", "fix", "refactor", "implement", "compile", "test", "lint"]) {
            return TaskClass::Coding;
        }
        if contains_any(&lower, &["search", "research", "paper", "article", "cite", "source", "web"]) {
            return TaskClass::Research;
        }
        if contains_any(&lower, &["file", "folder", "move", "rename", "edit", "write", "read"]) {
            return TaskClass::FileOps;
        }
        if contains_any(&lower, &["service", "systemd", "nginx", "cpu", "memory", "disk", "network"]) {
            return TaskClass::SystemAdmin;
        }
        TaskClass::General
    }
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}
