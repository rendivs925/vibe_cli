use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};
use std::fs;

pub struct ReplaceBlockTool;

impl Tool for ReplaceBlockTool {
    fn name(&self) -> &str {
        "replace_block"
    }

    fn description(&self) -> &str {
        "Replace exact text block in a file with diff preview"
    }

    fn usage(&self) -> &str {
        "replace_block <path> <old_block> <new_block>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["replace_block src/main.rs \"old\" \"new\""]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 3, self.usage())?;
        let path = args[0];
        let old_block = args[1];
        let new_block = args[2];

        let original = fs::read_to_string(path)
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        if !original.contains(old_block) {
            return Err(ToolError::NotFound(format!(
                "exact block not found in '{path}'"
            )));
        }

        let replaced = original.replacen(old_block, new_block, 1);
        fs::write(path, &replaced).map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let mut output = ToolOutput::success(format!(
            "Updated {path}\n{}",
            render_preview(old_block, new_block)
        ));
        output
            .metadata
            .insert("replacements".to_string(), "1".to_string());
        Ok(output)
    }
}

fn render_preview(old_text: &str, new_text: &str) -> String {
    format!(
        "--- old\n+++ new\n- {}\n+ {}",
        old_text.replace('\n', "\\n"),
        new_text.replace('\n', "\\n")
    )
}
