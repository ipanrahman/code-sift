# CodeSift

**Token-efficient code intelligence engine for AI coding agents.**

CodeSift retrieves the smallest amount of code context needed for an AI agent to understand or modify a requested part of a codebase.

## Quick Start

```bash
# Install
cargo build --release

# Search
./target/release/codesift --repo . "search query"

# MCP server mode
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/release/codesift --repo . --mcp
```

## Features

- **Token-efficient retrieval** - Returns minimal context within budget
- **Multi-language support** - Rust, JavaScript, TypeScript, Python
- **Symbol-level search** - Find functions, structs, enums by name
- **Call graph tracking** - Traverse callers and callees
- **Lexical search** - Exact, regex, identifier, case-insensitive modes
- **MCP integration** - JSON-RPC 2.0 protocol for AI agents
- **Context planning** - Automatic relevance ranking and deduplication

## Installation

```bash
cargo install --path .
```

## Usage

### CLI

```bash
# Basic search
codesift "my_function"

# With options
codesift --repo /path/to/repo --max-tokens 3000 -- "query"

# Output formats
codesift --format json "search"
codesift --format full "search"

# Verbose (show stats)
codesift -v "search"
```

### MCP Server

Start the MCP server for AI agent integration:

```bash
codesift --repo . --mcp
```

Send JSON-RPC requests via stdin:

```bash
# Initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | codesift --mcp

# List tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | codesift --mcp

# Find symbol
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"find_symbol","arguments":{"name":"Foo"}}}' | codesift --mcp

# Get context
echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_context","arguments":{"query":"authentication","max_tokens":2000}}}' | codesift --mcp
```

## Available Tools

| Tool | Description |
|------|-------------|
| `search_code` | Lexical search across files |
| `find_symbol` | Find symbol by name |
| `get_context` | Get minimal context for query |
| `find_callers` | Find functions calling symbol |
| `find_callees` | Find functions called by symbol |
| `get_definition` | Get symbol definition |
| `find_references` | Find all references to symbol |

## Architecture

CodeSift uses a retrieval pipeline optimized for token efficiency:

```
Query → Candidate Retrieval → Ranking → Context Planning → Output
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for details.

## Supported Languages

| Language | Status |
|----------|--------|
| Rust | ✅ |
| JavaScript | ✅ |
| TypeScript | ✅ |
| Python | ✅ |
| Go | Planned |
| Java | Planned |
| C/C++ | Planned |

## Performance

| Metric | Target |
|--------|--------|
| Index small repo (< 100 files) | < 1s |
| Search latency | < 100ms |
| Memory overhead | ~2-3x source size |

## Development

```bash
# Build
cargo build

# Test
cargo test

# Benchmark
cargo bench

# Release build
cargo build --release
```

## License

MIT
