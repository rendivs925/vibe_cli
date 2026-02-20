pub mod creator;
pub mod docx;
pub mod pdf;
pub mod qa;
pub mod tables;
pub mod xlsx;

use domain::tools::ToolError;

pub(crate) fn read_file_bytes(path: &str) -> Result<Vec<u8>, ToolError> {
    std::fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ToolError::NotFound(path.to_string())
        } else {
            ToolError::ExecutionFailed(e.to_string())
        }
    })
}

pub(crate) fn detect_extension(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}
