# Task: MCP Server Integration

## Description
Complete the MCP server implementation and add proper JSON-RPC protocol handling for AI agent integration.

## Requirements
- Implement JSON-RPC 2.0 protocol
- Add proper error handling with error codes
- Support tool cancellation
- Add progress reporting for long operations
- Implement workspace/session management
- Add health check endpoint

## MCP Protocol Implementation

### Required Methods
- `initialize` - Initialize MCP session
- `tools/list` - List available tools
- `tools/call` - Execute a tool
- `shutdown` - Clean shutdown
- `cancel` - Cancel in-progress request

### Tools to Implement
- `search_code` - Lexical search
- `find_symbol` - Symbol lookup by name
- `get_context` - Minimal context generation
- `find_callers` - Call graph traversal (up)
- `find_callees` - Call graph traversal (down)
- `get_definition` - Go to definition
- `find_references` - Find all references

## Technical Approach
1. Create JSON-RPC message types
2. Implement request/response handling
3. Add proper error propagation
4. Implement async tool execution
5. Add request timeout handling

## Files to Modify
- `src/mcp.rs` - Complete MCP implementation
- `src/main.rs` - Add MCP server mode

## Acceptance Criteria
- [x] JSON-RPC 2.0 protocol implemented
- [x] All tools return structured results
- [x] Errors are properly formatted
- [x] Long operations can be cancelled
- [x] Integration tested with Claude Code

## Status
- [x] Completed (2026-09-05)

## Implementation Notes
- Added CancellationToken with atomic operations
- Added Session management with request counting
- Added timeout handling (configurable via initialize params)
- Added progress notification types (ProgressNotification, ProgressParams)
- Added workspace/session info methods: `session/status`, `workspace/info`
- Added meta info to tool results: duration_ms, tokens_used
- New error codes: REQUEST_CANCELLED (-32800), REQUEST_TIMEOUT (-32801)
- Capabilities advertised: tools, progress, cancellation
