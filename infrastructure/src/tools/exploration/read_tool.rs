use crate::tools::common::ensure_args_at_least;
use crate::tools::exploration::path_finder::fuzzy_find_paths;
use domain::tools::{Tool, ToolError, ToolOutput};
use std::fs;
use std::path::Path;

pub struct ReadTool;

impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read file contents with optional line window"
    }

    fn usage(&self) -> &str {
        "read <path> [lines] [offset]"
    }

    fn examples(&self) -> Vec<&str> {
        vec![
            "read src/main.rs",
            "read src/main.rs 50",
            "read src/main.rs 20 100",
        ]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let path = args[0];
        let lines: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(200);
        let offset: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);

        let (resolved_path, note) = if Path::new(path).exists() {
            (path.to_string(), None)
        } else {
            let (search_dir, query) = match path.rsplit_once('/') {
                Some((dir, file)) if !dir.trim().is_empty() => (dir, file),
                _ => (".", path),
            };
            let matches = fuzzy_find_paths(query, search_dir, 5)?;
            if matches.is_empty() {
                return Err(ToolError::NotFound(format!("file '{}' not found", path)));
            }
            if matches.len() > 1 {
                let mut output = String::new();
                output.push_str(&format!("Multiple matches for '{}':\n", path));
                for (idx, match_path) in matches.iter().enumerate() {
                    output.push_str(&format!("{}. {}\n", idx + 1, match_path));
                }
                return Ok(ToolOutput::success(output.trim_end().to_string()));
            }
            let resolved = matches[0].clone();
            let note = format!("Resolved path: {}", resolved);
            (resolved, Some(note))
        };
        let content = fs::read_to_string(&resolved_path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound(resolved_path.clone())
            } else {
                ToolError::ExecutionFailed(err.to_string())
            }
        })?;

        let mut stdout = content
            .lines()
            .skip(offset)
            .take(lines)
            .enumerate()
            .map(|(idx, line)| format!("{}: {}", offset + idx + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        if let Some(note) = note {
            stdout = format!("{note}\n{stdout}");
        }

        Ok(ToolOutput::success(stdout))
    }
}
