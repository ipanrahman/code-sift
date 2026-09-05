//! Semantic search using TF-IDF for code similarity.
//!
//! This module provides lightweight semantic search using TF-IDF vectorization
//! and cosine similarity. For production use with large codebases, consider
//! upgrading to proper embeddings (e.g., code-transformer-tiny).
//!
//! Architecture:
//! - Documents are tokenized code fragments (symbols, file chunks)
//! - TF-IDF vectors are computed for all documents at index time
//! - Query vectors are computed on-demand
//! - Cosine similarity ranks results
//!
//! Performance:
//! - `search()` currently scans all documents linearly: O(n) per query.
//! - Acceptable for small-to-medium repos; for large repos, replace with
//!   an inverted index or vector DB-backed retrieval path.

use crate::types::{FileId, Range};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Semantic match result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMatch {
    pub file_id: FileId,
    pub range: Range,
    pub score: f64,
    pub doc_id: String,
}

/// TF-IDF document representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub file_id: FileId,
    pub range: Range,
    pub tokens: Vec<String>,
    pub term_freq: HashMap<String, f64>,
}

/// Inverted index for fast similarity search.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SemanticIndex {
    documents: Vec<Document>,
    idf: HashMap<String, f64>,
    doc_count: usize,
}

impl SemanticIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build index from code fragments.
    pub fn build(&mut self, fragments: Vec<CodeFragment>) {
        self.documents.clear();
        self.idf.clear();

        if fragments.is_empty() {
            self.doc_count = 0;
            return;
        }

        // Tokenize and build term frequency
        let mut doc_term_counts: Vec<HashMap<String, usize>> = Vec::new();

        for fragment in &fragments {
            let tokens = tokenize_code(&fragment.text);
            let term_counts = count_terms(&tokens);
            doc_term_counts.push(term_counts.clone());

            self.documents.push(Document {
                id: fragment.id.clone(),
                file_id: fragment.file_id,
                range: fragment.range.clone(),
                tokens,
                term_freq: term_counts
                    .iter()
                    .map(|(k, v)| (k.clone(), *v as f64))
                    .collect(),
            });
        }

        self.doc_count = self.documents.len();

        // Compute IDF
        let vocab: HashSet<_> = doc_term_counts.iter().flat_map(|t| t.keys()).collect();

        for term in vocab {
            let docs_with_term = doc_term_counts
                .iter()
                .filter(|t| t.contains_key(term))
                .count();
            // IDF = log(N / df) + 1 (smoothed)
            self.idf.insert(
                term.clone(),
                (self.doc_count as f64 / docs_with_term.max(1) as f64).ln() + 1.0,
            );
        }
    }

    /// Search for similar documents using cosine similarity.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SemanticMatch> {
        if self.documents.is_empty() {
            return Vec::new();
        }

        let query_tokens = tokenize_code(query);
        let query_tf = count_terms(&query_tokens);

        // Compute query TF-IDF vector (sparse)
        let query_vec: HashMap<&str, f64> = query_tf
            .iter()
            .map(|(term, tf)| {
                let idf = self.idf.get(term).copied().unwrap_or(0.0);
                (term.as_str(), *tf as f64 * idf)
            })
            .collect();

        // Compute query vector magnitude
        let query_mag = query_vec
            .values()
            .map(|v| v * v)
            .sum::<f64>()
            .sqrt()
            .max(1e-10);

        // Score all documents
        let mut scores: Vec<(usize, f64)> = self
            .documents
            .iter()
            .enumerate()
            .filter_map(|(i, doc)| {
                if doc.term_freq.is_empty() {
                    return None;
                }

                // Cosine similarity: dot(q, d) / (|q| * |d|)
                let dot: f64 = query_vec
                    .iter()
                    .filter_map(|(term, qv)| doc.term_freq.get(*term).map(|dv| qv * dv))
                    .sum();

                let doc_mag = doc.term_freq.values().map(|v| v * v).sum::<f64>().sqrt();

                if doc_mag < 1e-10 {
                    return None;
                }

                let similarity = dot / (query_mag * doc_mag);
                if similarity > 0.0 {
                    Some((i, similarity))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores
            .into_iter()
            .take(limit)
            .filter_map(|(i, score)| {
                let doc = &self.documents[i];
                Some(SemanticMatch {
                    file_id: doc.file_id,
                    range: doc.range.clone(),
                    score,
                    doc_id: doc.id.clone(),
                })
            })
            .collect()
    }

    /// Get document count.
    pub fn doc_count(&self) -> usize {
        self.doc_count
    }

    /// Serialize to bytes for caching.
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// Deserialize from bytes.
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        bincode::deserialize(data).ok()
    }
}

