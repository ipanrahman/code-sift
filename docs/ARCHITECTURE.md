# CodeSift Architecture

Token-efficient code intelligence engine for AI coding agents.

## Overview

CodeSift retrieves the **smallest amount of code/context** sufficient for an AI agent to understand or modify a requested part of a codebase.

```text
maximize:
    relevance × context_completeness

subject to:
    token_budget
    latency_budget
    memory_budget
```

## Core Pipeline

```
                    ┌─────────────────┐
                    │   Query Input   │
                    └────────┬────────┘
                             │
                             ▼
              ┌─────────────────────────────┐
              │      Query Understanding      │
              └─────────────┬───────────────┘
                            │
                            ▼
        ┌──────────────────────────────────────────┐
        │          Candidate Retrieval               │
        │  ┌────────┐ ┌────────┐ ┌────────┐      │
        │  │ lexical│ │ symbol │ │ graph  │      │
        │  └────────┘ └────────┘ └────────┘      │
        └───────────────────┬──────────────────────┘
                            │
                            ▼
                  ┌──────────────────┐
                  │      Ranking       │
                  └────────┬─────────┘
                           │
                           ▼
                ┌─────────────────────┐
                │   Context Planner    │
                └──────────┬───────────┘
                           │
                           ▼
                 ┌──────────────────┐
                 │  Token Budget     │
                 └────────┬─────────┘
                          │
                          ▼
                 ┌──────────────────┐
                 │ Minimal Context   │
                 └──────────────────┘
```

## Module Architecture

### `types.rs` - Core Types

```rust
FileId(u64)          // Unique file identifier
SymbolId(u64)        // Unique symbol identifier
Range                // Byte/line ranges in source
Language             // Programming language enum
Symbol               // Function, struct, enum, etc.
SymbolKind           // Category of symbol
Relationship         // Reference, call, import, etc.
```

### `repository.rs` - Repository Scanner

- Recursive file discovery with `ignore` crate
- `.gitignore` support
- Binary/generated/vendor detection
- Language detection by extension

```rust
Repository::open(path) -> Result<Self>
Repository::files() -> Iterator<Item = &FileEntry>
Repository::read_file(FileId) -> Result<Vec<u8>>
```

### `index.rs` - In-Memory Index

HashMap-backed storage for:
- Files by ID and path
- Symbols by ID, name, and file
- References between symbols
- File content cache

```rust
Index::add_file(FileEntry) -> FileId
Index::add_symbol(Symbol) -> SymbolId
Index::find_symbols_by_name(&str) -> Vec<SymbolId>
Index::symbols_in_file(FileId) -> Vec<&Symbol>
```

### `parser.rs` - Tree-sitter Parser

Extracts symbols from source using Tree-sitter:
- Rust, JavaScript/TypeScript, Python
- Symbol extraction (functions, structs, etc.)
- Call reference detection

```rust
Parser::parse(source: &[u8], lang: Language, file_id: FileId) -> Result<ParsedFile>
ParsedFile { symbols, references, calls }
```

### `search.rs` - Lexical Search

Multi-mode text search:
- `Exact` - String contains
- `Regex` - Pattern match
- `Identifier` - Word boundary
- `CaseInsensitive` - Case-insensitive

```rust
search(&SearchQuery, &Index, &TokenBudget) -> Result<Vec<LexicalMatch>>
```

### `graph.rs` - Call Graph

Tracks symbol relationships:
- Outgoing/incoming references
- Call graph (caller → callee)
- Bounded graph traversal

```rust
ReferenceIndex::add_reference(from, to, Relationship)
ReferenceIndex::get_callers(SymbolId) -> &[SymbolId]
ReferenceIndex::traverse_callers(SymbolId, depth) -> Vec<(SymbolId, depth)>
```

### `ranking.rs` - Relevance Scoring

Scores candidates based on structural signals:

