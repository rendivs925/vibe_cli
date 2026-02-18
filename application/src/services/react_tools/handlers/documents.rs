use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;

pub struct ReadPdfHandler;

#[async_trait]
impl ReactToolHandler for ReadPdfHandler {
    fn name(&self) -> &str {
        "read_pdf"
    }

    fn description(&self) -> &str {
        "Extract text from a PDF"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let command = build_doc_command(context, "pdf", "read_pdf");
        Ok(ToolResult::new(ReactTool::ReadPdf)
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Identify the relevant PDF and extract its text.".to_string()
    }
}

pub struct ReadDocxHandler;

#[async_trait]
impl ReactToolHandler for ReadDocxHandler {
    fn name(&self) -> &str {
        "read_docx"
    }

    fn description(&self) -> &str {
        "Extract text from a DOCX"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let command = build_doc_command(context, "docx", "read_docx");
        Ok(ToolResult::new(ReactTool::ReadDocx)
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Identify the relevant DOCX and extract its text.".to_string()
    }
}

pub struct ReadXlsxHandler;

#[async_trait]
impl ReactToolHandler for ReadXlsxHandler {
    fn name(&self) -> &str {
        "read_xlsx"
    }

    fn description(&self) -> &str {
        "Read data from an XLSX/CSV"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Investigation
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let command = if let Some(path) = find_path_with_ext(context, &["xlsx", "xls", "csv"]) {
            format!("read_xlsx {}", path)
        } else {
            "fd .xlsx$ .".to_string()
        };
        Ok(ToolResult::new(ReactTool::ReadXlsx)
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Identify the relevant spreadsheet and read its data.".to_string()
    }
}

pub struct ExtractTablesHandler;

#[async_trait]
impl ReactToolHandler for ExtractTablesHandler {
    fn name(&self) -> &str {
        "extract_tables"
    }

    fn description(&self) -> &str {
        "Extract tables from documents"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let command = if let Some(path) = find_path_with_ext(context, &["xlsx", "xls", "csv"]) {
            format!("extract_tables {}", path)
        } else {
            "fd .xlsx$ .".to_string()
        };
        Ok(ToolResult::new(ReactTool::ExtractTables)
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Extract table data from the relevant document.".to_string()
    }
}

pub struct DocQaHandler;

#[async_trait]
impl ReactToolHandler for DocQaHandler {
    fn name(&self) -> &str {
        "doc_qa"
    }

    fn description(&self) -> &str {
        "Q&A over document content"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let question = context.goal.replace('"', "\\\"");
        let path = find_path_with_ext(context, &["pdf", "docx", "xlsx", "csv"])
            .unwrap_or_else(|| "".to_string());
        let command = if path.is_empty() {
            format!("doc_qa <path> \"{}\"", question)
        } else {
            format!("doc_qa {} \"{}\"", path, question)
        };
        Ok(ToolResult::new(ReactTool::DocQa)
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Answer the question using the document content.".to_string()
    }
}

pub fn build_document_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::ReadPdf, Arc::new(ReadPdfHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ReadDocx, Arc::new(ReadDocxHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ReadXlsx, Arc::new(ReadXlsxHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ExtractTables, Arc::new(ExtractTablesHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::DocQa, Arc::new(DocQaHandler) as Arc<dyn ReactToolHandler>),
    ]
}

fn build_doc_command(context: &RetrievedContext, ext: &str, tool: &str) -> String {
    if let Some(path) = find_path_with_ext(context, &[ext]) {
        format!("{} {}", tool, path)
    } else {
        format!("fd .{}$ .", ext)
    }
}

fn find_path_with_ext(context: &RetrievedContext, exts: &[&str]) -> Option<String> {
    for haystack in [&context.latest_output, &context.session_history, &context.goal] {
        for token in haystack.split_whitespace() {
            let cleaned = token.trim_matches(|c: char| c == ')' || c == ']' || c == ',' || c == '.');
            for ext in exts {
                if cleaned.to_lowercase().ends_with(&format!(".{}", ext)) {
                    return Some(cleaned.to_string());
                }
            }
        }
    }
    None
}
