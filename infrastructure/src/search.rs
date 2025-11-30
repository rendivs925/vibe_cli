use domain::models::Embedding;

pub struct SearchEngine;

impl SearchEngine {
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot_product / (norm_a * norm_b)
    }

    pub fn find_relevant_chunks(
        query_embedding: &[f32],
        embeddings: &[Embedding],
        top_k: usize,
    ) -> Vec<String> {
        let mut similarities: Vec<(f32, &str)> = embeddings
            .iter()
            .map(|emb| {
                (
                    Self::cosine_similarity(query_embedding, &emb.vector),
                    &emb.text[..],
                )
            })
            .collect();

        similarities.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        similarities
            .into_iter()
            .take(top_k)
            .map(|(_, text)| text.to_string())
            .collect()
    }
}
