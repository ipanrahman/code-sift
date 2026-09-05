//! Persistent storage for CodeSift index.
//!
//! Stores index to disk for fast startup without re-indexing.

use crate::index::Index;
use crate::types::{FileEntry, FileId, Symbol};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// Current cache version for format compatibility.
const CACHE_VERSION: u32 = 1;

/// Cache directory name.
const CACHE_DIR: &str = ".codesift";

/// Cache file names.
const INDEX_FILE: &str = "index.bin";
const MANIFEST_FILE: &str = "manifest.json";
const HASHES_FILE: &str = "hashes.bin";
#[cfg(feature = "semantic")]
const SEMANTIC_INDEX_FILE: &str = "semantic.bin";

/// Cache manifest containing metadata.
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    repo_path: String,
    file_count: usize,
    symbol_count: usize,
    created_at: u64,
}

/// File hash entry for cache invalidation.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct FileHash {
    path: String,
    hash: u64,
    modified_at: u64,
}

/// Persistent storage manager.
pub struct Storage {
    cache_dir: PathBuf,
}

impl Storage {
    /// Create storage manager for a repository.
    pub fn new(repo_path: &Path) -> Self {
        let cache_dir = repo_path.join(CACHE_DIR);
        Self { cache_dir }
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Check if a valid cache exists.
    pub fn cache_exists(&self) -> bool {
        self.cache_dir.join(MANIFEST_FILE).exists()
    }

    /// Save the index to disk.
    pub fn save(&self, index: &Index, repo_path: &Path) -> std::io::Result<()> {
        // Create cache directory
        fs::create_dir_all(&self.cache_dir)?;

        let file_count = index.file_count();
        let symbol_count = index.symbol_count();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        #[cfg(feature = "semantic")]
        let has_semantic_index = {
            let semantic_bytes = crate::retrieval::RetrievalEngine::default()
                .get_semantic_index()
                .serialize();
            fs::write(self.cache_dir.join(SEMANTIC_INDEX_FILE), &semantic_bytes)?;
            !semantic_bytes.is_empty()
        };

        #[cfg(not(feature = "semantic"))]
        let _has_semantic_index = false;

        // Save manifest
        let manifest = Manifest {
            version: CACHE_VERSION,
            repo_path: repo_path.to_string_lossy().to_string(),
            file_count,
            symbol_count,
            created_at: now,
        };
        let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
        fs::write(self.cache_dir.join(MANIFEST_FILE), manifest_json)?;

        // Save file hashes for invalidation
        let hashes: Vec<FileHash> = index
            .files()
            .map(|f| FileHash {
                path: f.path.to_string_lossy().to_string(),
                hash: hash_file(&f.path),
                modified_at: f.modified_at,
            })
            .collect();
        let hash_bytes = serde_json::to_vec(&hashes).unwrap();
        fs::write(self.cache_dir.join(HASHES_FILE), hash_bytes)?;

        // Save index data
        let index_data = SerializableIndexData {
            files: index.files().cloned().collect(),
            file_paths: index.files().map(|f| (f.path.to_string_lossy().to_string(), f.id.0)).collect(),
            symbols: index.symbols().cloned().collect(),
            references: index.all_references().iter()
                .map(|(from, to, rel)| (from.0, to.0, *rel as u8))
                .collect(),
            next_file_id: index.next_file_id(),
            next_symbol_id: index.next_symbol_id(),
        };

        let file = fs::File::create(self.cache_dir.join(INDEX_FILE))?;
        let mut writer = BufWriter::new(file);
        serialize_data(&mut writer, &index_data)?;

        Ok(())
    }

    /// Load the index from disk.
    pub fn load(&self) -> std::io::Result<(Index, crate::graph::ReferenceIndex)> {
        let manifest: Manifest = serde_json::from_str(
            &fs::read_to_string(self.cache_dir.join(MANIFEST_FILE))?,
        )?;

        if manifest.version != CACHE_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Cache version mismatch: expected {}, got {}", CACHE_VERSION, manifest.version),
            ));
        }

        let file = fs::File::open(self.cache_dir.join(INDEX_FILE))?;
        let mut reader = BufReader::new(file);
        let index_data: SerializableIndexData = deserialize_data(&mut reader)?;

        // Rebuild index
        let mut index = Index::new();

        for file_entry in &index_data.files {
            index.insert_file(file_entry.clone());
        }
        for (path, id) in &index_data.file_paths {
            index.insert_path(PathBuf::from(path), FileId(*id));
        }

        // Add symbols directly (preserve their IDs)
        for symbol in &index_data.symbols {
            index.insert_symbol(symbol.clone());
        }

        // Set the next ID counters
        index.set_next_file_id(index_data.next_file_id);
        index.set_next_symbol_id(index_data.next_symbol_id);

        index.add_references_from_serializable(index_data.references);

        // Rebuild reference index from references
        let mut refs = crate::graph::ReferenceIndex::new();
        for (from, to, rel) in index.all_references() {
            refs.add_reference(from, to, rel);
        }

        Ok((index, refs))
    }

    /// Load cached semantic index data when the semantic feature is enabled.
    #[cfg(feature = "semantic")]
    pub fn load_semantic_index(&self) -> std::io::Result<Option<crate::semantic::SemanticIndex>> {
        let path = self.cache_dir.join(SEMANTIC_INDEX_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(path)?;
        Ok(crate::semantic::SemanticIndex::deserialize(&bytes))
    }

    /// Check whether a cached semantic index exists.
    #[cfg(feature = "semantic")]
    pub fn has_semantic_index(&self) -> bool {
        self.cache_dir.join(SEMANTIC_INDEX_FILE).exists()
    }

    /// Get cached file hashes for invalidation checking.
    pub fn get_file_hashes(&self) -> std::io::Result<HashMap<String, (u64, u64)>> {
        let hashes: Vec<FileHash> = serde_json::from_str(
            &fs::read_to_string(self.cache_dir.join(HASHES_FILE))?,
        ).unwrap_or_default();

        Ok(hashes
            .into_iter()
            .map(|h| (h.path, (h.hash, h.modified_at)))
            .collect())
    }

    /// Delete the cache.
    pub fn delete(&self) -> std::io::Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }
}

