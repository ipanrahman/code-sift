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
- [ ] JSON-RPC 2.0 protocol implemented
- [ ] All tools return structured results
- [ ] Errors are properly formatted
- [ ] Long operations can be cancelled
- [ ] Integration tested with Claude Code

## Status
- [x] Completed (2024-09-05)
