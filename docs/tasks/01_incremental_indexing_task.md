# Task: Incremental Indexing

## Description
Implement filesystem change detection to enable incremental re-indexing of repositories. Currently, CodeSift rebuilds the entire index on every open. Incremental indexing will only process changed files.

## Requirements
- Detect file additions, modifications, and deletions
- Use filesystem watchers (notify crate or equivalent)
- Only re-parse affected files
- Update symbol and reference indices incrementally
- Support watch mode for development workflows

## Technical Approach
1. Add `notify` crate dependency for filesystem watching
2. Create a `Watcher` struct that tracks file changes
3. Implement `Index::update_file()` for incremental updates
4. Add `CodeSift::watch()` method to start watching
5. Add `CodeSift::reindex()` to trigger manual re-index

## Files to Modify
- `Cargo.toml` - Add notify dependency
- `src/lib.rs` - Add watch/reindex methods
- `src/index.rs` - Add update_file method
- `src/repository.rs` - Add change detection

## Acceptance Criteria
- [x] Only changed files are re-parsed on modification
- [x] Deleted files are removed from index
- [x] Added files are indexed
- [x] Watch mode detects changes in real-time
- [x] Index size remains bounded over time

## Status
- [x] Completed
