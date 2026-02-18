use crate::memory::{default_memory_path, lifelong::LifelongMemoryStore};
use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct RecallTool;

impl Tool for RecallTool {
    fn name(&self) -> &str {
        "recall"
    }

    fn description(&self) -> &str {
        "Retrieve from lifelong memory"
    }

    fn usage(&self) -> &str {
        "recall <query> [limit]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["recall \"nginx config\" 5"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let (query, limit) = parse_query_limit(args);
        let store = LifelongMemoryStore::new(default_memory_path())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let results = store
            .search(&query, limit)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        if results.is_empty() {
            return Ok(ToolOutput::success("No memory matches.".to_string()));
        }
        let mut lines = Vec::new();
        for entry in results {
            lines.push(format!("{}: {}", entry.id, entry.content));
        }
        Ok(ToolOutput::success(lines.join("\n")))
    }
}

fn parse_query_limit(args: &[&str]) -> (String, usize) {
    let mut limit = 5_usize;
    let mut parts = args.to_vec();
    if let Some(last) = args.last().and_then(|v| v.parse::<usize>().ok()) {
        limit = last.max(1).min(20);
        parts.pop();
    }
    (parts.join(" "), limit)
}
