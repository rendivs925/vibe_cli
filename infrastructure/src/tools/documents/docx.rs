use crate::tools::common::ensure_args_at_least;
use crate::tools::documents::read_file_bytes;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct ReadDocxTool;

impl Tool for ReadDocxTool {
    fn name(&self) -> &str {
        "read_docx"
    }

    fn description(&self) -> &str {
        "Extract text from DOCX"
    }

    fn usage(&self) -> &str {
        "read_docx <path>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["read_docx docs/spec.docx"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let path = args[0];
        let bytes = read_file_bytes(path)?;
        let docx =
            docx_rs::read_docx(&bytes).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let mut text = String::new();
        for child in &docx.document.children {
            match child {
                docx_rs::DocumentChild::Paragraph(p) => {
                    text.push_str(&p.raw_text());
                    text.push('\n');
                }
                docx_rs::DocumentChild::Table(_t) => {
                    text.push_str("[Table content not extracted]\n");
                }
                _ => {}
            }
        }
        Ok(ToolOutput::success(text))
    }
}
