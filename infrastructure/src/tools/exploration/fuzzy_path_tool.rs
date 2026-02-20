use crate::tools::common::ensure_args_at_least;
use crate::tools::exploration::path_finder::fuzzy_find_paths;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct FuzzyPathTool;

impl Tool for FuzzyPathTool {
    fn name(&self) -> &str {
        "fuzzy_path"
    }

    fn description(&self) -> &str {
        "Find file paths using fuzzy matching"
    }

    fn usage(&self) -> &str {
        "fuzzy_path <query> [directory] [limit]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["fuzzy_path errors.log", "fuzzy_path react.rs src 5"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let query = args[0];
        let directory = args.get(1).copied().unwrap_or(".");
        let limit = args
            .get(2)
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(10);

        let matches = fuzzy_find_paths(query, directory, limit)?;
        if matches.is_empty() {
            return Ok(ToolOutput::success(format!(
                "No matches for '{}' in {}",
                query, directory
            )));
        }

        let mut output = String::new();
        output.push_str(&format!(
            "Found {} match(es) for '{}' in {}:\n",
            matches.len(),
            query,
            directory
        ));
        for (idx, path) in matches.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", idx + 1, path));
        }
        Ok(ToolOutput::success(output.trim_end().to_string()))
    }
}
