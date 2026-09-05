//! Hybrid retrieval combining lexical and semantic search.
//!
//! This module implements the hybrid search strategy described in the semantic
//! search task, combining TF-IDF semantic similarity with BM25-style lexical
//! matching for optimal code retrieval.

use crate::index::{Index, LexicalMatch, TokenBudget};
use crate::semantic::{self, HybridConfig, SemanticIndex, SemanticMatch};
use crate::types::{FileId, Range, Relationship, Symbol};
use hashbrown::HashMap;
use std::sync::{Arc, RwLock};

/// Hybrid search mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RetrievalMode {
    /// Lexical search only (default).
    Lexical,
    /// Semantic search only.
    Semantic,
    /// Combine lexical and semantic (default for natural language queries).
    Hybrid,
}

impl Default for RetrievalMode {
    fn default() -> Self {
        Self::Lexical
    }
}

/// Retrieval result combining all signals.
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub file_id: FileId,
    pub range: Range,
    pub total_score: f64,
    pub lexical_score: f64,
    pub semantic_score: f64,
    pub symbol: Option<Symbol>,
    pub relationship: Option<Relationship>,
}

/// Query analysis to determine best retrieval strategy.
pub fn analyze_query(query: &str) -> RetrievalMode {
    let has_question_words = ["what", "how", "why", "where", "when", "which"]
        .iter()
        .any(|w| query.to_lowercase().contains(w));

    let is_identifier = query.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ':');

    if has_question_words || (query.len() > 20 && !is_identifier) {
        RetrievalMode::Hybrid
    } else {
        RetrievalMode::Lexical
    }
}

/// Perform hybrid retrieval across lexical and semantic indices.
pub fn hybrid_search(
    query: &str,
    index: &Index,
    semantic_index: &SemanticIndex,
    budget: &TokenBudget,
    config: &HybridConfig,
) -> Vec<RetrievalResult> {
    let mode = analyze_query(query);

    match mode {
        RetrievalMode::Lexical => {
            lexical_only(query, index, budget)
                .into_iter()
                .map(|m| RetrievalResult {
                    file_id: m.file_id,
                    range: m.range,
                    total_score: 1.0,
                    lexical_score: 1.0,
                    semantic_score: 0.0,
                    symbol: None,
                    relationship: None,
                })
                .collect()
        }
        RetrievalMode::Semantic => {
            semantic_only(query, semantic_index, budget)
                .into_iter()
                .map(|m| RetrievalResult {
                    file_id: m.file_id,
                    range: m.range,
                    total_score: m.score,
                    lexical_score: 0.0,
                    semantic_score: m.score,
                    symbol: None,
                    relationship: None,
                })
                .collect()
        }
        RetrievalMode::Hybrid => {
            combine_results(query, index, semantic_index, budget, config)
        }
    }
}

fn lexical_only(query: &str, index: &Index, budget: &TokenBudget) -> Vec<LexicalMatch> {
    let search_query = crate::search::SearchQuery {
        pattern: query.to_string(),
        mode: crate::search::SearchMode::CaseInsensitive,
        file_ids: None,
        max_results: budget.max_files * 100,
    };

    crate::search::search(&search_query, index, budget).unwrap_or_default()
}

fn semantic_only(query: &str, semantic_index: &SemanticIndex, budget: &TokenBudget) -> Vec<SemanticMatch> {
    semantic_index.search(query, budget.max_files * 10)
}

