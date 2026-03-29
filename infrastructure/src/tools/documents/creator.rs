use std::fs::File;
use std::io::Write;

pub struct DocumentCreator;

impl DocumentCreator {
    pub fn create_markdown(title: &str, content: &str, output_path: &str) -> Result<(), String> {
        let mut file = File::create(output_path).map_err(|e| e.to_string())?;

        writeln!(file, "# {}", title).map_err(|e| e.to_string())?;
        writeln!(file).map_err(|e| e.to_string())?;
        writeln!(file, "{}", content).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn create_csv(data: &[Vec<String>], output_path: &str) -> Result<(), String> {
        let mut file = File::create(output_path).map_err(|e| e.to_string())?;

        for row in data {
            let line = row.join(",");
            writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub fn create_text(content: &str, output_path: &str) -> Result<(), String> {
        let mut file = File::create(output_path).map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn create_html(title: &str, content: &str, output_path: &str) -> Result<(), String> {
        let mut file = File::create(output_path).map_err(|e| e.to_string())?;

        writeln!(file, "<!DOCTYPE html>").map_err(|e| e.to_string())?;
        writeln!(file, "<html>").map_err(|e| e.to_string())?;
        writeln!(file, "<head><title>{}</title></head>", title).map_err(|e| e.to_string())?;
        writeln!(file, "<body>").map_err(|e| e.to_string())?;
        writeln!(file, "{}", content).map_err(|e| e.to_string())?;
        writeln!(file, "</body>").map_err(|e| e.to_string())?;
        writeln!(file, "</html>").map_err(|e| e.to_string())?;

        Ok(())
    }
}

pub struct SpreadsheetAnalyzer {
    data: Vec<Vec<String>>,
}

impl SpreadsheetAnalyzer {
    pub fn from_csv(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

        let data: Vec<Vec<String>> = content
            .lines()
            .map(|line| line.split(',').map(|s| s.trim().to_string()).collect())
            .collect();

        Ok(Self { data })
    }

    pub fn from_xlsx(path: &str) -> Result<Self, String> {
        use calamine::Reader;

        let mut workbook = calamine::open_workbook_auto(path).map_err(|e| e.to_string())?;
        let sheet_names = workbook.sheet_names().to_owned();

        let mut data = Vec::new();

        if let Some(sheet) = sheet_names.first() {
            if let Ok(range) = workbook.worksheet_range(sheet) {
                for row in range.rows() {
                    let row_data: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();
                    data.push(row_data);
                }
            }
        }

        Ok(Self { data })
    }

    pub fn get_summary(&self) -> String {
        let rows = self.data.len();
        let cols = if rows > 0 { self.data[0].len() } else { 0 };

        let mut summary = format!("Spreadsheet Summary:\n");
        summary.push_str(&format!("- Rows: {}\n", rows));
        summary.push_str(&format!("- Columns: {}\n", cols));

        if rows > 1 {
            let numeric_cols: Vec<usize> = (0..cols)
                .filter(|&j| {
                    self.data.iter().skip(1).all(|row| {
                        row.get(j)
                            .map(|c| c.parse::<f64>().is_ok())
                            .unwrap_or(false)
                    })
                })
                .collect();

            if !numeric_cols.is_empty() {
                summary.push_str("\nNumeric Columns:\n");
                for j in numeric_cols {
                    let sum: f64 = self
                        .data
                        .iter()
                        .skip(1)
                        .filter_map(|row| row.get(j).and_then(|c| c.parse::<f64>().ok()))
                        .sum();

                    let col_name = self.data[0]
                        .get(j)
                        .cloned()
                        .unwrap_or_else(|| format!("Column {}", j + 1));
                    summary.push_str(&format!("- {}: sum = {:.2}\n", col_name, sum));
                }
            }
        }

        summary
    }

    pub fn filter_rows(&self, column: usize, value: &str) -> Vec<Vec<String>> {
        self.data
            .iter()
            .filter(|row| row.get(column).map(|c| c == value).unwrap_or(false))
            .cloned()
            .collect()
    }

    pub fn sort_by_column(&self, column: usize, ascending: bool) -> Vec<Vec<String>> {
        let mut sorted = self.data.clone();

        if sorted.len() > 1 {
            let default_val = String::new();
            sorted[1..].sort_by(|a, b| {
                let a_val = a.get(column).unwrap_or(&default_val);
                let b_val = b.get(column).unwrap_or(&default_val);

                let cmp = a_val.cmp(b_val);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }

        sorted
    }
}
