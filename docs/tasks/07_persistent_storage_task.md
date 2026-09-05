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
- [x] Index is serialized to disk
- [x] Index loads from cache on startup
- [x] Cache invalidation works correctly
- [x] Large repos benefit from caching

## Status
- [x] Completed (2026-09-05)

## Implementation Notes
- Added `src/storage.rs` with `Storage` struct for cache management
- Cache stored in `.codesift/` directory with:
  - `manifest.json` - metadata (version, file/symbol counts)
  - `index.bin` - serialized index data (files, symbols, references)
  - `hashes.bin` - file hashes for invalidation
- Added `--use-cache` flag to use cached index
- Cache invalidation uses file modification time + size hash
- Added methods to Index: `insert_file`, `insert_path`, `insert_symbol`, `set_next_file_id`, `set_next_symbol_id`
- `open_cached()` tries cache first, falls back to full index if invalid
