//! CodeSift - Token-efficient code intelligence engine.

pub use crate::context::{ContextFragment, ContextPlan};
pub use crate::graph::ReferenceIndex;
pub use crate::index::TokenBudget;
pub use crate::mcp::McpServer;
pub use crate::watcher::{FileChange, FsWatcher};
pub mod context;
pub mod error;
pub mod graph;
pub mod index;
pub mod mcp;
pub mod parser;
pub mod ranking;
pub mod repository;
pub mod search;
pub mod types;
pub mod watcher;

#[cfg(test)]
pub mod watcher_tests;

use crate::context::plan_context;
use crate::error::Result;
use crate::index::{Index, LexicalMatch};
use crate::parser::Parser;
use crate::ranking::rank_candidates;
use crate::search::{search, SearchMode, SearchQuery};
use crate::types::{FileEntry, FileId, Relationship, Symbol, SymbolId};


/// Main CodeSift engine.
#[derive(Clone)]
pub struct CodeSift {
    index: Index,
    parser: Parser,
    references: ReferenceIndex,
    repo_path: Option<std::path::PathBuf>,
}

impl CodeSift {
    /// Create a new CodeSift instance.
    pub fn new() -> Self {
        Self {
            index: Index::new(),
            parser: Parser::new(),
            references: ReferenceIndex::new(),
            repo_path: None,
        }
    }

    /// Open and index a repository.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let repo = repository::Repository::open(&path)?;

        let mut codesift = Self {
            index: Index::new(),
            parser: Parser::new(),
            references: ReferenceIndex::new(),
            repo_path: Some(path.clone()),
        };

        // Index all files
        for file in repo.files() {
            let file_id = codesift.index.add_file(file.clone());

            // Skip non-text files
            if file.is_binary || file.is_vendor {
                continue;
            }

            // Read and store content
            let content = repo.read_file(file_id)?;
            codesift.index.store_content(file_id, content.clone());

            // Parse and extract symbols and references
            if let Ok(parsed) = codesift.parser.parse(&content, file.language, file_id) {
                // Add symbols to index
                for symbol in &parsed.symbols {
                    let sym_id = codesift.index.add_symbol(symbol.clone());

                    // Build call graph
                    for call in &parsed.calls {
                        if let Some(caller_id) = call.caller {
                            codesift.references.add_reference(
                                caller_id,
                                sym_id,
                                Relationship::Calls,
                            );
                        }
                    }
                }

                // Store references
                for (sym_id, _, rel) in &parsed.references {
                    codesift.index.add_reference(*sym_id, *sym_id, rel.clone());
                }
            }
        }