| Signal | Score |
|--------|-------|
| Exact symbol match | +100 |
| Definition | +80 |
| Direct caller/callee | +60 |
| Test relationship | +50 |
| Reference | +40 |
| Lexical match | +20 |

### `context.rs` - Context Planner

Selects and formats minimal context:
- Token budget enforcement
- Duplicate elimination
- Range merging
- Structural boundary preservation

```rust
plan_context(candidates, &TokenBudget, get_source) -> ContextPlan
ContextPlan { fragments, total_tokens, total_files }
```

### `mcp.rs` - MCP Adapter

JSON-RPC 2.0 server for AI agent integration:

**Methods:**
- `initialize` - Protocol handshake
- `tools/list` - List available tools
- `tools/call` - Execute tool
- `shutdown` - Clean shutdown
- `health` - Health check

**Tools:**
- `search_code` - Lexical search
- `find_symbol` - Symbol lookup
- `get_context` - Minimal context
- `find_callers` / `find_callees` - Graph traversal
- `get_definition` - Go to definition
- `find_references` - Find all references

## Data Flow

```
1. Open Repository
   └── Repository::open(path)
       └── WalkBuilder scans directory
           └── FileEntry created for each source file

2. Index Repository
   └── For each file:
       ├── Read content into cache
       └── Parser::parse() extracts symbols

3. Query Execution
   └── CodeSift::plan_context(query, budget)
       ├── find_symbol(query) - Symbol matches
       ├── search(query) - Lexical matches
       ├── rank_candidates() - Score by relevance
       └── plan_context() - Select minimal context

4. Output
   └── ContextPlan with fragments
       └── Each fragment: file, range, symbol, content
```

## Token Budget

First-class constraint throughout:

```rust
struct TokenBudget {
    max_tokens: usize,    // Max tokens to return
    max_files: usize,     // Max files to include
    max_symbols: usize,   // Max symbols
    max_depth: usize,     // Max graph traversal depth
}
```

**Token Estimation:** ~4 characters per token (approximation).

## CLI Usage

```bash
# Basic search
codesift --repo . "search query"

# With options
codesift --repo . --max-tokens 5000 --format json "query"

# MCP server mode
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"find_symbol","arguments":{"name":"Foo"}}}' | codesift --repo . --mcp
```

## File Structure

```
codesift/
├── src/
│   ├── lib.rs          # Main engine API
│   ├── main.rs         # CLI entry point
│   ├── types.rs        # Core types
│   ├── repository.rs   # File discovery
│   ├── index.rs        # In-memory index
│   ├── parser.rs       # Tree-sitter parsing
│   ├── search.rs       # Lexical search
│   ├── graph.rs        # Call graph
│   ├── ranking.rs      # Relevance scoring
│   ├── context.rs      # Context planning
│   ├── mcp.rs         # MCP adapter
│   └── error.rs       # Error types
├── benches/
│   └── search.rs       # Benchmarks
├── docs/
│   └── tasks/          # Task tracking
├── Cargo.toml
└── README.md
```

## Performance Characteristics

| Operation | Expected Performance |
|----------|---------------------|
| Index small repo (< 100 files) | < 1s |
| Index medium repo (100-1000 files) | 1-10s |
| Search latency | < 100ms |
| Context planning | < 50ms |

**Memory:** Proportional to source size + index overhead (~2-3x source size).

## Future Enhancements

1. **Incremental Indexing** - Watch mode for live updates
2. **Reference Resolution** - Link call sites to definitions
3. **Persistent Storage** - Serialize index to disk
4. **Additional Languages** - Go, Java, C/C++
5. **Semantic Search** - Embedding-based retrieval (future)

## Design Principles

1. **Token efficiency first** - Never dump entire files
2. **Symbol-level retrieval** - Prefer functions over files
3. **Structural relationships** - Graph over similarity
4. **Deterministic behavior** - No randomness
5. **Local-first** - No cloud dependencies
6. **Idiomatic Rust** - Ownership, enums, pattern matching
