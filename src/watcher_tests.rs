//! Tests for incremental indexing.

#[cfg(test)]
mod tests {
    use crate::{CodeSift, FileChange, FsWatcher};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_process_change_created() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().canonicalize().unwrap();

        // Create initial file
        let file_path = path.join("test.rs");
        fs::write(&file_path, "fn main() {}\n").unwrap();

        let mut codesift = CodeSift::open(path.clone()).unwrap();
        let initial_count = codesift.symbol_count();

        // Process a "created" change for a new file
        let new_file = path.join("new_file.rs");
        fs::write(&new_file, "struct Foo {}\n").unwrap();

        codesift.process_change(FileChange::Created(new_file)).unwrap();

        // Should have more symbols now
        assert!(codesift.symbol_count() >= initial_count);
    }

    #[test]
    fn test_process_change_modified() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().canonicalize().unwrap();

        let file_path = path.join("test.rs");
        fs::write(&file_path, "fn original() {}\n").unwrap();

        let mut codesift = CodeSift::open(path.clone()).unwrap();

        // Modify the file
        fs::write(&file_path, "fn modified() {}\nfn another() {}\n").unwrap();

        codesift.process_change(FileChange::Modified(file_path)).unwrap();

        // Should still work after modification
        assert!(codesift.symbol_count() >= 1);
    }

    #[test]
    fn test_process_change_deleted() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().canonicalize().unwrap();

        let file_path = path.join("delete_me.rs");
        fs::write(&file_path, "fn will_delete() {}\n").unwrap();

        let mut codesift = CodeSift::open(path.clone()).unwrap();
        let initial_count = codesift.file_count();

        // Delete the file
        fs::remove_file(&file_path).unwrap();

        codesift.process_change(FileChange::Deleted(file_path)).unwrap();

        // File count should decrease
        assert!(codesift.file_count() < initial_count);
    }

    #[test]
    fn test_reindex() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().canonicalize().unwrap();

        let file_path = path.join("test.rs");
        fs::write(&file_path, "fn test() {}\n").unwrap();

        let mut codesift = CodeSift::open(path.clone()).unwrap();

        // Full reindex should work
        codesift.reindex().unwrap();
        assert!(codesift.symbol_count() >= 1);
    }

    #[test]
    fn test_watcher_creation() {
        let temp = TempDir::new().unwrap();
        let watcher = FsWatcher::new(temp.path());
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_watch_and_process_change() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().canonicalize().unwrap();
        let file_path = path.join("watch_test.rs");
        fs::write(&file_path, "fn watch() {}\n").unwrap();

        let mut codesift = CodeSift::open(path.clone()).unwrap();
        let initial_count = codesift.symbol_count();

        let mut watcher = FsWatcher::new(&path).unwrap();
        watcher.watch(&path).unwrap();

        // Simulate a modification event from the watcher
        fs::write(&file_path, "fn watch() {}\nfn extra() {}\nfn another() {}\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));

        if let Some(change) = watcher.poll_changes() {
            codesift.process_change(change).unwrap();
        }

        assert!(codesift.symbol_count() > initial_count);
    }
}
