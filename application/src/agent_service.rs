use shared::types::Result;

pub struct AgentService;

impl AgentService {
    pub fn new() -> Self {
        Self
    }

    pub async fn run_agent(&self, _input: &str) -> Result<String> {
        Ok("Agent not implemented".to_string())
    }
}
