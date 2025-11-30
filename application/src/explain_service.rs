use shared::types::Result;

pub struct ExplainService;

impl ExplainService {
    pub fn new() -> Self {
        Self
    }

    pub async fn explain_file(&self, _file_path: &str) -> Result<String> {
        Ok("Explanation not implemented".to_string())
    }
}
