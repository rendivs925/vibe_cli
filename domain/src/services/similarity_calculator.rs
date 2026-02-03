use super::super::value_objects::embedding::{Embedding, SearchResult};
use smallvec::SmallVec;

/// Domain service for calculating similarity between embeddings
pub struct SimilarityCalculator;

impl SimilarityCalculator {
    pub fn new() -> Self {
        Self
    }

    /// Calculate cosine similarity between two embeddings
    pub fn cosine_similarity(&self, a: &Embedding, b: &Embedding) -> f32 {
        if a.dimensions() != b.dimensions() {
            return 0.0;
        }

        let dot_product: f32 = a
            .vector()
            .iter()
            .zip(b.vector().iter())
            .map(|(x, y)| x * y)
            .sum();

        let magnitude_a = a.magnitude();
        let magnitude_b = b.magnitude();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            0.0
        } else {
            dot_product / (magnitude_a * magnitude_b)
        }
    }

    /// Calculate Euclidean distance between two embeddings
    pub fn euclidean_distance(&self, a: &Embedding, b: &Embedding) -> f32 {
        if a.dimensions() != b.dimensions() {
            return f32::INFINITY;
        }

        let distance_squared: f32 = a
            .vector()
            .iter()
            .zip(b.vector().iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum();

        distance_squared.sqrt()
    }

    /// Calculate Manhattan distance between two embeddings
    pub fn manhattan_distance(&self, a: &Embedding, b: &Embedding) -> f32 {
        if a.dimensions() != b.dimensions() {
            return f32::INFINITY;
        }

        a.vector()
            .iter()
            .zip(b.vector().iter())
            .map(|(x, y)| (x - y).abs())
            .sum()
    }

    /// Find most similar embeddings to a query
    pub fn find_similar(
        &self,
        query: &Embedding,
        candidates: &[Embedding],
        max_results: usize,
    ) -> SmallVec<[SearchResult; 8]> {
        let mut results: SmallVec<[SearchResult; 8]> = candidates
            .iter()
            .map(|candidate| {
                let similarity = self.cosine_similarity(query, candidate);
                SearchResult::new(candidate.clone(), similarity)
            })
            .filter(|result| result.similarity() > 0.0)
            .collect();

        // Sort by similarity (descending)
        results.sort_by(|a, b| {
            b.similarity()
                .partial_cmp(&a.similarity())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top results
        results.truncate(max_results);
        results
    }

    /// Find embeddings above a similarity threshold
    pub fn find_above_threshold(
        &self,
        query: &Embedding,
        candidates: &[Embedding],
        threshold: f32,
    ) -> SmallVec<[SearchResult; 8]> {
        candidates
            .iter()
            .map(|candidate| {
                let similarity = self.cosine_similarity(query, candidate);
                SearchResult::new(candidate.clone(), similarity)
            })
            .filter(|result| result.similarity() >= threshold)
            .collect()
    }

    /// Calculate similarity matrix for a set of embeddings
    pub fn similarity_matrix(&self, embeddings: &[Embedding]) -> Vec<Vec<f32>> {
        let n = embeddings.len();
        let mut matrix = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in i..n {
                let similarity = if i == j {
                    1.0
                } else {
                    self.cosine_similarity(&embeddings[i], &embeddings[j])
                };
                matrix[i][j] = similarity;
                matrix[j][i] = similarity;
            }
        }

        matrix
    }

    /// Cluster embeddings by similarity
    pub fn cluster_by_similarity(
        &self,
        embeddings: &[Embedding],
        threshold: f32,
    ) -> Vec<Vec<usize>> {
        let mut clusters = Vec::new();
        let mut assigned = vec![false; embeddings.len()];

        for i in 0..embeddings.len() {
            if assigned[i] {
                continue;
            }

            let mut cluster = vec![i];
            assigned[i] = true;

            for j in (i + 1)..embeddings.len() {
                if assigned[j] {
                    continue;
                }

                let similarity = self.cosine_similarity(&embeddings[i], &embeddings[j]);
                if similarity >= threshold {
                    cluster.push(j);
                    assigned[j] = true;
                }
            }

            clusters.push(cluster);
        }

        clusters
    }

    /// Calculate average similarity within a cluster
    pub fn cluster_cohesion(&self, embeddings: &[Embedding], cluster: &[usize]) -> f32 {
        if cluster.len() < 2 {
            return 1.0;
        }

        let mut total_similarity = 0.0;
        let mut comparisons = 0;

        for i in 0..cluster.len() {
            for j in (i + 1)..cluster.len() {
                let similarity =
                    self.cosine_similarity(&embeddings[cluster[i]], &embeddings[cluster[j]]);
                total_similarity += similarity;
                comparisons += 1;
            }
        }

        if comparisons == 0 {
            1.0
        } else {
            total_similarity / comparisons as f32
        }
    }

    /// Find outliers based on low similarity to others
    pub fn find_outliers(&self, embeddings: &[Embedding], threshold: f32) -> SmallVec<[usize; 8]> {
        let mut outliers: SmallVec<[usize; 8]> = SmallVec::new();

        for i in 0..embeddings.len() {
            let mut max_similarity = 0.0;

            for j in 0..embeddings.len() {
                if i == j {
                    continue;
                }

                let similarity = self.cosine_similarity(&embeddings[i], &embeddings[j]);
                if similarity > max_similarity {
                    max_similarity = similarity;
                }
            }

            if max_similarity < threshold {
                outliers.push(i);
            }
        }

        outliers
    }

    /// Calculate diversity score for a set of embeddings
    pub fn diversity_score(&self, embeddings: &[Embedding]) -> f32 {
        if embeddings.len() < 2 {
            return 0.0;
        }

        let mut total_similarity = 0.0;
        let mut comparisons = 0;

        for i in 0..embeddings.len() {
            for j in (i + 1)..embeddings.len() {
                let similarity = self.cosine_similarity(&embeddings[i], &embeddings[j]);
                total_similarity += similarity;
                comparisons += 1;
            }
        }

        let avg_similarity = total_similarity / comparisons as f32;
        1.0 - avg_similarity // Diversity = 1 - average similarity
    }
}

impl Default for SimilarityCalculator {
    fn default() -> Self {
        Self::new()
    }
}
