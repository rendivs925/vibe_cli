use crate::tools::common::ensure_args_at_least;
use crate::tools::documents::detect_extension;
use calamine::Reader;
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct ReadXlsxTool;

impl Tool for ReadXlsxTool {
    fn name(&self) -> &str {
        "read_xlsx"
    }

    fn description(&self) -> &str {
        "Read data from XLSX or CSV"
    }

    fn usage(&self) -> &str {
        "read_xlsx <path> [max_rows]"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["read_xlsx data.xlsx", "read_xlsx data.csv 20"]
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
            .unwrap_or(50);

        let ext = detect_extension(path);
        let output = if ext == "csv" {
            read_csv(path, max_rows)?
        } else {
            read_workbook(path, max_rows)?
        };

        Ok(ToolOutput::success(output))
    }
}

fn read_csv(path: &str, max_rows: usize) -> Result<String, ToolError> {
    let content =
        std::fs::read_to_string(path).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    let lines = content.lines().take(max_rows).collect::<Vec<_>>();
    Ok(lines.join("\n"))
}

fn read_workbook(path: &str, max_rows: usize) -> Result<String, ToolError> {
    let mut workbook = calamine::open_workbook_auto(path)
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    let sheet_names = workbook.sheet_names().to_owned();
    let mut output = String::new();

    for sheet in sheet_names.iter().take(3) {
        if let Ok(range) = workbook.worksheet_range(sheet) {
            output.push_str(&format!("Sheet: {}\n", sheet));
            for (idx, row) in range.rows().take(max_rows).enumerate() {
                let line = row
                    .iter()
                    .map(|cell| cell.to_string())
                    .collect::<Vec<_>>()
                    .join("\t");
                output.push_str(&format!("{}\t{}\n", idx + 1, line));
            }
            output.push('\n');
        }
    }

    if output.trim().is_empty() {
        output = "(no data)".to_string();
    }

    Ok(output)
}
