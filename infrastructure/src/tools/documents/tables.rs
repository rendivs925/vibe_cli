use crate::tools::common::ensure_args_at_least;
use crate::tools::documents::detect_extension;
use calamine::Reader;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct ExtractTablesTool;

impl Tool for ExtractTablesTool {
    fn name(&self) -> &str {
        "extract_tables"
    }

    fn description(&self) -> &str {
        "Extract table data from XLSX or CSV"
    }

    fn usage(&self) -> &str {
        "extract_tables <path> [max_rows]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["extract_tables data.xlsx", "extract_tables data.csv 20"]
    }

    fn requires_confirmation(&self) -> bool {
        false
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        let path = args[0];
        let max_rows = args
            .get(1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(30);

        let ext = detect_extension(path);
        let output = if ext == "csv" {
            read_csv_table(path, max_rows)?
        } else {
            read_xlsx_table(path, max_rows)?
        };

        Ok(ToolOutput::success(output))
    }
}

fn read_csv_table(path: &str, max_rows: usize) -> Result<String, ToolError> {
    let content =
        std::fs::read_to_string(path).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    let mut lines = content.lines();
    let header = lines.next().unwrap_or("");
    let mut output = String::new();
    output.push_str(header);
    output.push('\n');
    for line in lines.take(max_rows) {
        output.push_str(line);
        output.push('\n');
    }
    Ok(output.trim().to_string())
}

fn read_xlsx_table(path: &str, max_rows: usize) -> Result<String, ToolError> {
    let mut workbook = calamine::open_workbook_auto(path)
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    let sheet_names = workbook.sheet_names().to_owned();
    let Some(sheet) = sheet_names.first() else {
        return Ok("(no sheets)".to_string());
    };
    let range = workbook
        .worksheet_range(sheet)
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

    let mut output = String::new();
    for row in range.rows().take(max_rows) {
        let line = row
            .iter()
            .map(|cell| cell.to_string())
            .collect::<Vec<_>>()
            .join("\t");
        output.push_str(&line);
        output.push('\n');
    }

    Ok(output.trim().to_string())
}
