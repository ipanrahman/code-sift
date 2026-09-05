# Task: Semantic Search Fixes

## Description
Address issues found in the initial semantic search implementation to make it production-ready before wider use.

## Issues to Fix

### 1. Feature flag hygiene (`Cargo.toml`)
- **Problem**: `bincode` is always compiled even when `semantic` feature is off
- **Fix**: Make `bincode` optional and gate it behind the `semantic` feature
- **Acceptance Criteria**: `cargo build` (without features) does not compile `bincode`

### 2. Dead method (`src/retrieval.rs`)
- **Problem**: `RetrievalEngine::set_semantic_weight()` creates a local `config`, mutates it, then drops it — has no effect
- **Fix**: Either implement proper config mutation (use `Arc<Mutex<HybridConfig>>`) or remove the dead method
- **Acceptance Criteria**: No dead code warnings

### 3. Semantic score discarded in ranking (`src/lib.rs`)
- **Problem**: `plan_context()` wraps semantic matches in fake `Symbol` objects with name `"semantic:..."`, then `rank_candidates()` only knows structural signals — semantic relevance is lost
- **Fix**: Add a semantic-aware ranking path or boost semantic matches before structural ranking
- **Acceptance Criteria**: High-scoring semantic matches appear before low-scoring lexical matches in `plan_context()`

### 4. Semantic index rebuilt on every startup (`src/lib.rs`)
- **Problem**: `open`, `open_cached`, and `reindex` all call `build_index()` from scratch. No persistence for the semantic index.
- **Fix**: Serialize/deserialize `SemanticIndex` alongside the main index in `Storage`, invalidate when source files change
- **Acceptance Criteria**: Semantic index loads from cache without re-tokenizing

### 5. Linear scan performance (`src/semantic.rs`)
- **Problem**: `SemanticIndex::search()` iterates all documents on every query — O(n) per query
- **Fix**: Acceptable for small repos. Add a doc comment noting the limitation and add a TODO for inverted index or vector DB upgrade path
- **Acceptance Criteria**: Documented limitation with upgrade path

### 6. Mixed-language stop words (`src/semantic.rs`)
- **Problem**: Stop word list mixes Rust (`fn`, `let`, `mut`) with Python/Java/JS (`import`, `class`, `def`) keywords
- **Fix**: Either split into language-aware stop lists or keep unified list but remove language-specific noise
- **Acceptance Criteria**: Consistent stop word strategy

## Files to Modify
- `Cargo.toml` - Fix optional dependency
- `src/retrieval.rs` - Fix or remove `set_semantic_weight`
- `src/lib.rs` - Integrate semantic scores into ranking, persist semantic index
- `src/semantic.rs` - Document performance limitation, clean up stop words

## Acceptance Criteria
- [ ] `cargo build` without features does not compile `bincode`
- [ ] No dead code warnings
- [ ] Semantic matches ranked by semantic score in `plan_context`
- [ ] Semantic index persisted and loaded from cache
- [ ] Performance limitation documented with upgrade path
- [ ] Stop word list is consistent

## Status
- [ ] Not started
