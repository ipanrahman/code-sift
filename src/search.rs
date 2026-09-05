//! Lexical search implementation.

use crate::error::Result;
use crate::index::{Index, LexicalMatch, TokenBudget};
use crate::types::{FileId, Range};
use rayon::prelude::*;

/// Search mode for lexical queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Match exact text.
    #[default]
    Exact,
    /// Match as regex pattern.
    Regex,
    /// Match as identifier (word boundary).
    Identifier,
    /// Case-insensitive match.
    CaseInsensitive,
}

/// Search query parameters.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub pattern: String,
    pub mode: SearchMode,
    pub file_ids: Option<Vec<FileId>>,
    pub max_results: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            mode: SearchMode::Exact,
            file_ids: None,
            max_results: 1000,
        }
    }
}

/// Perform lexical search across the index.
pub fn search(
    query: &SearchQuery,
    index: &Index,
    budget: &TokenBudget,
) -> Result<Vec<LexicalMatch>> {
    let max_results = query.max_results.min(budget.max_files * 100);

    // Get files to search
    let files: Vec<FileId> = match &query.file_ids {
        Some(ids) => ids.clone(),
        None => index.files().map(|f| f.id).collect(),
    };

    let pattern = &query.pattern;
    let mode = query.mode;

    let matches: Vec<LexicalMatch> = files
        .par_iter()
        .filter_map(|&file_id| {
            let content = index.get_content(file_id)?;
            Some(search_file_content(
                content,
                file_id,
                pattern,
                mode,
                max_results,
            ))
        })
        .flatten()
        .collect();

    Ok(matches)
}

/// Search a single file's content.
fn search_file_content(
    content: &[u8],
    file_id: FileId,
    pattern: &str,
    mode: SearchMode,
    max_per_file: usize,
) -> Vec<LexicalMatch> {
    let text = String::from_utf8_lossy(content);
    let mut matches = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        let line_number = line_number as u32 + 1;

        let is_match = match mode {
            SearchMode::Exact => line.contains(pattern),
            SearchMode::CaseInsensitive => line.to_lowercase().contains(&pattern.to_lowercase()),
            SearchMode::Identifier => line.split_whitespace().any(|word| word == pattern),
            SearchMode::Regex => regex::Regex::new(pattern)
                .map(|re| re.is_match(line))
                .unwrap_or(false),
        };

        if is_match {
            let start_byte = text
                .lines()
                .take(line_number as usize - 1)
                .map(|l| l.len() + 1)
                .sum::<usize>();
            let end_byte = start_byte + line.len();

            matches.push(LexicalMatch {
                file_id,
                range: Range::new(start_byte, end_byte, line_number, line_number),
                line: line.to_string(),
                line_number,
            });

            if matches.len() >= max_per_file {
                break;
            }
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let content = b"fn main() {\n    println!(\"hello\");\n}";
        let matches = search_file_content(content, FileId::new(0), "main", SearchMode::Exact, 100);
        assert!(!matches.is_empty());
    }

    #[test]
    fn test_no_match() {
        let content = b"fn foo() {}";
        let matches = search_file_content(content, FileId::new(0), "bar", SearchMode::Exact, 100);
        assert!(matches.is_empty());
    }
}
