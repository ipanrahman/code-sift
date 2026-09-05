//! CodeSift - Token-efficient code intelligence engine.

pub use crate::context::{ContextFragment, ContextPlan};
pub use crate::graph::ReferenceIndex;
pub use crate::index::TokenBudget;
pub use crate::mcp::McpServer;
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

use crate::context::plan_context;
use crate::error::Result;
use crate::index::{Index, LexicalMatch};
use crate::parser::Parser;
use crate::ranking::rank_candidates;
use crate::search::{search, SearchMode, SearchQuery};
use crate::types::{FileId, Relationship, Symbol, SymbolId};

/// Main CodeSift engine.
#[derive(Clone)]
pub struct CodeSift {
    index: Index,
    parser: Parser,
    references: ReferenceIndex,
}

impl CodeSift {
    /// Create a new CodeSift instance.
    pub fn new() -> Self {
        Self {
            index: Index::new(),
            parser: Parser::new(),
            references: ReferenceIndex::new(),
        }
    }

    /// Open and index a repository.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let repo = repository::Repository::open(path)?;

        let mut codesift = Self::new();

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
}

impl Default for CodeSift {
    fn default() -> Self {
        Self::new()
    }
}
