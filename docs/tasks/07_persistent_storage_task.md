# Task: Persistent Storage

## Description
Add optional persistent storage for the index to enable fast startup without re-indexing. Currently, the index is rebuilt from scratch every time.

## Requirements
- Serialize index to disk
- Deserialize index on startup
- Handle index format versioning
- Support incremental persistence
- Optional: SQLite-backed storage

## Technical Approach

### Option 1: File-based serialization
- Use serde to serialize index
- Store in `.codesift/` directory
- Compute file hashes for change detection
- Fast startup by loading cached index

### Option 2: SQLite-backed storage
- Use rusqlite for storage
- Store symbols, references, file metadata
- Enable SQL queries on index
- Better for large repositories

## Files to Modify
- `src/index.rs` - Add serialization
- `src/storage.rs` - New storage module
- `src/lib.rs` - Add open_from_cache

## Acceptance Criteria
- [ ] Index is serialized to disk
- [ ] Index loads from cache on startup
- [ ] Cache invalidation works correctly
- [ ] Large repos benefit from caching

## Status
- [ ] Not started
