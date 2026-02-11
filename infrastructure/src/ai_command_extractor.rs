use crate::ollama_client::OllamaClient;
use domain::services::command_extraction::cleanup_ai_response;
use domain::services::CommandExtractor;
use tokio::runtime::Handle;

pub struct OllamaCommandExtractor {
    client: OllamaClient,
}

impl OllamaCommandExtractor {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            client: OllamaClient::new()?,
        })
    }

    pub fn extract(&self, input: &str) -> Option<String> {
        CommandExtractor::extract_command(self, input)
    }

    fn build_prompt(input: &str) -> String {
        format!(
            "Extract the single shell command from the user request.\n\
If there is no command, return an empty string.\n\
Return only the command. No JSON. No prose. No code fences.\n\
\n\
User request:\n{}\n",
            input
        )
    }

    fn generate_blocking(&self, prompt: &str) -> Option<String> {
        if let Ok(handle) = Handle::try_current() {
            return tokio::task::block_in_place(|| {
                handle.block_on(self.client.generate_response(prompt))
            })
            .ok();
        }

        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(self.client.generate_response(prompt)).ok()
    }
}

impl CommandExtractor for OllamaCommandExtractor {
    fn extract_command(&self, input: &str) -> Option<String> {
        let prompt = Self::build_prompt(input);
        let response = self.generate_blocking(&prompt)?;
        let cleaned = cleanup_ai_response(&response);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }
}
