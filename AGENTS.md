# OpenCode Agents: code-sift

> Token-efficient code intelligence engine for AI agents. This file provides quick-start guidance and common pitfalls to avoid.

## TL;DR

```bash
# Build
cargo build                      # debug
cargo build --release            # release binary in target/release/
make bin                         # build + copy to bin/codesift

# Test
cargo test --lib                 # 14 unit tests
cargo clippy --all-targets     # check for warnings

# MCP server (JSON-RPC 2.0 via stdin)
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | codesift --repo . --mcp

# Search
codesift --repo . "my query"
```

## Build

- `cargo build`           → debug build (no bincode)
- `cargo build --release` → optimized binary at `target/release/codesift`
- `cargo build --features semantic` → enables semantic search + bincode persistence
- `cargo build --no-default-features` → verify bincode is truly optional

**Mandatory**: `cargo build` (without `semantic` feature) must NOT compile bincode. If it does, the feature flag hygiene is broken.

## Test

- `cargo test --lib`          → run all 14 unit tests
- `cargo clippy --all-targets` → check for warnings (6 accepted, structural)
- After any change, always run `cargo test --lib` to confirm nothing is broken

## MCP Server (JSON-RPC 2.0)

Start the server:
```bash
codesift --repo . --mcp
```

Send requests via stdin. Example sequences:

```bash
# Initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"repo":"."}}' | codesift --mcp

# List tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | codesift --mcp

# Find symbol
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"find_symbol","arguments":{"name":"Foo"}}}' | codesift --mcp

# Get context
echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_context","arguments":{"query":"authentication","max_tokens":2000}}}' | codesift --mcp
```

**Key**: Requests with `"id": null` are notifications and must not be answered. The server handles this correctly.

**MCP methods available**: `initialize`, `tools/list`, `tools/call` (search_code, find_symbol, get_context, find_callers, find_callees, get_definition, find_references), `shutdown`, `cancel`, `health`, `session/status`, `workspace/info`

## Codebase Structure

| Directory | Purpose |
|-----------|---------|
| `src/lib.rs` | Main `CodeSift` struct, `open`/`open_cached`/`reindex`, all public APIs |
| `src/main.rs` | CLI entry point, `run_mcp_server`, `run_cli`, `run_watch` |
| `src/mcp.rs` | MCP server adapter, JSON-RPC 2.0, tool definitions & execution |
| `src/semantic.rs` | TF-IDF semantic search, stop words, HybridConfig |
| `src/ranking.rs` | `compute_score`, `RankedCandidate`, `RelevanceScore` |
| `src/context.rs` | `plan_context`, `ContextPlan`, context fragment planning |
| `src/storage.rs` | Persistent cache (`.codesift/`), `Storage::save`/`load`/`load_semantic_index` |
| `src/retrieval.rs` | `hybrid_search`, `RetrievalEngine`, `hybrid_search` modes |
| `src/search.rs` | Lexical search, `SearchQuery`, `SearchMode` |
| `src/index.rs` | `Index`, `ReferenceIndex`, global symbol/relationship graph |
| `src/parser.rs` | Tree-sitter-based parsing for Rust/JS/TS/Python/Go/Java/C/C++ |
| `src/graph.rs` | `ReferenceIndex`, caller/callee traversal |

**Key data types** (see `src/types.rs`): `FileId(u64)`, `SymbolId(u64)`, `Symbol`, `Range`, `SemanticMatch`, `Document`, `SemanticIndex`, `CodeFragment`, `RetrievalResult`, `RankedCandidate`, `RelevanceScore`, `ContextFragment`, `ContextPlan`, `Tool`, `JsonRpcRequest/Response`

## Warnings That Are Acceptable

Clippy produces ~6 warnings that are pre-existing and not actionable:
- `manual_div_ceil` in `src/context.rs`
- `transmute used without annotations` in `src/index.rs`
- `very complex type used` in `src/parser.rs`
- `too many arguments` in `src/parser.rs::walk_node`
- `parameter is only used in recursion` in `src/parser.rs`
- `this impl can be derived` in `src/search.rs` (resolved by adding `#[derive(Default)]` to `SearchMode`)

## Common Pitfalls

1. **Feature flag hygiene**: `bincode` is `optional = true` behind `semantic` feature. Build without `semantic` must NOT compile bincode.

2. **MCP notifications**: Requests without `id` (null) are notifications and must not produce a response. The server handles this.

3. **Semantic index not persisted across startups** without `--features semantic` + cache. Use `make bin` then run with `--mcp` to test.

4. **Stop words are universal**: The `is_stop_word()` function uses only cross-language keywords (`if`, `else`, `for`, `while`, `return`, `true`, `false`, `nil`, `none`). Removed language-specific noise (`fn`, `let`, `mut`, `pub`, etc.).

5. **Ranking boost**: Symbols with `"semantic:"` prefix get a 150x semantic score multiplier vs 100x for regular matches, ensuring semantic matches rank above low-structural lexical matches.

6. **`reindex` preserves semantic index**: After reindex, the semantic index from cache is loaded if available.

7. **`env_logger::init()`** initializes default logger. `RUST_LOG` env var controls log level.

## Helpful Commands

```bash
# Quick MCP test (all tools)
cd /Users/ipanrahman/Workspace/code-sift && \
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"repo":"."}}' | \
  ./target/release/codesift --repo . --mcp && \
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | \
  ./target/release/codesift --repo . --mcp
```

## Further Reading

- `CLAUDE.md` - build/test/lint commands
- `docs/` - architectural docs and task files
- `Cargo.toml` - feature flags and dependencies
- `src/lib.rs` - public API surface
- `src/mcp.rs` - MCP protocol details