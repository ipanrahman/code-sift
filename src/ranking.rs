//! Candidate ranking based on structural signals.

use crate::types::{Relationship, Symbol};

/// Relevance score for a candidate.
#[derive(Debug, Clone)]
pub struct RelevanceScore {
    pub total: f64,
    pub symbol_match: f64,
    pub definition: f64,
    pub reference: f64,
    pub caller: f64,
    pub callee: f64,
    pub test: f64,
    pub lexical: f64,
    pub semantic: f64,
}

impl Default for RelevanceScore {
    fn default() -> Self {
        Self {
            total: 0.0,
            symbol_match: 0.0,
            definition: 0.0,
            reference: 0.0,
            caller: 0.0,
            callee: 0.0,
            test: 0.0,
            lexical: 0.0,
            semantic: 0.0,
        }
    }
}

/// Ranked candidate for retrieval.
#[derive(Debug, Clone)]
pub struct RankedCandidate {
    pub symbol: Symbol,
    pub score: RelevanceScore,
    pub relationship: Option<Relationship>,
}

/// Rank candidates based on structural and semantic signals.
pub fn rank_candidates(
    candidates: Vec<(Symbol, Option<Relationship>)>,
    semantic_scores: &[f64],
    query: &str,
) -> Vec<RankedCandidate> {
    let mut ranked: Vec<RankedCandidate> = candidates
        .into_iter()
        .enumerate()
        .map(|(i, (symbol, relationship))| {
            let semantic_score = semantic_scores.get(i).copied().unwrap_or(0.0);
            let score = compute_score(&symbol, relationship.as_ref(), query, semantic_score);
            RankedCandidate {
                symbol,
                score,
                relationship,
            }
        })
        .collect();

    ranked.sort_by(|a, b| b.score.total.partial_cmp(&a.score.total).unwrap_or(std::cmp::Ordering::Equal));

    ranked
}

fn compute_score(
    symbol: &Symbol,
    relationship: Option<&Relationship>,
    query: &str,
    semantic_score: f64,
) -> RelevanceScore {
    let mut score = RelevanceScore::default();

    // Exact symbol match
    if symbol.name == query {
        score.symbol_match = 100.0;
    } else if symbol.name.to_lowercase().contains(&query.to_lowercase()) {
        score.symbol_match = 50.0;
    }

    // Definition bonus
    if symbol.kind.is_definition() {
        score.definition = 80.0;
    }

    // Reference penalty
    if symbol.kind.is_reference() {
        score.reference = -20.0;
    }

    // Relationship bonuses
    if let Some(rel) = relationship {
        match rel {
            Relationship::Calls | Relationship::CalleeOf => score.caller = 60.0,
            Relationship::References => score.reference = 40.0,
            Relationship::Tests | Relationship::TestFor => score.test = 50.0,
            Relationship::Implements | Relationship::ExtendedBy => score.definition = 70.0,
            _ => {}
        }
    }

    // Semantic score: scale from [0,1] to [0, 100]
    score.semantic = semantic_score * 100.0;

    // Calculate total
    score.total = score.symbol_match + score.definition + score.reference
        + score.caller + score.callee + score.test + score.lexical + score.semantic;

    score
}
