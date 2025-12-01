use domain::models::Embedding;
use std::cmp::Ordering;

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
        use std::collections::BinaryHeap;

        #[derive(Debug)]
        struct Scored<'a> {
            score: f32,
            text: &'a str,
        }

        impl<'a> PartialEq for Scored<'a> {
            fn eq(&self, other: &Self) -> bool {
                self.score.eq(&other.score)
            }
        }
        impl<'a> Eq for Scored<'a> {}
        impl<'a> PartialOrd for Scored<'a> {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                self.score.partial_cmp(&other.score)
            }
        }
        impl<'a> Ord for Scored<'a> {
            fn cmp(&self, other: &Self) -> Ordering {
                self.partial_cmp(other).unwrap_or(Ordering::Equal)
            }
        }

        let mut heap: BinaryHeap<Scored> =
            BinaryHeap::with_capacity(top_k.saturating_mul(2).max(8));
        for emb in embeddings {
            let score = Self::cosine_similarity(query_embedding, &emb.vector);
            heap.push(Scored {
                score,
                text: emb.text.as_str(),
            });
            if heap.len() > top_k * 3 {
                heap.pop();
            }
        }

        let mut results: Vec<Scored> = heap.into_iter().collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        results
            .into_iter()
            .take(top_k)
            .map(|s| s.text.to_string())
            .collect()
    }
}
