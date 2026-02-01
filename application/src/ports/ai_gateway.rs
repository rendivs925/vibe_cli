use async_trait::async_trait;
use shared::error::AppError;

/// AI Gateway port for interacting with AI services
#[async_trait]
pub trait AiClient: Send + Sync {
    /// Generate a text response from AI
    async fn generate_response(&self, prompt: &str) -> Result<String, AppError>;

    /// Generate response with conversation context
    async fn generate_response_with_context(
        &self,
        prompt: &str,
        context: &[String],
    ) -> Result<String, AppError>;

    /// Generate embeddings for text
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, AppError>;

    /// Generate embeddings for multiple texts
    async fn generate_embeddings_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError>;

    /// Stream response generation
    async fn stream_response(
        &self,
        prompt: &str,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String, AppError>> + Send>, AppError>;

    /// Check if AI service is available
    async fn health_check(&self) -> Result<bool, AppError>;

    /// Get model information
    async fn get_model_info(&self) -> Result<ModelInfo, AppError>;
}

/// Information about the AI model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub version: String,
    pub max_tokens: usize,
    pub supports_embeddings: bool,
    pub supports_streaming: bool,
}

impl ModelInfo {
    pub fn new(
        name: String,
        version: String,
        max_tokens: usize,
        supports_embeddings: bool,
        supports_streaming: bool,
    ) -> Self {
        Self {
            name,
            version,
            max_tokens,
            supports_embeddings,
            supports_streaming,
        }
    }
}

/// Request for AI generation
#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub prompt: String,
    pub context: Vec<String>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub stream: bool,
}

impl GenerationRequest {
    pub fn new(prompt: String) -> Self {
        Self {
            prompt,
            context: Vec::new(),
            max_tokens: None,
            temperature: None,
            stream: false,
        }
    }

    pub fn with_context(mut self, context: Vec<String>) -> Self {
        self.context = context;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_streaming(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

/// Response from AI generation
#[derive(Debug, Clone)]
pub struct GenerationResponse {
    pub content: String,
    pub tokens_used: usize,
    pub model: String,
    pub finish_reason: FinishReason,
}

impl GenerationResponse {
    pub fn new(
        content: String,
        tokens_used: usize,
        model: String,
        finish_reason: FinishReason,
    ) -> Self {
        Self {
            content,
            tokens_used,
            model,
            finish_reason,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FinishReason {
    Stop,
    Length,
    Error,
}

/// Embedding request
#[derive(Debug, Clone)]
pub struct EmbeddingRequest {
    pub texts: Vec<String>,
    pub model: Option<String>,
}

impl EmbeddingRequest {
    pub fn new(texts: Vec<String>) -> Self {
        Self { texts, model: None }
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }
}

/// Embedding response
#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub model: String,
    pub tokens_used: usize,
}

impl EmbeddingResponse {
    pub fn new(embeddings: Vec<Vec<f32>>, model: String, tokens_used: usize) -> Self {
        Self {
            embeddings,
            model,
            tokens_used,
        }
    }
}
