use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};
use crate::session_indexing_service::SessionIndexingService;

pub struct SemanticSearchTool;

impl Tool for SemanticSearchTool {
    fn name(&self) -> &str {
        "semantic_search"
    }

    fn description(&self) -> &str {
        "Semantic search across past sessions"
    }

    fn usage(&self) -> &str {
        "semantic_search <query> [limit]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["semantic_search \"nginx 502\" 3"]
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
                .find_similar_sessions(&query, Some(limit))
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            if results.is_empty() {
                return Ok::<_, ToolError>("No similar sessions found.".to_string());
            }

            let mut lines = Vec::new();
            for (idx, item) in results.iter().enumerate() {
                let score = (item.similarity * 100.0) as i32;
                let summary = item.summary.as_deref().unwrap_or("(no summary)");
                lines.push(format!(
                    "{}. {} ({}% similar)\n   Session: {}\n   Summary: {}",
                    idx + 1,
                    item.goal,
                    score,
                    item.session_id,
                    summary
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