/// Code fragment for indexing.
#[derive(Debug, Clone)]
pub struct CodeFragment {
    pub id: String,
    pub file_id: FileId,
    pub range: Range,
    pub text: String,
}

/// Tokenize code into terms.
fn tokenize_code(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    // Simple tokenization: split on whitespace and non-alphanumeric
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else {
            if !current.is_empty() {
                let lower = current.to_lowercase();
                if !is_stop_word(&lower) {
                    tokens.push(lower);
                }
                current.clear();
            }
        }
    }
    if !current.is_empty() {
        let lower = current.to_lowercase();
        if !is_stop_word(&lower) {
            tokens.push(lower);
        }
    }

    tokens
}

/// Check if term is a stop word.
fn is_stop_word(term: &str) -> bool {
    matches!(
        term,
        // Common control flow / keywords across most languages
        "if" | "else" | "for" | "while" | "return"
            // Common boolean / nil literals
            | "true" | "false" | "nil" | "none"
    )
}

/// Count term frequencies.
fn count_terms(tokens: &[String]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for token in tokens {
        *counts.entry(token.clone()).or_default() += 1;
    }
    // Normalize by document length
    let len = tokens.len().max(1);
    for count in counts.values_mut() {
        let v: usize = *count;
        *count = (v * 100 / len).max(1);
    }
    counts
}

/// Hybrid search configuration.
#[derive(Debug, Clone)]
pub struct HybridConfig {
    /// Weight for semantic scores (0.0 - 1.0).
    pub semantic_weight: f64,
    /// Weight for lexical scores (0.0 - 1.0).
    pub lexical_weight: f64,
    /// Minimum score threshold.
    pub min_score: f64,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            semantic_weight: 0.5,
            lexical_weight: 0.5,
            min_score: 0.1,
        }
    }
}

/// Combine semantic and lexical scores.
pub fn combine_scores(
    semantic: &[SemanticMatch],
    lexical_count: usize,
    config: &HybridConfig,
) -> f64 {
    if semantic.is_empty() && lexical_count == 0 {
        return 0.0;
    }

    let sem_score = if !semantic.is_empty() {
        semantic.iter().map(|m| m.score).sum::<f64>() / semantic.len() as f64
    } else {
        0.0
    };

    let lex_score = if lexical_count > 0 { 1.0 } else { 0.0 };

    let combined = config.semantic_weight * sem_score + config.lexical_weight * lex_score;
    combined.max(config.min_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_code() {
        let code = "fn main() { let x = 5; }";
        let tokens = tokenize_code(code);
        assert!(tokens.contains(&"main".to_string()));
        assert!(tokens.contains(&"x".to_string()));
    }

    #[test]
    fn test_semantic_search() {
        let mut index = SemanticIndex::new();

        let fragments = vec![
            CodeFragment {
                id: "1".into(),
                file_id: FileId::new(0),
                range: Range::new(0, 10, 1, 1),
                text: "fn calculate_sum(a: i32, b: i32) -> i32".to_string(),
            },
            CodeFragment {
                id: "2".into(),
                file_id: FileId::new(0),
                range: Range::new(0, 10, 1, 1),
                text: "fn calculate_product(a: i32, b: i32) -> i32".to_string(),
            },
            CodeFragment {
                id: "3".into(),
                file_id: FileId::new(0),
                range: Range::new(0, 10, 1, 1),
                text: "fn print_hello()".to_string(),
            },
        ];

        index.build(fragments);

        let results = index.search("calculate", 10);
        assert!(!results.is_empty());
        // "calculate" should match calculate_sum and calculate_product
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_empty_index() {
        let index = SemanticIndex::new();
        let results = index.search("query", 10);
        assert!(results.is_empty());
    }
}
