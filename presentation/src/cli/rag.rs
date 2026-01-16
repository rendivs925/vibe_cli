//! RAG and file explanation functionality

use anyhow::Result;
use docx_rs::{read_docx, DocumentChild};
use std::path::Path;

/// Read file content with support for multiple formats (text, PDF, DOCX)
pub fn read_file_content(file: &str) -> Result<String> {
    let path = Path::new(file);

    let content = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext.to_lowercase().as_str() {
            "pdf" => read_pdf_content(file)?,
            "docx" => read_docx_content(file)?,
            _ => read_text_content(file)?,
        }
    } else {
        read_text_content(file)?
    };

    if content.trim().is_empty() {
        anyhow::bail!("No text content found in file '{}'", file);
    }

    Ok(content)
}

/// Read PDF file content
fn read_pdf_content(file: &str) -> Result<String> {
    match pdf_extract::extract_text(file) {
        Ok(text) => Ok(text),
        Err(e) => {
            anyhow::bail!("Error extracting text from PDF '{}': {}", file, e)
        }
    }
}

/// Read DOCX file content
fn read_docx_content(file: &str) -> Result<String> {
    let bytes = std::fs::read(file)
        .map_err(|e| anyhow::anyhow!("Error reading DOCX file '{}': {}", file, e))?;

    let docx = read_docx(&bytes)
        .map_err(|e| anyhow::anyhow!("Error parsing DOCX '{}': {}", file, e))?;

    let mut text = String::new();
    for child in &docx.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                text.push_str(&p.raw_text());
                text.push('\n');
            }
            DocumentChild::Table(_t) => {
                // Table extraction not implemented yet
                text.push_str("[Table content not extracted]\n");
            }
            _ => {
                // Skip other elements
            }
        }
    }

    Ok(text)
}

/// Read text file content
fn read_text_content(file: &str) -> Result<String> {
    std::fs::read_to_string(file).map_err(|_| {
        anyhow::anyhow!(
            "Cannot read file '{}' as text. Supported formats: text files, PDF, DOCX.",
            file
        )
    })
}

/// Create explanation prompt from file content
pub fn create_explain_prompt(content: &str) -> String {
    format!("Explain this content in detail:\n\n{}", content)
}

/// Check if response contains secrets detection marker
pub fn is_secrets_detected_response(response: &str) -> bool {
    response.starts_with("__SECRETS_DETECTED__:")
}

/// Strip secrets detection marker from response
pub fn strip_secrets_marker(response: &str) -> &str {
    response.trim_start_matches("__SECRETS_DETECTED__:").trim()
}

/// Get user feedback for RAG query improvement
pub fn get_user_feedback() -> Result<String> {
    use std::io::{self, Write};

    let mut feedback = String::new();
    eprint!("Provide feedback for improvement: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut feedback)?;
    Ok(feedback.trim().to_string())
}