/// Internal index data structure for serialization.
#[derive(Debug, Serialize, Deserialize)]
struct SerializableIndexData {
    files: Vec<FileEntry>,
    file_paths: Vec<(String, u64)>,
    symbols: Vec<Symbol>,
    references: Vec<(u64, u64, u8)>,
    next_file_id: u64,
    next_symbol_id: u64,
}

/// Simple binary serialization helpers.
fn serialize_data(writer: &mut impl Write, data: &SerializableIndexData) -> std::io::Result<()> {
    let json = serde_json::to_string(data).unwrap();
    let len = json.len() as u64;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(json.as_bytes())?;
    writer.flush()
}

fn deserialize_data(reader: &mut impl Read) -> std::io::Result<SerializableIndexData> {
    let mut len_bytes = [0u8; 8];
    reader.read_exact(&mut len_bytes)?;
    let len = u64::from_le_bytes(len_bytes) as usize;

    let mut json_bytes = vec![0u8; len];
    reader.read_exact(&mut json_bytes)?;
    let json = String::from_utf8(json_bytes).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;

    serde_json::from_str(&json).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })
}

/// Simple file hash using modification time and size (fast).
pub fn hash_file(path: &Path) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return 0,
    };

    let mut hasher = DefaultHasher::new();
    metadata.len().hash(&mut hasher);
    if let Ok(modified) = metadata.modified() {
        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
            duration.as_secs().hash(&mut hasher);
        }
    }

    hasher.finish()
}

/// Check if any cached files have changed.
pub fn files_changed(
    cached: &HashMap<String, (u64, u64)>,
    current: &HashMap<String, (u64, u64)>,
) -> bool {
    for (path, (cached_hash, cached_mtime)) in cached {
        if let Some((current_hash, current_mtime)) = current.get(path) {
            if cached_hash != current_hash || cached_mtime != current_mtime {
                return true;
            }
        } else {
            // File was deleted
            return true;
        }
    }

    // Check for new files
    if cached.len() != current.len() {
        return true;
    }

    false
}
