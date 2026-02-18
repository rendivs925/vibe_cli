use crate::memory::{default_memory_path, lifelong::LifelongMemoryStore};
use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct LearnPatternsTool;

impl Tool for LearnPatternsTool {
    fn name(&self) -> &str {
        "learn_patterns"
    }

    fn description(&self) -> &str {
        "Store reusable patterns with confidence"
    }

    fn usage(&self) -> &str {
        "learn_patterns <pattern> [success_count] [failure_count] [confidence]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["learn_patterns \"restart nginx on 502\" 3 1 0.75"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let mut parts: Vec<&str> = args.to_vec();
        let mut confidence = 0.6_f32;
        let mut failure_count = 0_i32;
        let mut success_count = 1_i32;

        if let Some(last) = parts.last().and_then(|v| v.parse::<f32>().ok()) {
            confidence = last;
            parts.pop();
        }
        if let Some(last) = parts.last().and_then(|v| v.parse::<i32>().ok()) {
            failure_count = last;
            parts.pop();
        }
        if let Some(last) = parts.last().and_then(|v| v.parse::<i32>().ok()) {
            success_count = last;
            parts.pop();
        }

        let pattern = parts.join(" ");

        let store = LifelongMemoryStore::new(default_memory_path())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let id = store
            .add_pattern(&pattern, success_count, failure_count, confidence)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolOutput::success(format!("Stored pattern id {}", id)))
    }
}
