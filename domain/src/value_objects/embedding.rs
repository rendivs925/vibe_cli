use serde::{Deserialize, Serialize};

/// Embedding value object representing a vector embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub id: String,
    pub vector: Vec<f32>,
    pub text: String,
    pub document_path: String,
}

impl Embedding {
    pub fn new(id: String, vector: Vec<f32>, text: String, document_path: String) -> Self {
        Self {
            id,
            vector,
            text,
            document_path,
        }
    }

    // Create from raw components (for infrastructure)
    pub fn from_components(
        id: String,
        vector: Vec<f32>,
        text: String,
        document_path: String,
    ) -> Self {
        Self::new(id, vector, text, document_path)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn vector(&self) -> &[f32] {
        &self.vector
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn document_path(&self) -> &str {
        &self.document_path
    }

    // Direct field access for infrastructure
    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_vector(&self) -> &[f32] {
        &self.vector
    }

    pub fn get_text(&self) -> &str {
        &self.text
    }

    pub fn get_document_path(&self) -> &str {
        &self.document_path
    }

    pub fn dimensions(&self) -> usize {
        self.vector.len()
    }

    pub fn magnitude(&self) -> f32 {
        self.vector.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Calculate cosine similarity with another embedding
    pub fn cosine_similarity(&self, other: &Embedding) -> f32 {
        if self.dimensions() != other.dimensions() {
            return 0.0;
        }

        let dot_product: f32 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| a * b)
            .sum();

        let magnitude_a = self.magnitude();
        let magnitude_b = other.magnitude();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            0.0
        } else {
            dot_product / (magnitude_a * magnitude_b)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.vector.is_empty() || self.text.trim().is_empty()
    }

    pub fn snippet(&self, max_chars: usize) -> String {
        if self.text.len() <= max_chars {
            self.text.clone()
        } else {
            format!("{}...", &self.text[..max_chars])
        }
    }
}

/// Result of a similarity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    embedding: Embedding,
    similarity: f32,
}

impl SearchResult {
    pub fn new(embedding: Embedding, similarity: f32) -> Self {
        Self {
            embedding,
            similarity,
        }
    }

    pub fn embedding(&self) -> &Embedding {
        &self.embedding
    }

    pub fn similarity(&self) -> f32 {
        self.similarity
    }

    pub fn relevance_score(&self) -> f32 {
        self.similarity.clamp(0.0, 1.0)
    }
}
