use crate::tools::common::ensure_args_at_least;
use crate::session_indexing_service::SessionIndexingService;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct FindPatternsTool;

impl Tool for FindPatternsTool {
    fn name(&self) -> &str {
        "find_patterns"
    }

    fn description(&self) -> &str {
        "Find learned patterns from memory"
    }

    fn usage(&self) -> &str {
        "find_patterns <query> [limit]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["find_patterns \"disk full\" 5"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let (query, limit) = parse_query_limit(args);

        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let output = rt.block_on(async {
            let service = SessionIndexingService::new().await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            let results = service
                .find_patterns(&query, Some(limit), None)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            if results.is_empty() {
                return Ok::<_, ToolError>("No patterns found.".to_string());
            }

            let mut lines = Vec::new();
            for (idx, (pattern_id, pattern_text, similarity, confidence)) in results.iter().enumerate() {
                let sim = (similarity * 100.0) as i32;
                let conf = (confidence * 100.0) as i32;
                lines.push(format!(
                    "{}. {} ({}% match, {}% confidence) - {}",
                    idx + 1,
                    pattern_text,
                    sim,
                    conf,
                    pattern_id
                ));
            }

            Ok::<_, ToolError>(lines.join("\n"))
        })?;

        Ok(ToolOutput::success(output))
    }
}

fn parse_query_limit(args: &[&str]) -> (String, usize) {
    let mut limit = 5_usize;
    let mut parts = args.to_vec();
    if let Some(last) = args.last().and_then(|v| v.parse::<usize>().ok()) {
        limit = last.max(1).min(10);
        parts.pop();
    }
    (parts.join(" "), limit)
}
