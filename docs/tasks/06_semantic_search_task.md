# Task: Semantic Search (Future)

## Description
Add optional semantic/embedding-based search for natural language queries. This is NOT for MVP but for future enhancement when structural + lexical retrieval proves insufficient.

## Requirements
- Optional embedding generation for files
- Vector similarity search
- Hybrid search (lexical + semantic)
- LLM-based query reformulation (optional)
- Configurable semantic vs structural weighting

## When to Implement
This should only be added after:
1. Benchmarking shows structural + lexical retrieval cannot solve relevant queries
2. Token efficiency measurements are in place
3. Core functionality is proven in production

## Technical Approach

### Phase 1: Embedding Generation
- Add `embeddings` feature flag
- Use lightweight embeddings (e.g., code-transformer-tiny)
- Store vectors in index
- Add similarity search to retrieval engine

### Phase 2: Hybrid Search
- Combine BM25 with vector similarity
- Learn optimal weighting from feedback
- Cache embeddings for unchanged files

### Phase 3: Advanced Features
- Query expansion using LLM
- Reranking using cross-encoder
- Semantic code summarization

## Files to Create/Modify
- `src/semantic.rs` - Embedding and similarity
- `src/retrieval.rs` - Hybrid retrieval
- `Cargo.toml` - Add embedding dependencies

## Metrics to Track
- Precision at k
- Recall at k
- Latency (embedding + search)
- Memory usage (vector storage)
- Token savings vs pure lexical

## Status
- [ ] Not started (Future)
