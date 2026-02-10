use super::CliHandlers;
use anyhow::anyhow;
use shared::types::Result;

impl CliHandlers {
    pub async fn handle_explain(&self, file: &str) -> Result<()> {
        let content = match self.load_explain_content(file) {
            Ok(Some(content)) => content,
            Ok(None) => {
                println!("Error: No text content found in file '{}'.", file);
                return Ok(());
            }
            Err(err) => {
                println!("{err}");
                return Ok(());
            }
        };

        let prompt = format!("Explain this content in detail:\n\n{}", content);

        if let Some(cached_response) = self.cache_manager.load_explain_cached(&prompt)? {
            println!("{}", cached_response);
            return Ok(());
        }

        eprintln!("Analyzing file content...");
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let response = client.generate_response(&prompt).await?;

        self.cache_manager.save_explain_cached(&prompt, &response)?;

        println!("{}", response);
        Ok(())
    }

    fn load_explain_content(&self, file: &str) -> Result<Option<String>> {
        let path = std::path::Path::new(file);
        let content = match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("pdf") => Self::read_pdf_text(file)?,
            Some(ext) if ext.eq_ignore_ascii_case("docx") => Self::read_docx_text(file)?,
            _ => Self::read_text_file(file)?,
        };

        if content.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(content))
        }
    }

    fn read_text_file(file: &str) -> Result<String> {
        std::fs::read_to_string(file).map_err(|_| {
            anyhow!(
                "Error: Cannot read file '{}' as text. Supported formats: text files, PDF, DOCX.",
                file
            )
        })
    }

    fn read_pdf_text(file: &str) -> Result<String> {
        pdf_extract::extract_text(file)
            .map_err(|e| anyhow!("Error extracting text from PDF '{}': {}", file, e))
    }

    fn read_docx_text(file: &str) -> Result<String> {
        let bytes =
            std::fs::read(file).map_err(|e| anyhow!("Error reading DOCX file '{}': {}", file, e))?;
        let docx =
            docx_rs::read_docx(&bytes).map_err(|e| anyhow!("Error parsing DOCX '{}': {}", file, e))?;
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
        Ok(text)
    }
}