        Ok(codesift)
    }

    /// Search for text across the codebase.
    pub fn search(&self, query: &str, budget: Option<TokenBudget>) -> Result<Vec<LexicalMatch>> {
        let budget = budget.unwrap_or_default();
        let search_query = SearchQuery {
            pattern: query.to_string(),
            mode: SearchMode::Exact,
            file_ids: None,
            max_results: 1000,
        };
        search(&search_query, &self.index, &budget)
    }

    /// Search for a symbol by name.
    pub fn find_symbol(&self, name: &str) -> Vec<&Symbol> {
        self.index
            .find_symbols_by_name(name)
            .iter()
            .filter_map(|id| self.index.get_symbol(*id))
            .collect()
    }

    /// Get symbol by ID.
    pub fn get_symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.index.get_symbol(id)
    }

    /// Get file by ID.
    pub fn get_file(&self, id: FileId) -> Option<&crate::types::FileEntry> {
        self.index.get_file(id)
    }

    /// Get file content.
    pub fn get_content(&self, file_id: FileId) -> Option<&Vec<u8>> {
        self.index.get_content(file_id)
    }

    /// Get source text for a range.
    pub fn get_source(&self, file_id: FileId, range: &crate::types::Range) -> Option<String> {
        let content = self.index.get_content(file_id)?;
        let bytes = content.get(range.start_byte..range.end_byte)?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    /// Get callers of a symbol.
    pub fn get_callers(&self, id: SymbolId) -> Vec<SymbolId> {
        self.references.get_callers(id).to_vec()
    }

    /// Get callees of a symbol.
    pub fn get_callees(&self, id: SymbolId) -> Vec<SymbolId> {
        self.references.get_callees(id).to_vec()
    }

    /// Get callers up to a depth.
    pub fn get_callers_upto(&self, id: SymbolId, depth: usize) -> Vec<(SymbolId, usize)> {
        self.references.traverse_callers(id, depth)
    }

    /// Get callees up to a depth.
    pub fn get_callees_upto(&self, id: SymbolId, depth: usize) -> Vec<(SymbolId, usize)> {
        self.references.traverse_callees(id, depth)
    }

    /// Plan context for an agent query.
    pub fn plan_context(
        &self,
        query: &str,
        budget: Option<TokenBudget>,
    ) -> Result<ContextPlan> {
        let budget = budget.unwrap_or_default();

        // Step 1: Find candidates via symbol search and lexical search
        let mut candidates: Vec<(Symbol, Option<Relationship>)> = Vec::new();

        // Symbol matches
        for symbol in self.find_symbol(query) {
            candidates.push((symbol.clone(), Some(Relationship::References)));
        }

        // Lexical matches - add as raw candidates
        if let Ok(matches) = self.search(query, Some(budget)) {
            for m in matches {
                let sym = Symbol {
                    id: SymbolId::new(0),
                    name: format!("match:{}", m.line.trim()),
                    kind: crate::types::SymbolKind::Variable,
                    file_id: m.file_id,
                    range: m.range,
                    parent: None,
                    visibility: crate::types::Visibility::Public,
                    signature: None,
                };
                candidates.push((sym, None));
            }
        }

        // Step 2: Rank candidates
        let ranked = rank_candidates(candidates, query);

        // Step 3: Plan context with token budget
        let plan = plan_context(ranked, &budget, |file_id, range| {
            self.get_source(file_id, range)
        });

        Ok(plan)
    }

    /// Get all symbols.
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.index.symbols()
    }

    /// Get file count.
    pub fn file_count(&self) -> usize {
        self.index.file_count()
    }

    /// Get symbol count.
    pub fn symbol_count(&self) -> usize {
        self.index.symbol_count()
    }

    /// Process a file change event (for incremental indexing).
    pub fn process_change(&mut self, change: FileChange) -> Result<()> {
        let repo_path = self.repo_path.clone().ok_or_else(|| {
            crate::error::Error::Index("No repository path set".to_string())
        })?;

        match change {
            FileChange::Created(path) | FileChange::Modified(path) => {
                self.reindex_file(&repo_path, &path)?;
            }
            FileChange::Deleted(path) => {
                self.index.remove_file_by_path(&path);
            }
        }
        Ok(())
    }

    /// Re-index a single file.
    pub fn reindex_file(&mut self, repo_path: &std::path::Path, file_path: &std::path::Path) -> Result<()> {
        // Read the file
        let full_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            repo_path.join(file_path)
        };

        // Check if file exists
        if !full_path.exists() {
            // File was deleted
            self.index.remove_file_by_path(&full_path);
            return Ok(());
        }

        // Detect language from extension
        let ext = full_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let language = crate::types::Language::from_extension(ext);

        // Read content
        let content = std::fs::read(&full_path)?;

        // Create file entry (use FileEntry::from_path for proper initialization)
        let mut entry = FileEntry::from_path(full_path.clone())
            .ok_or_else(|| crate::error::Error::FileNotFound(full_path.display().to_string()))?;
        entry.is_binary = false;
        entry.is_vendor = false;

        // Update or add file
        let file_id = if self.index.has_file(&full_path) {
            self.index.remove_file_by_path(&full_path);
            self.index.add_file(entry)
        } else {
            self.index.add_file(entry)
        };

        // Store content
        self.index.store_content(file_id, content.clone());

        // Parse and update symbols
        if let Ok(parsed) = self.parser.parse(&content, language, file_id) {
            // Remove old symbols and add new ones
            // Note: references in parsed format are (SymbolId, String, Relationship)
            // We only need the Relationship for index storage
            self.index.update_file_symbols(
                file_id,
                parsed.symbols,
                Vec::new(), // References stored separately in the reference index
            );
        }

        Ok(())
    }

    /// Re-index all files (full rebuild).
    pub fn reindex(&mut self) -> Result<()> {
        let repo_path = self.repo_path.as_ref().ok_or_else(|| {
            crate::error::Error::Index("No repository path set".to_string())
        })?;

        let repo = repository::Repository::open(repo_path)?;

        // Clear and rebuild
        self.index = Index::new();
        self.references = ReferenceIndex::new();

        for file in repo.files() {
            let file_id = self.index.add_file(file.clone());

            if file.is_binary || file.is_vendor {
                continue;
            }

            let content = repo.read_file(file_id)?;
            self.index.store_content(file_id, content.clone());

            if let Ok(parsed) = self.parser.parse(&content, file.language, file_id) {
                for symbol in &parsed.symbols {
                    let _ = self.index.add_symbol(symbol.clone());
                }

                for (sym_id, _, rel) in &parsed.references {
                    self.index.add_reference(*sym_id, *sym_id, rel.clone());
                }
            }
        }

        Ok(())
    }

    /// Get the repository path.
    pub fn repo_path(&self) -> Option<&std::path::Path> {
        self.repo_path.as_deref()
    }
}

impl Default for CodeSift {
    fn default() -> Self {
        Self::new()
    }
}
