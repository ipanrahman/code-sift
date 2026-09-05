//! Repository scanning and file discovery.

use crate::error::{Error, Result};
use crate::types::{FileEntry, FileId, Language};
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Represents a scanned repository.
pub struct Repository {
    root: PathBuf,
    files: HashMap<FileId, FileEntry>,
    paths: HashMap<PathBuf, FileId>,
}

impl Repository {
    /// Open and scan a repository at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path
            .as_ref()
            .canonicalize()
            .map_err(|e| Error::Repository(format!("failed to canonicalize path: {}", e)))?;

        if !root.is_dir() {
            return Err(Error::Repository("path is not a directory".into()));
        }

        let files = Self::scan_directory(&root)?;

        let mut paths: HashMap<PathBuf, FileId> = HashMap::new();
        for (id, entry) in &files {
            paths.insert(entry.path.clone(), *id);
        }

        Ok(Self { root, files, paths })
    }

    /// Scan directory for source files.
    fn scan_directory(root: &Path) -> Result<HashMap<FileId, FileEntry>> {
        let mut files = HashMap::new();
        let mut file_id_counter = 0u64;

        let excluded_dirs = [
            "target",
            "node_modules",
            "dist",
            "build",
            ".git",
            ".svn",
            "__pycache__",
            ".next",
            ".nuxt",
            ".cache",
            ".parcel-cache",
            "coverage",
            ".turbo",
        ];

        let walker = WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();

            // Skip excluded directories at the top level
            if path.parent() == Some(root) {
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if excluded_dirs.iter().any(|d| name_str == *d) {
                        continue;
                    }
                }
            }

            if !path.is_file() {
                continue;
            }

            // Skip binary files and files without extensions
            let extension = path.extension().and_then(|e| e.to_str());
            let Some(ext) = extension else {
                continue;
            };

            let language = Language::from_extension(ext);
            if matches!(language, Language::Unknown) {
                continue;
            }

            let Some(mut file_entry) = FileEntry::from_path(path.to_path_buf()) else {
                continue;
            };

            // Mark vendor files
            if path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().contains("vendor"))
            {
                file_entry.is_vendor = true;
            }

            // Mark generated files
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.ends_with(".gen.rs")
                || file_name.ends_with(".pb.rs")
                || file_name.contains(".generated.")
            {
                file_entry.is_generated = true;
            }

            file_entry.id = FileId::new(file_id_counter);
            files.insert(file_entry.id, file_entry);
            file_id_counter += 1;
        }

        Ok(files)
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

    /// Get file count.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Get root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Read file content.
    pub fn read_file(&self, id: FileId) -> Result<Vec<u8>> {
        let entry = self
            .files
            .get(&id)
            .ok_or_else(|| Error::FileNotFound(format!("{:?}", id)))?;

        std::fs::read(&entry.path).map_err(|e| Error::Io(e))
    }

    /// Get relative path from root.
    pub fn relative_path(&self, id: FileId) -> Option<PathBuf> {
        let entry = self.files.get(&id)?;
        entry
            .path
            .strip_prefix(&self.root)
            .ok()
            .map(|p| p.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("JS"), Language::JavaScript);
        assert_eq!(Language::from_extension("xyz"), Language::Unknown);
    }
}
