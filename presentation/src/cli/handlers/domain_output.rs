use super::CliHandlers;
use colored::Colorize;
use shared::types::Result;

impl CliHandlers {
    pub(crate) async fn interpret_output(&self, query: &str, output: &str) -> Result<()> {
        println!("\n{}", "=== AI Interpretation ===".green().bold());

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let prompt = format!(
            "The user asked: '{}'\n\n\
            Command output:\n{}\n\n\
            Please provide a clear, concise summary of what this output means. \
            Focus on the key information and present it in a well-organized format. \
            Use sections and bullet points where appropriate.",
            query, output
        );

        let response = client.generate_response(&prompt).await?;
        println!("{}", response);
        Ok(())
    }
}
