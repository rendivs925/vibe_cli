use crate::tools::common::ensure_args_at_least;
use crate::tools::documents::{detect_extension, read_file_bytes};
use calamine::Reader;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct DocQaTool;

impl Tool for DocQaTool {
    fn name(&self) -> &str {
        "doc_qa"
    }

    fn description(&self) -> &str {
        "Answer questions over document content"
    }

    fn usage(&self) -> &str {
        "doc_qa <path> <question>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["doc_qa docs/spec.pdf \"what is the SLA?\""]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 2, self.usage())?;
        let path = args[0];
        let question = args[1..].join(" ");

        let text = extract_text(path)?;
        if text.trim().is_empty() {
            return Ok(ToolOutput::success("(no text extracted)".to_string()));
        }

        let keywords = extract_keywords(&question);
        let mut matches = Vec::new();
        for line in text.lines() {
            let lower = line.to_lowercase();
            if keywords.iter().any(|kw| lower.contains(kw)) {
                matches.push(line.trim().to_string());
            }
            if matches.len() >= 8 {
                break;
            }
        }

        let output = if matches.is_empty() {
            "No matching lines found. Consider using read_pdf/read_docx/read_xlsx first."
                .to_string()
        } else {
            matches.join("\n")
        };

        Ok(ToolOutput::success(output))
    }
}

fn extract_text(path: &str) -> Result<String, ToolError> {
    let ext = detect_extension(path);
    match ext.as_str() {
        "pdf" => {
            pdf_extract::extract_text(path).map_err(|e| ToolError::ExecutionFailed(e.to_string()))
        }
        "docx" => {
            let bytes = read_file_bytes(path)?;
            let docx = docx_rs::read_docx(&bytes)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            let mut text = String::new();
            for child in &docx.document.children {
                if let docx_rs::DocumentChild::Paragraph(p) = child {
                    text.push_str(&p.raw_text());
                    text.push('\n');
                }
            }
            Ok(text)
        }
        "xlsx" | "xls" | "csv" => {
            let content = if ext == "csv" {
                std::fs::read_to_string(path)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            } else {
                let mut workbook = calamine::open_workbook_auto(path)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                let sheet_names = workbook.sheet_names().to_owned();
                let mut output = String::new();
                for sheet in sheet_names.iter().take(2) {
                    if let Ok(range) = workbook.worksheet_range(sheet) {
                        for row in range.rows().take(50) {
                            let line = row
                                .iter()
                                .map(|cell| cell.to_string())
                                .collect::<Vec<_>>()
                                .join(" ");
                            output.push_str(&line);
                            output.push('\n');
                        }
                    }
                }
                output
            };
            Ok(content)
        }
        _ => std::fs::read_to_string(path).map_err(|e| ToolError::ExecutionFailed(e.to_string())),
    }
}

fn extract_keywords(question: &str) -> Vec<String> {
    question
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3)
        .map(|w| w.to_lowercase())
        .collect()
}
