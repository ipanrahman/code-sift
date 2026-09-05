//! Filesystem watcher for incremental indexing.

use crate::error::{Error, Result};
use hashbrown::HashSet;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// File change event types.
#[derive(Debug, Clone)]
pub enum FileChange {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

/// Filesystem watcher for detecting file changes.
pub struct FsWatcher {
    watcher: RecommendedWatcher,
    receiver: Receiver<Result<Event>>,
    watched_paths: HashSet<PathBuf>,
}

impl FsWatcher {
    pub fn new(_path: impl AsRef<Path>) -> Result<Self> {
        let (tx, rx) = channel();

        let watcher = RecommendedWatcher::new(
            move |res: std::result::Result<Event, notify::Error>| {
                let _ = tx.send(res.map_err(|e| Error::Io(std::io::Error::other(e.to_string()))));
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )
        .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;

        Ok(Self {
            watcher,
            receiver: rx,
            watched_paths: HashSet::new(),
        })
    }

    pub fn watch(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        self.watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
        self.watched_paths.insert(path);
        Ok(())
    }

    pub fn poll_changes(&self) -> Option<FileChange> {
        self.receiver.try_recv().ok().and_then(|res| {
            res.ok().and_then(|event| {
                let path = event.paths.first()?.clone();
                match event.kind {
                    notify::EventKind::Create(_) => Some(FileChange::Created(path)),
                    notify::EventKind::Modify(_) => Some(FileChange::Modified(path)),
                    notify::EventKind::Remove(_) => Some(FileChange::Deleted(path)),
                    _ => None,
                }
            })
        })
    }

    pub fn iter(&self) -> WatcherIter<'_> {
        WatcherIter { watcher: self }
    }
}

pub struct WatcherIter<'a> {
    watcher: &'a FsWatcher,
}

impl<'a> Iterator for WatcherIter<'a> {
    type Item = FileChange;

    fn next(&mut self) -> Option<Self::Item> {
        self.watcher.poll_changes()
    }
}

/// File metadata cache for change detection.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub content_hash: u64,
    pub modified: std::time::SystemTime,
}

impl FileMetadata {
    pub fn compute_hash(content: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    pub fn from_content(path: PathBuf, content: &[u8], modified: std::time::SystemTime) -> Self {
        Self {
            path,
            content_hash: Self::compute_hash(content),
            modified,
        }
    }
}

/// Change detection state for incremental indexing.
#[derive(Default)]
pub struct ChangeDetector {
    metadata: HashSet<(PathBuf, u64)>,
}

impl ChangeDetector {
    pub fn new() -> Self {
        Self {
            metadata: HashSet::new(),
        }
    }

    pub fn has_changed(&self, path: &Path, content: &[u8]) -> bool {
        let hash = FileMetadata::compute_hash(content);
        !self.metadata.contains(&(path.to_path_buf(), hash))
    }

    pub fn update(&mut self, path: PathBuf, content: &[u8]) {
        let hash = FileMetadata::compute_hash(content);
        self.metadata.insert((path, hash));
    }

    pub fn remove(&mut self, path: &Path) {
        self.metadata.retain(|(p, _)| p != path);
    }

    pub fn len(&self) -> usize {
        self.metadata.len()
    }

    pub fn is_empty(&self) -> bool {
        self.metadata.is_empty()
    }
}