fn combine_results(
    query: &str,
    index: &Index,
    semantic_index: &SemanticIndex,
    budget: &TokenBudget,
    config: &HybridConfig,
) -> Vec<RetrievalResult> {
    let lex_matches = lexical_only(query, index, budget);
    let sem_matches = semantic_only(query, semantic_index, budget);

    let mut lex_scores: HashMap<String, f64> = HashMap::new();
    for (i, m) in lex_matches.iter().enumerate() {
        let key = format!("{}:{}", m.file_id.0, m.range.start_byte);
        let score = 1.0 / (60.0 + i as f64);
        lex_scores.insert(key, score);
    }

    let mut sem_scores: HashMap<String, f64> = HashMap::new();
    for m in &sem_matches {
        let key = format!("{}:{}", m.file_id.0, m.range.start_byte);
        sem_scores.insert(key, m.score);
    }

    let k = 60.0;
    let mut combined: HashMap<String, (f64, FileId, Range)> = HashMap::new();

    for (i, m) in lex_matches.iter().enumerate() {
        let key = format!("{}:{}", m.file_id.0, m.range.start_byte);
        let lex_rrf = 1.0 / (k + i as f64);
        let entry = combined.entry(key).or_insert_with(|| (0.0, m.file_id, m.range.clone()));
        entry.0 += config.lexical_weight * lex_rrf;
    }

    for m in &sem_matches {
        let key = format!("{}:{}", m.file_id.0, m.range.start_byte);
        let sem_rrf = m.score;
        let entry = combined.entry(key).or_insert_with(|| (0.0, m.file_id, m.range.clone()));
        entry.0 += config.semantic_weight * sem_rrf;
    }

    let mut results: Vec<RetrievalResult> = combined
        .into_iter()
        .map(|(key, (total_score, file_id, range))| {
            let lex_score = lex_scores.get(&key).copied().unwrap_or(0.0);
            let sem_score = sem_scores.get(&key).copied().unwrap_or(0.0);
            RetrievalResult {
                file_id,
                range,
                total_score,
                lexical_score: lex_score,
                semantic_score: sem_score,
                symbol: None,
                relationship: None,
            }
        })
        .collect();

    results.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(budget.max_files * 10);

    results
}

pub fn build_semantic_index(index: &Index) -> SemanticIndex {
    let mut fragments: Vec<semantic::CodeFragment> = Vec::new();

    for file in index.files() {
        let content = match index.get_content(file.id) {
            Some(c) => c,
            None => continue,
        };

        let text = String::from_utf8_lossy(content);
        let lines: Vec<&str> = text.lines().collect();

        let mut start_byte = 0usize;
        for (i, line) in lines.iter().enumerate() {
            let line_text = line.trim();
            if line_text.is_empty() || line_text.starts_with("//") || line_text.starts_with("#") {
                start_byte += line.len() + 1;
                continue;
            }

            fragments.push(semantic::CodeFragment {
                id: format!("{}:{}", file.id.0, i),
                file_id: file.id,
                range: Range::new(start_byte, start_byte + line.len(), i as u32 + 1, i as u32 + 1),
                text: line.to_string(),
            });

            start_byte += line.len() + 1;
        }
    }

    let mut semantic_index = SemanticIndex::new();
    semantic_index.build(fragments);
    semantic_index
}

#[derive(Clone)]
pub struct RetrievalEngine {
    semantic_index: Arc<RwLock<SemanticIndex>>,
    config: HybridConfig,
}

impl RetrievalEngine {
    pub fn new() -> Self {
        Self {
            semantic_index: Arc::new(RwLock::new(SemanticIndex::new())),
            config: HybridConfig::default(),
        }
    }

    pub fn build_index(&self, index: &Index) {
        let mut guard = self.semantic_index.write().unwrap();
        *guard = build_semantic_index(index);
    }

    pub fn build_index_from(&self, semantic_index: crate::semantic::SemanticIndex) {
        let mut guard = self.semantic_index.write().unwrap();
        *guard = semantic_index;
    }

    pub fn search(&self, query: &str, index: &Index, budget: &TokenBudget) -> Vec<RetrievalResult> {
        let guard = self.semantic_index.read().unwrap();
        hybrid_search(query, index, &guard, budget, &self.config)
    }

    pub fn get_semantic_index(&self) -> std::sync::RwLockReadGuard<'_, SemanticIndex> {
        self.semantic_index.read().unwrap()
    }
}

impl Default for RetrievalEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_query_natural_language() {
        assert_eq!(analyze_query("How does the cache work?"), RetrievalMode::Hybrid);
        assert_eq!(analyze_query("What is the token budget?"), RetrievalMode::Hybrid);
    }

    #[test]
    fn test_analyze_query_identifier() {
        assert_eq!(analyze_query("calculate_sum"), RetrievalMode::Lexical);
        assert_eq!(analyze_query("MyStruct::new"), RetrievalMode::Lexical);
    }

    #[test]
    fn test_analyze_query_long() {
        let query = "How do I implement the authentication middleware for the API?";
        assert_eq!(analyze_query(query), RetrievalMode::Hybrid);
    }
}
