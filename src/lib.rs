//! CodeSift - Token-efficient code intelligence engine.

pub use crate::context::{ContextFragment, ContextPlan};
pub use crate::graph::ReferenceIndex;
pub use crate::index::TokenBudget;
pub use crate::mcp::McpServer;
pub use crate::storage::Storage;
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
pub mod storage;
pub mod types;
pub mod watcher;

#[cfg(feature = "semantic")]
pub mod semantic;
#[cfg(feature = "semantic")]
pub mod retrieval;

#[cfg(test)]
pub mod watcher_tests;
#[cfg(test)]
pub mod reference_tests;

use crate::context::plan_context;
use crate::error::Result;
use crate::index::{Index, LexicalMatch};
use crate::parser::Parser;
use crate::ranking::rank_candidates;
use crate::search::{search, SearchMode, SearchQuery};
use crate::types::{FileEntry, FileId, Relationship, Symbol, SymbolId};


/// Main CodeSift engine.
pub struct CodeSift {
    index: Index,
    parser: Parser,
    references: ReferenceIndex,
    repo_path: Option<std::path::PathBuf>,
    #[cfg(feature = "semantic")]
    retrieval_engine: retrieval::RetrievalEngine,
}

impl Clone for CodeSift {
    fn clone(&self) -> Self {
        Self {
            index: self.index.clone(),
            parser: self.parser.clone(),
            references: self.references.clone(),
            repo_path: self.repo_path.clone(),
            #[cfg(feature = "semantic")]
            retrieval_engine: self.retrieval_engine.clone(),
        }
    }
}

impl CodeSift {
    /// Create a new CodeSift instance.
    pub fn new() -> Self {
        Self {
            index: Index::new(),
            parser: Parser::new(),
            references: ReferenceIndex::new(),
            repo_path: None,
            #[cfg(feature = "semantic")]
            retrieval_engine: retrieval::RetrievalEngine::new(),
        }
    }

    /// Open and index a repository.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let repo = repository::Repository::open(&path)?;

        let mut codesift = Self::new();
        codesift.repo_path = Some(path.clone());

        let mut all_calls: Vec<crate::parser::CallReference> = Vec::new();
        let mut all_parsed_symbols: Vec<(SymbolId, String)> = Vec::new();

        for file in repo.files() {
            let file_id = codesift.index.add_file(file.clone());

            if file.is_binary || file.is_vendor {
                continue;
            }

            let content = repo.read_file(file_id)?;
            codesift.index.store_content(file_id, content.clone());

            if let Ok(parsed) = codesift.parser.parse(&content, file.language, file_id) {
                let mut parsed_symbols = Vec::new();
                for symbol in &parsed.symbols {
                    let sym_id = codesift.index.add_symbol(symbol.clone());
                    parsed_symbols.push((sym_id, symbol.name.clone()));
                }

                for call in &parsed.calls {
                    all_calls.push(crate::parser::CallReference {
                        caller: call.caller,
                        caller_name: call.caller_name.clone(),
                        callee_name: call.callee_name.clone(),
                        range: call.range,
                    });
                }
                all_parsed_symbols.extend(parsed_symbols);
            }
        }

        codesift.resolve_all_references(&all_calls, &all_parsed_symbols);

        // Build semantic index if feature is enabled
        #[cfg(feature = "semantic")]
        {
            codesift.retrieval_engine.build_index(&codesift.index);
        }

