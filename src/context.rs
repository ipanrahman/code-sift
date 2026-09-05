//! Context planning - produce minimal sufficient context from ranked candidates.

use crate::index::TokenBudget;
use crate::ranking::RankedCandidate;
use crate::types::{FileId, Range};
use std::collections::BTreeMap;

/// Context fragment representing a piece of code to return.
#[derive(Debug, Clone)]
pub struct ContextFragment {
    pub file_id: FileId,
    pub range: Range,
    pub symbol_name: Option<String>,
    pub content: String,
    pub token_count: usize,
    pub depth: usize,
}

/// Context plan - the output of the context planner.
#[derive(Debug, Clone)]
pub struct ContextPlan {
    pub fragments: Vec<ContextFragment>,
    pub total_tokens: usize,
    pub total_files: usize,
    pub budget: TokenBudget,
}

impl ContextPlan {
    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }

    pub fn len(&self) -> usize {
        self.fragments.len()
    }
}

/// Plan minimal context from ranked candidates.
pub fn plan_context(
    candidates: Vec<RankedCandidate>,
    budget: &TokenBudget,
    get_source: impl Fn(FileId, &Range) -> Option<String>,
) -> ContextPlan {
    let mut fragments: Vec<ContextFragment> = Vec::new();
    let mut total_tokens = 0;
    let mut total_files: usize = 0;
    let mut files_used: std::collections::HashSet<FileId> = std::collections::HashSet::new();
    // Use (file_id, start_byte) as unique identifier to avoid duplicates
    let mut ranges_used: std::collections::HashSet<(FileId, usize)> = std::collections::HashSet::new();

    // Sort by score descending
    let mut candidates = candidates;
    candidates.sort_by(|a, b| b.score.total.partial_cmp(&a.score.total).unwrap_or(std::cmp::Ordering::Equal));

    for candidate in candidates {
        // Check budget constraints
        if fragments.len() >= budget.max_symbols {
            break;
        }

        if total_files >= budget.max_files && !files_used.contains(&candidate.symbol.file_id) {
            continue;
        }

        // Avoid duplicates by range
        let range_key = (candidate.symbol.file_id, candidate.symbol.range.start_byte);
        if ranges_used.contains(&range_key) {
            continue;
        }

        // Get source content
        let content = match get_source(candidate.symbol.file_id, &candidate.symbol.range) {
            Some(c) => c,
            None => continue,
        };

        let tokens = estimate_tokens(&content);

        // Check if adding this would exceed budget
        if total_tokens + tokens > budget.max_tokens {
            // Try to fit a smaller portion
            if let Some(compact) = try_compact(&content, budget.max_tokens - total_tokens) {
                let tokens = estimate_tokens(&compact);
                total_tokens += tokens;
                files_used.insert(candidate.symbol.file_id);
                total_files = files_used.len();

                fragments.push(ContextFragment {
                    file_id: candidate.symbol.file_id,
                    range: candidate.symbol.range,
                    symbol_name: Some(candidate.symbol.name),
                    content: compact,
                    token_count: tokens,
                    depth: 0,
                });
                ranges_used.insert((candidate.symbol.file_id, candidate.symbol.range.start_byte));
            }
            continue;
        }

        total_tokens += tokens;
        files_used.insert(candidate.symbol.file_id);
        total_files = files_used.len();

        fragments.push(ContextFragment {
            file_id: candidate.symbol.file_id,
            range: candidate.symbol.range,
            symbol_name: Some(candidate.symbol.name),
            content,
            token_count: tokens,
            depth: 0,
        });
        ranges_used.insert((candidate.symbol.file_id, candidate.symbol.range.start_byte));
    }

    ContextPlan {
        fragments,
        total_tokens,
        total_files,
        budget: *budget,
    }
}

/// Merge overlapping ranges from the same file.
pub fn merge_ranges(ranges: &[(FileId, Range)]) -> Vec<(FileId, Range)> {
    // Group by file
    let mut by_file: BTreeMap<FileId, Vec<Range>> = BTreeMap::new();
    for (file_id, range) in ranges {
        by_file.entry(*file_id).or_default().push(*range);
    }

    let mut merged: Vec<(FileId, Range)> = Vec::new();

    for (file_id, ranges) in by_file {
        let mut ranges = ranges;
        ranges.sort_by_key(|r| r.start_byte);

        let mut current: Option<Range> = None;
        for range in ranges {
            if let Some(ref mut curr) = current {
                if curr.end_byte >= range.start_byte {
                    // Overlapping - extend
                    curr.end_byte = curr.end_byte.max(range.end_byte);
                    curr.end_line = curr.end_line.max(range.end_line);
                } else {
                    // Non-overlapping - emit current and start new
                    merged.push((file_id, *curr));
                    *curr = range;
                }
            } else {
                current = Some(range);
            }
        }

        if let Some(curr) = current {
            merged.push((file_id, curr));
        }
    }

    merged
}

/// Estimate token count (rough approximation).
pub fn estimate_tokens(text: &str) -> usize {
    // Simple heuristic: ~4 chars per token on average
    (text.len() + 3) / 4
}

/// Try to create a more compact version if full content doesn't fit.
fn try_compact(content: &str, max_tokens: usize) -> Option<String> {
    let target_chars = max_tokens * 4;

    if content.len() <= target_chars {
        return Some(content.to_string());
    }

    // Take first portion that fits
    let chars: Vec<char> = content.chars().collect();
    if chars.len() <= target_chars {
        return Some(content.to_string());
    }

    // Find a good cut point (end of line)
    let target_chars = target_chars.min(chars.len());
    let mut cut = target_chars;
    for (i, c) in chars.iter().enumerate().take(target_chars) {
        if *c == '\n' && i < target_chars - 20 {
            cut = i + 1;
        }
    }

    let compact = chars[..cut].iter().collect::<String>();
    Some(format!("{}...\n[{} more lines]", compact, chars[cut..].iter().filter(|c| **c == '\n').count()))
}
