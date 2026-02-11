use super::super::entities::document::{Document, DocumentType};
use std::cmp::Ordering;

/// Domain service for analyzing documents
pub struct DocumentAnalyzer;

impl Default for DocumentAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyze document content and extract metadata
    pub fn analyze(&self, document: &Document) -> DocumentAnalysis {
        let word_count = document.word_count();
        let line_count = document.line_count();
        let complexity = self.calculate_complexity(document);
        let readability = self.calculate_readability(document);
        let key_topics = self.extract_key_topics(document);

        DocumentAnalysis::new(word_count, line_count, complexity, readability, key_topics)
    }

    /// Check if document is relevant for a query
    pub fn is_relevant_for_query(&self, document: &Document, query: &str) -> f32 {
        let content_lower = document.content().to_lowercase();
        let query_lower = query.to_lowercase();

        // Simple relevance calculation - in real implementation would be more sophisticated
        let exact_matches = content_lower.matches(&query_lower).count() as f32;
        let word_matches: f32 = query_lower
            .split_whitespace()
            .map(|word| content_lower.matches(word).count() as f32)
            .sum();

        let total_score = (exact_matches * 2.0 + word_matches) / document.content().len() as f32;
        total_score.clamp(0.0, 1.0)
    }

    /// Extract summary from document
    pub fn extract_summary(&self, document: &Document, max_sentences: usize) -> String {
        let sentences: Vec<&str> = document
            .content()
            .split_terminator(['.', '!', '?'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .take(max_sentences)
            .collect();

        sentences.join(". ")
    }

    /// Find similar documents based on content
    pub fn find_similar_documents(
        &self,
        target: &Document,
        candidates: &[Document],
    ) -> Vec<SimilarityScore> {
        let mut results = Vec::new();
        let target_content = target.content().to_lowercase();
        let target_words: std::collections::HashSet<_> =
            target_content.split_whitespace().collect();

        for doc in candidates {
            let doc_content = doc.content().to_lowercase();
            let doc_words: std::collections::HashSet<_> = doc_content.split_whitespace().collect();

            let intersection = target_words.intersection(&doc_words).count();
            let union = target_words.union(&doc_words).count();

            let similarity = if union == 0 {
                0.0
            } else {
                intersection as f32 / union as f32
            };

            if similarity > 0.1 {
                results.push(SimilarityScore::new(doc.id().to_string(), similarity));
            }
        }

        // Sort by similarity (descending)
        results.sort_by(|a, b| {
            b.similarity()
                .partial_cmp(&a.similarity())
                .unwrap_or(Ordering::Equal)
        });
        results
    }

    // Private helper methods
    fn calculate_complexity(&self, document: &Document) -> ComplexityScore {
        match document.content_type() {
            DocumentType::Code(_) => self.calculate_code_complexity(document),
            DocumentType::PlainText | DocumentType::Markdown => {
                self.calculate_text_complexity(document)
            }
            _ => ComplexityScore::Low,
        }
    }

    fn calculate_code_complexity(&self, document: &Document) -> ComplexityScore {
        let content = document.content();
        let line_count = document.line_count();

        // Simple complexity metrics for code
        let nested_blocks = content.matches("    ").count();
        let special_chars = content
            .matches(|c: char| !c.is_alphanumeric() && !c.is_whitespace())
            .count();

        let complexity_score = (nested_blocks + special_chars) as f32 / line_count as f32;

        if complexity_score > 0.5 {
            ComplexityScore::High
        } else if complexity_score > 0.2 {
            ComplexityScore::Medium
        } else {
            ComplexityScore::Low
        }
    }

    fn calculate_text_complexity(&self, document: &Document) -> ComplexityScore {
        let words = document.word_count();
        let sentences = document.content().split_terminator(['.', '!', '?']).count();

        if sentences == 0 {
            return ComplexityScore::Low;
        }

        let avg_words_per_sentence = words as f32 / sentences as f32;

        if avg_words_per_sentence > 20.0 {
            ComplexityScore::High
        } else if avg_words_per_sentence > 12.0 {
            ComplexityScore::Medium
        } else {
            ComplexityScore::Low
        }
    }

    fn calculate_readability(&self, document: &Document) -> ReadabilityScore {
        let words = document.word_count();
        let sentences = document.content().split_terminator(['.', '!', '?']).count();
        let syllables = self.count_syllables(document.content());

        if sentences == 0 {
            return ReadabilityScore::new(0.0, "Very Easy");
        }

        // Simple Flesch Reading Ease calculation
        let avg_sentence_length = words as f32 / sentences as f32;
        let avg_syllables_per_word = syllables as f32 / words as f32;

        let flesch_score =
            206.835 - (1.015 * avg_sentence_length) - (84.6 * avg_syllables_per_word);

        let (score, description) = if flesch_score >= 90.0 {
            (flesch_score, "Very Easy")
        } else if flesch_score >= 80.0 {
            (flesch_score, "Easy")
        } else if flesch_score >= 70.0 {
            (flesch_score, "Fairly Easy")
        } else if flesch_score >= 60.0 {
            (flesch_score, "Standard")
        } else if flesch_score >= 50.0 {
            (flesch_score, "Fairly Difficult")
        } else if flesch_score >= 30.0 {
            (flesch_score, "Difficult")
        } else {
            (flesch_score, "Very Difficult")
        };

        ReadabilityScore::new(score, description)
    }

    fn extract_key_topics(&self, document: &Document) -> Vec<String> {
        // Simple topic extraction - in real implementation would use NLP
        let mut word_counts = std::collections::HashMap::new();
        document
            .content()
            .to_lowercase()
            .split_whitespace()
            .filter(|word| word.len() > 4) // Filter out short words
            .map(|word| {
                word.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|word| !word.is_empty())
            .for_each(|word| {
                *word_counts.entry(word).or_insert(0) += 1;
            });

        let mut topics: Vec<_> = word_counts
            .into_iter()
            .filter(|(_, count)| *count >= 3) // Words that appear at least 3 times
            .collect();

        topics.sort_by(|a, b| b.1.cmp(&a.1));
        topics.into_iter().take(5).map(|(topic, _)| topic).collect()
    }

    fn count_syllables(&self, text: &str) -> usize {
        // Very simple syllable counting - in real implementation would be more accurate
        text.to_lowercase()
            .split_whitespace()
            .map(|word| {
                let vowel_groups = word.matches(['a', 'e', 'i', 'o', 'u']).count();
                std::cmp::max(1, vowel_groups)
            })
            .sum()
    }
}

/// Result of document analysis
#[derive(Debug, Clone)]
pub struct DocumentAnalysis {
    word_count: usize,
    line_count: usize,
    complexity: ComplexityScore,
    readability: ReadabilityScore,
    key_topics: Vec<String>,
}

impl DocumentAnalysis {
    pub fn new(
        word_count: usize,
        line_count: usize,
        complexity: ComplexityScore,
        readability: ReadabilityScore,
        key_topics: Vec<String>,
    ) -> Self {
        Self {
            word_count,
            line_count,
            complexity,
            readability,
            key_topics,
        }
    }

    pub fn word_count(&self) -> usize {
        self.word_count
    }

    pub fn line_count(&self) -> usize {
        self.line_count
    }

    pub fn complexity(&self) -> &ComplexityScore {
        &self.complexity
    }

    pub fn readability(&self) -> &ReadabilityScore {
        &self.readability
    }

    pub fn key_topics(&self) -> &[String] {
        &self.key_topics
    }
}

#[derive(Debug, Clone)]
pub enum ComplexityScore {
    Low,
    Medium,
    High,
}

impl ComplexityScore {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComplexityScore::Low => "Low",
            ComplexityScore::Medium => "Medium",
            ComplexityScore::High => "High",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReadabilityScore {
    score: f32,
    description: &'static str,
}

impl ReadabilityScore {
    pub fn new(score: f32, description: &'static str) -> Self {
        Self { score, description }
    }

    pub fn score(&self) -> f32 {
        self.score
    }

    pub fn description(&self) -> &'static str {
        self.description
    }
}

#[derive(Debug, Clone)]
pub struct SimilarityScore {
    document_id: String,
    similarity: f32,
}

impl SimilarityScore {
    pub fn new(document_id: String, similarity: f32) -> Self {
        Self {
            document_id,
            similarity,
        }
    }

    pub fn document_id(&self) -> &str {
        &self.document_id
    }

    pub fn similarity(&self) -> f32 {
        self.similarity
    }
}