        Ok(codesift)
    }

    /// Save index to cache for faster startup.
    pub fn save_cache(&self) -> Result<()> {
        let path = self.repo_path.as_ref().ok_or_else(|| {
            crate::error::Error::Index("No repository path set".to_string())
        })?;
        let storage = Storage::new(path);
        storage.save(&self.index, path).map_err(|e| {
            crate::error::Error::Index(format!("Failed to save cache: {}", e))
        })
    }

    /// Open repository, using cache if available and valid.
    pub fn open_cached(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let storage = Storage::new(&path);

        // Try to load from cache
        if storage.cache_exists() {
            if let Ok((index, references)) = storage.load() {
                // Check if files have changed
                let cached_hashes = storage.get_file_hashes().unwrap_or_default();
                let repo = repository::Repository::open(&path).ok();

                let mut current_hashes = std::collections::HashMap::new();
                if let Some(ref repo) = repo {
                    for file in repo.files() {
                        if !file.is_binary && !file.is_vendor {
                            let hash = crate::storage::hash_file(&file.path);
                            current_hashes.insert(
                                file.path.to_string_lossy().to_string(),
                                (hash, file.modified_at),
                            );
                        }
                    }
                }

                if !crate::storage::files_changed(&cached_hashes, &current_hashes) {
                    // Cache is valid, use it
                    let mut codesift = Self::new();
                    codesift.index = index;
                    codesift.references = references;
                    codesift.repo_path = Some(path);

                    // Load semantic index from cache if available
                    #[cfg(feature = "semantic")]
                    {
                        if let Ok(Some(semantic_index)) = storage.load_semantic_index() {
                            let mut engine = codesift.retrieval_engine;
                            engine.build_index_from(semantic_index);
                            codesift.retrieval_engine = engine;
                        } else {
                            codesift.retrieval_engine.build_index(&codesift.index);
                        }
                    }

                    return Ok(codesift);
                }
            }
        }

        // Cache miss or invalid - build from scratch
        let codesift = Self::open(&path)?;

        // Try to save cache
        if let Err(e) = codesift.save_cache() {
            eprintln!("Warning: failed to save cache: {}", e);
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

    /// Search with semantic/AI understanding (requires semantic feature).
    #[cfg(feature = "semantic")]
    pub fn semantic_search(&self, query: &str, budget: Option<TokenBudget>) -> Vec<retrieval::RetrievalResult> {
        let budget = budget.unwrap_or_default();
        self.retrieval_engine.search(query, &self.index, &budget)
    }

    /// Analyze a query to determine the best retrieval mode.
    #[cfg(feature = "semantic")]
    pub fn analyze_query(query: &str) -> retrieval::RetrievalMode {
        retrieval::analyze_query(query)
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
        let mut semantic_scores: Vec<f64> = Vec::new();

        // Symbol matches
        for symbol in self.find_symbol(query) {
            candidates.push((symbol.clone(), Some(Relationship::References)));
            semantic_scores.push(0.0);
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
                semantic_scores.push(0.0);
            }
        }

        // Semantic matches - add as raw candidates when feature is enabled
        #[cfg(feature = "semantic")]
        {
            let semantic_results = self.semantic_search(query, Some(budget));
            for r in semantic_results {
                let sym = Symbol {
                    id: SymbolId::new(0),
                    name: format!("semantic:{}:{}", r.file_id.0, r.range.start_byte),
                    kind: crate::types::SymbolKind::Variable,
                    file_id: r.file_id,
                    range: r.range,
                    parent: None,
                    visibility: crate::types::Visibility::Public,
                    signature: None,
                };
                candidates.push((sym, None));
                semantic_scores.push(r.semantic_score);
            }
        }

        // Step 2: Rank candidates
        let ranked = rank_candidates(candidates, &semantic_scores, query);

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

        let mut all_calls: Vec<crate::parser::CallReference> = Vec::new();
        let mut all_parsed_symbols: Vec<(SymbolId, String)> = Vec::new();

        for file in repo.files() {
            let file_id = self.index.add_file(file.clone());

            if file.is_binary || file.is_vendor {
                continue;
            }

            let content = repo.read_file(file_id)?;
            self.index.store_content(file_id, content.clone());

            if let Ok(parsed) = self.parser.parse(&content, file.language, file_id) {
                let mut parsed_symbols = Vec::new();
                for symbol in &parsed.symbols {
                    let sym_id = self.index.add_symbol(symbol.clone());
                    parsed_symbols.push((sym_id, symbol.name.clone()));
                }

                for call in &parsed.calls {
                    all_calls.push(crate::parser::CallReference {
                        caller: call.caller,
                        caller_name: call.caller_name.clone(),
                        callee_name: call.callee_name.clone(),
                        range: call.range,
                    });
                }
                all_parsed_symbols.extend(parsed_symbols);
            }
        }

        self.resolve_all_references(&all_calls, &all_parsed_symbols);

        // Rebuild semantic index
        #[cfg(feature = "semantic")]
        {
            self.retrieval_engine.build_index(&self.index);
        }

        Ok(())
    }

    /// Get the repository path.
    pub fn repo_path(&self) -> Option<&std::path::Path> {
        self.repo_path.as_deref()
    }

    /// Resolve all call references using the global symbol index.
    /// This handles both intra-file and cross-file references.
    pub fn resolve_all_references(
        &mut self,
        calls: &[crate::parser::CallReference],
        _parsed_symbols: &[(SymbolId, String)],
    ) {
        // Build global symbol name -> SymbolId map
        let mut global_index: std::collections::HashMap<String, Vec<SymbolId>> = std::collections::HashMap::new();
        for symbol in self.index.symbols() {
            global_index
                .entry(symbol.name.clone())
                .or_default()
                .push(symbol.id);
        }

        for call in calls {
            let target_name = call.callee_name.trim_start_matches("self::");

            // Look up callee by name in global index
            if let Some(callee_ids) = global_index.get(target_name) {
                // Prefer same-file resolution
                let target_id = if let Some(caller_id) = call.caller {
                    if let Some(caller_symbol) = self.index.get_symbol(caller_id) {
                        callee_ids.iter().find_map(|&id| {
                            self.index.get_symbol(id).filter(|s| s.file_id == caller_symbol.file_id).map(|_| id)
                        }).or_else(|| callee_ids.first().copied())
                    } else {
                        callee_ids.first().copied()
                    }
                } else {
                    callee_ids.first().copied()
                };

                if let Some(target_id) = target_id {
                    if let Some(caller_id) = call.caller {
                        self.references.add_reference(
                            caller_id,
                            target_id,
                            Relationship::Calls,
                        );
                    }
                }
            }
        }
    }

    /// Resolve call references to actual symbol IDs using a local symbol index.
    pub fn resolve_references(
        &mut self,
        calls: &[crate::parser::CallReference],
        parsed_symbols: &[(SymbolId, String)],
    ) {
        let mut local_index: std::collections::HashMap<String, SymbolId> = std::collections::HashMap::new();
        for (id, name) in parsed_symbols {
            local_index.insert(name.clone(), *id);
        }

        for call in calls {
            if let Some(callee_id) = call.callee_name.strip_prefix("self::") {
                if let Some(&target_id) = local_index.get(callee_id) {
                    if let Some(caller_id) = call.caller {
                        self.references.add_reference(
                            caller_id,
                            target_id,
                            Relationship::Calls,
                        );
                    }
                }
            } else if let Some(&target_id) = local_index.get(&call.callee_name) {
                if let Some(caller_id) = call.caller {
                    self.references.add_reference(
                        caller_id,
                        target_id,
                        Relationship::Calls,
                    );
                }
            }
        }
    }

    /// Find all references to a symbol across the codebase.
    pub fn find_references(&self, name: &str) -> Vec<(&Symbol, Relationship)> {
        let mut results = Vec::new();

        // Find all symbols with this name
        for id in self.index.find_symbols_by_name(name) {
            // Get incoming references for this symbol
            for (from_id, rel) in self.references.get_incoming(id) {
                if let Some(from_symbol) = self.index.get_symbol(*from_id) {
                    results.push((from_symbol, *rel));
                }
            }
        }

        results
    }

    /// Get the definition site for a symbol by name.
    /// Returns all definitions matching the name.
    pub fn get_definition(&self, name: &str) -> Vec<&Symbol> {
        self.find_symbol(name)
    }

    /// Watch repository for filesystem changes and re-index incrementally.
    /// Blocks until interrupted (Ctrl+C).
    pub fn watch(&mut self) -> Result<()> {
        let repo_path = self.repo_path.clone().ok_or_else(|| {
            crate::error::Error::Index("No repository path set".to_string())
        })?;

        let mut watcher = crate::FsWatcher::new(&repo_path)?;
        watcher.watch(&repo_path)?;

        eprintln!("Watching {} for changes...", repo_path.display());

        loop {
            if let Some(change) = watcher.poll_changes() {
                eprintln!("Change detected: {:?}", change);
                if let Err(e) = self.process_change(change) {
                    eprintln!("Error processing change: {}", e);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
}

impl Default for CodeSift {
    fn default() -> Self {
        Self::new()
    }
}
