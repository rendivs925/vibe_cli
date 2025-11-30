use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQuery {
    pub question: String,
    pub context: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub id: String,
    pub vector: Vec<f32>,
    pub text: String,
}