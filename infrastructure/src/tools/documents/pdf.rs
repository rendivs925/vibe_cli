use crate::tools::common::ensure_args_at_least;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct ReadPdfTool;

impl Tool for ReadPdfTool {
    fn name(&self) -> &str {
        "read_pdf"
    }

    fn description(&self) -> &str {
        "Extract text from PDF"
    }

    fn usage(&self) -> &str {
        "read_pdf <path>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["read_pdf docs/spec.pdf"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let path = args[0];
        let text = pdf_extract::extract_text(path)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolOutput::success(text))
    }
}
