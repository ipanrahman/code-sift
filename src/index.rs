//! Index structures for fast retrieval.

use crate::types::{
    FileEntry, FileId, Range, Relationship, Symbol, SymbolId,
};
use hashbrown::HashMap;
use std::path::{Path, PathBuf};

const INITIAL_FILE_CAPACITY: usize = 10_000;
const INITIAL_SYMBOL_CAPACITY: usize = 50_000;

/// Central index for code intelligence.
#[derive(Clone)]
pub struct Index {
    files: HashMap<FileId, FileEntry>,
    paths: HashMap<PathBuf, FileId>,
    symbols: HashMap<SymbolId, Symbol>,
    symbols_by_name: HashMap<String, Vec<SymbolId>>,
    symbols_by_file: HashMap<FileId, Vec<SymbolId>>,
    references: HashMap<SymbolId, Vec<(SymbolId, Relationship)>>,
    file_content: HashMap<FileId, Vec<u8>>,
    next_file_id: u64,
    next_symbol_id: u64,
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

impl Index {
    pub fn new() -> Self {
        Self {
            files: HashMap::with_capacity(INITIAL_FILE_CAPACITY),
            paths: HashMap::with_capacity(INITIAL_FILE_CAPACITY),
            symbols: HashMap::with_capacity(INITIAL_SYMBOL_CAPACITY),
            symbols_by_name: HashMap::new(),
            symbols_by_file: HashMap::with_capacity(INITIAL_FILE_CAPACITY),
            references: HashMap::new(),
            file_content: HashMap::with_capacity(INITIAL_FILE_CAPACITY),
            next_file_id: 0,
            next_symbol_id: 0,
        }
    }

    /// Add a file to the index.
    pub fn add_file(&mut self, entry: FileEntry) -> FileId {
        let id = FileId::new(self.next_file_id);
        self.next_file_id += 1;

        let mut entry = entry;
        entry.id = id;

        self.paths.insert(entry.path.clone(), id);
        self.files.insert(id, entry);

        id
    }

    /// Get a file by ID.
    pub fn get_file(&self, id: FileId) -> Option<&FileEntry> {
        self.files.get(&id)
    }

    /// Get a file by path.
    pub fn get_file_by_path(&self, path: &Path) -> Option<&FileEntry> {
        self.paths.get(path).and_then(|id| self.files.get(id))
    }

    /// Get all files.
    pub fn files(&self) -> impl Iterator<Item = &FileEntry> {
        self.files.values()
    }

    /// File count.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Add a symbol to the index.
    pub fn add_symbol(&mut self, mut symbol: Symbol) -> SymbolId {
        let id = SymbolId::new(self.next_symbol_id);
        self.next_symbol_id += 1;

        symbol.id = id;

        self.symbols.insert(id, symbol.clone());

        // Index by name
        self.symbols_by_name
            .entry(symbol.name.clone())
            .or_default()
            .push(id);

        // Index by file
        self.symbols_by_file
            .entry(symbol.file_id)
            .or_default()
            .push(id);

        id
    }

    /// Get a symbol by ID.
    pub fn get_symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(&id)
    }

