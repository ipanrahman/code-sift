# Task: Benchmark Suite for Token Efficiency

## Description
Create comprehensive benchmarks to measure token efficiency and performance. The key metric is `useful_context / tokens`, not just `tokens_saved`.

## Requirements
- Benchmark search latency (small/medium/large repos)
- Benchmark indexing time (cold vs incremental)
- Measure memory usage (RSS, index size, peak)
- Compare against ripgrep for representative queries
- Measure token efficiency vs context completeness
- Create fixture repositories for testing

## Technical Approach
1. Create `benches/` directory with benchmark files
2. Use criterion for benchmarking
3. Create test fixtures with varying repo sizes
4. Implement token counting helpers
5. Add integration tests with fixtures

## Benchmarks to Implement
- `search_latency` - Time to search for a symbol
- `index_cold` - Time to index a fresh repository
- `index_incremental` - Time to re-index after a change
- `context_planning` - Time to generate context
- `token_efficiency` - Compare output size vs ripgrep
- `memory_usage` - Measure RSS during indexing

## Files to Create
- `benches/search_latency.rs`
- `benches/indexing.rs`
- `benches/context_planning.rs`
- `fixtures/small/` - Small test repo
- `fixtures/medium/` - Medium test repo

## Acceptance Criteria
- [x] Benchmarks run and report times
- [x] Memory usage is measured
- [x] Token efficiency is measured
- [x] Regression tests detect performance changes

## Status
- [x] Completed
