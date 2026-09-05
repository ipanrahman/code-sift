# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build
cargo build [--release]

# Test (unit tests only)
cargo test --lib

# Run single test
cargo test --lib test_name

# Benchmarks
cargo bench

# Format
cargo fmt
```

## Architecture

CodeSift is a token-efficient code intelligence engine. Core pipeline:

```
Query → Candidate Retrieval → Ranking → Context Planning → Output
```

Key modules:
- `src/lib.rs` - Main `CodeSift` struct, entry points for search and context planning
- `src/parser.rs` - Tree-sitter parsing, extracts `ParsedFile { symbols, references, calls }`
- `src/graph.rs` - `ReferenceIndex` for call graph traversal (callers/callees)
- `src/context.rs` - `plan_context()` selects minimal context within token budget
- `src/mcp.rs` - JSON-RPC 2.0 adapter, exposes tools via stdin/stdout

Token budget is first-class: `TokenBudget { max_tokens, max_files, max_symbols, max_depth }`.

## MCP Integration

The MCP server mode reads JSON-RPC from stdin:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"find_symbol","arguments":{"name":"Foo"}}}' | codesift --repo . --mcp
```

## Adding Languages

To add a new language:
1. Add `tree-sitter-{lang}` to `Cargo.toml`
2. Update `parser.rs::parse()` match to handle the language
3. Add node kinds in `node_to_symbol()` matching tree-sitter grammar

## Key Types

```rust
FileId(u64), SymbolId(u64)   // Newtype IDs
Range { start_byte, end_byte, start_line, end_line }
Symbol { id, name, kind, file_id, range, parent }
ParsedFile { symbols: Vec<Symbol>, references, calls: Vec<CallReference> }
ContextPlan { fragments, total_tokens, total_files }
```