    /// Get symbol mutably.
    pub fn get_symbol_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(&id)
    }

    /// Find symbols by name.
    pub fn find_symbols_by_name(&self, name: &str) -> Vec<SymbolId> {
        self.symbols_by_name.get(name).cloned().unwrap_or_default()
    }

    /// Get all symbols.
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.values()
    }

    /// Get symbol count.
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Get symbols for a file.
    pub fn symbols_in_file(&self, file_id: FileId) -> Vec<&Symbol> {
        self.symbols_by_file
            .get(&file_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.symbols.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Add a reference relationship between symbols.
    pub fn add_reference(&mut self, from: SymbolId, to: SymbolId, rel: Relationship) {
        self.references.entry(from).or_default().push((to, rel));
    }

    /// Get references from a symbol.
    pub fn get_references(&self, id: SymbolId) -> &[(SymbolId, Relationship)] {
        self.references.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Store file content.
    pub fn store_content(&mut self, id: FileId, content: Vec<u8>) {
        self.file_content.insert(id, content);
    }

    /// Get stored content.
    pub fn get_content(&self, id: FileId) -> Option<&Vec<u8>> {
        self.file_content.get(&id)
    }

    /// Clear stored content to free memory.
    pub fn clear_content(&mut self, id: FileId) {
        self.file_content.remove(&id);
    }

    /// Clear all content.
    pub fn clear_all_content(&mut self) {
        self.file_content.clear();
    }

    /// Remove a file and its associated data from the index.
    pub fn remove_file(&mut self, file_id: FileId) -> Option<FileEntry> {
        let entry = self.files.remove(&file_id)?;

        // Remove from paths map
        self.paths.remove(&entry.path);

        // Remove symbols belonging to this file
        if let Some(symbol_ids) = self.symbols_by_file.remove(&file_id) {
            for sym_id in symbol_ids {
                // Remove from symbols_by_name
                if let Some(symbol) = self.symbols.remove(&sym_id) {
                    if let Some(name_list) = self.symbols_by_name.get_mut(&symbol.name) {
                        name_list.retain(|id| *id != sym_id);
                        if name_list.is_empty() {
                            self.symbols_by_name.remove(&symbol.name);
                        }
                    }
                }
                // Remove references involving this symbol
                self.references.remove(&sym_id);
                for refs in self.references.values_mut() {
                    refs.retain(|(id, _)| *id != sym_id);
                }
            }
        }

        // Remove content
        self.file_content.remove(&file_id);

        Some(entry)
    }

    /// Remove a file by path.
    pub fn remove_file_by_path(&mut self, path: &Path) -> Option<FileEntry> {
        let file_id = self.paths.remove(path)?;
        self.remove_file(file_id)
    }

    /// Update symbols for a file (re-index).
    /// Removes old symbols for the file and adds new ones.
    pub fn update_file_symbols(
        &mut self,
        file_id: FileId,
        new_symbols: Vec<Symbol>,
        new_references: Vec<(SymbolId, SymbolId, Relationship)>,
    ) {
        // Remove old symbols for this file
        if let Some(old_symbol_ids) = self.symbols_by_file.remove(&file_id) {
            for sym_id in old_symbol_ids {
                if let Some(symbol) = self.symbols.remove(&sym_id) {
                    if let Some(name_list) = self.symbols_by_name.get_mut(&symbol.name) {
                        name_list.retain(|id| *id != sym_id);
                        if name_list.is_empty() {
                            self.symbols_by_name.remove(&symbol.name);
                        }
                    }
                }
                // Remove old references for this symbol
                self.references.remove(&sym_id);
                for refs in self.references.values_mut() {
                    refs.retain(|(id, _)| *id != sym_id);
                }
            }
        }

        // Add new symbols
        for symbol in new_symbols {
            let sym_id = self.add_symbol(symbol);
            // Update the symbol's ID reference in calls if needed
            let _ = sym_id; // Symbol already has correct ID set
        }

        // Add new references
        for (from_id, to_id, rel) in new_references {
            self.add_reference(from_id, to_id, rel);
        }
    }

    /// Get all file paths in the index.
    pub fn file_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.paths.keys()
    }

    /// Check if a file path is in the index.
    pub fn has_file(&self, path: &Path) -> bool {
        self.paths.contains_key(path)
    }
}

/// Lexical search result.
#[derive(Debug, Clone)]
pub struct LexicalMatch {
    pub file_id: FileId,
    pub range: Range,
    pub line: String,
    pub line_number: u32,
}

/// Token budget configuration.
#[derive(Debug, Clone, Copy)]
pub struct TokenBudget {
    pub max_tokens: usize,
    pub max_files: usize,
    pub max_symbols: usize,
    pub max_depth: usize,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            max_tokens: 2000,
            max_files: 10,
            max_symbols: 20,
            max_depth: 3,
        }
    }
}

impl TokenBudget {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            max_files: 10,
            max_symbols: 20,
            max_depth: 3,
        }
    }

    pub fn with_files(mut self, max_files: usize) -> Self {
        self.max_files = max_files;
        self
    }

    pub fn with_symbols(mut self, max_symbols: usize) -> Self {
        self.max_symbols = max_symbols;
        self
    }

    pub fn with_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }
}
