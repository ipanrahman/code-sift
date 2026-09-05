//! MCP server adapter for CodeSift.
//!
//! Provides JSON-RPC 2.0 protocol for AI coding agents to query code context.

use crate::{CodeSift, ContextPlan, TokenBudget};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// MCP error codes.
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
}

/// MCP tool definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// MCP tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

/// MCP server state.
pub struct McpServer {
    codesift: CodeSift,
    initialized: bool,
}

impl McpServer {
    pub fn new(codesift: CodeSift) -> Self {
        Self {
            codesift,
            initialized: false,
        }
    }

    /// Process a JSON-RPC request and return response.
    pub fn handle(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id;

        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: error_codes::INVALID_REQUEST,
                    message: "Invalid JSON-RPC version".into(),
                    data: None,
                }),
            };
        }

        // Handle methods
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(request.params),
            "shutdown" => self.handle_shutdown(),
            "cancel" => self.handle_cancel(request.params),
            "health" => self.handle_health(),
            _ => Err((error_codes::METHOD_NOT_FOUND, format!("Unknown method: {}", request.method))),
        };

        match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: Some(value),
                error: None,
            },
            Err((code, msg)) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code,
                    message: msg,
                    data: None,
                }),
            },
        }
    }

    fn handle_initialize(&mut self, _params: Option<Value>) -> Result<Value, (i32, String)> {
        self.initialized = true;
        Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "codesift",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {}
            }
        }))
    }

    fn handle_tools_list(&self) -> Result<Value, (i32, String)> {
        let tools = self.list_tools();
        Ok(serde_json::json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, params: Option<Value>) -> Result<Value, (i32, String)> {
        let params = params.ok_or((error_codes::INVALID_PARAMS, "Missing params".into()))?;

        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or((error_codes::INVALID_PARAMS, "Missing 'name' param".into()))?;

        let arguments = params.get("arguments")
            .and_then(|v| v.as_object())
            .map(|m| serde_json::Value::Object(m.clone()))
            .unwrap_or(serde_json::Value::Null);

        let result = self.execute_tool(name, &arguments);

        Ok(serde_json::json!({
            "content": result.content,
            "isError": result.is_error.unwrap_or(false)
        }))
    }

    fn handle_shutdown(&self) -> Result<Value, (i32, String)> {
        Ok(serde_json::json!({ "shutdown": true }))
    }

    fn handle_cancel(&self, _params: Option<Value>) -> Result<Value, (i32, String)> {
        // Cancellation not yet implemented
        Ok(serde_json::json!({ "cancelled": true }))
    }

    fn handle_health(&self) -> Result<Value, (i32, String)> {
        Ok(serde_json::json!({
            "status": "ok",
            "files": self.codesift.file_count(),
            "symbols": self.codesift.symbol_count()
        }))
    }

    /// List available tools.
    pub fn list_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "search_code".into(),
                description: "Search for text in code files".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Search pattern"},
                        "max_tokens": {"type": "integer", "default": 2000}
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: "find_symbol".into(),
                description: "Find a symbol (function, struct, etc.) by name".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "Symbol name"},
                        "max_tokens": {"type": "integer", "default": 2000}
                    },
                    "required": ["name"]
                }),
            },
            Tool {
                name: "get_context".into(),
                description: "Get minimal context for a query with token budget".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Query string"},
                        "max_tokens": {"type": "integer", "default": 2000},
                        "max_files": {"type": "integer", "default": 10}
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: "find_callers".into(),
                description: "Find functions that call a given symbol".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "symbol": {"type": "string", "description": "Symbol name"},
                        "depth": {"type": "integer", "default": 2}
                    },
                    "required": ["symbol"]
                }),
            },
            Tool {
                name: "find_callees".into(),
                description: "Find functions called by a given symbol".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "symbol": {"type": "string", "description": "Symbol name"},
                        "depth": {"type": "integer", "default": 2}
                    },
                    "required": ["symbol"]
                }),
            },
            Tool {
                name: "get_definition".into(),
                description: "Get the definition of a symbol".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "Symbol name"}
                    },
                    "required": ["name"]
                }),
            },
            Tool {
                name: "find_references".into(),
                description: "Find all references to a symbol".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "Symbol name"}
                    },
                    "required": ["name"]
                }),
            },
        ]
    }

    /// Execute a tool.
    pub fn execute_tool(&self, name: &str, args: &Value) -> ToolResult {
        match name {
            "search_code" => self.tool_search_code(args),
            "find_symbol" => self.tool_find_symbol(args),
            "get_context" => self.tool_get_context(args),
            "find_callers" => self.tool_find_callers(args),
            "find_callees" => self.tool_find_callees(args),
            "get_definition" => self.tool_get_definition(args),
            "find_references" => self.tool_find_references(args),
            _ => ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("Unknown tool: {}", name),
                }],
                is_error: Some(true),
            },
        }
    }

    fn tool_search_code(&self, args: &Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return self.error("Missing 'query' argument"),
        };
        let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

        let budget = TokenBudget::new(max_tokens);

        match self.codesift.search(query, Some(budget)) {
            Ok(matches) => {
                let file_count = matches.iter().map(|m| m.file_id).collect::<std::collections::HashSet<_>>().len();
                let summary = format!("Found {} matches in {} files", matches.len(), file_count);
                ToolResult {
                    content: vec![ContentBlock::Text { text: summary }],
                    is_error: None,
                }
            }
            Err(e) => self.error(&e.to_string()),
        }
    }

    fn tool_find_symbol(&self, args: &Value) -> ToolResult {
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return self.error("Missing 'name' argument"),
        };

        let symbols = self.codesift.find_symbol(name);

        if symbols.is_empty() {
            return ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("Symbol '{}' not found", name),
                }],
                is_error: Some(true),
            };
        }

        let mut output = format!("Found {} symbol(s):\n", symbols.len());
        for sym in &symbols {
            if let Some(file) = self.codesift.get_file(sym.file_id) {
                output.push_str(&format!(
                    "- {} ({:?}) at {}:{}:{}\n",
                    sym.name,
                    sym.kind,
                    file.path.display(),
                    sym.range.start_line,
                    sym.range.end_line
                ));
            }
        }

        ToolResult {
            content: vec![ContentBlock::Text { text: output }],
            is_error: None,
        }
    }

    fn tool_get_context(&self, args: &Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return self.error("Missing 'query' argument"),
        };
        let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;
        let max_files = args.get("max_files").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let budget = TokenBudget::new(max_tokens).with_files(max_files);

        match self.codesift.plan_context(query, Some(budget)) {
            Ok(plan) => {
                let output = Self::format_context_plan(&self.codesift, &plan);
                ToolResult {
                    content: vec![ContentBlock::Text { text: output }],
                    is_error: None,
                }
            }
            Err(e) => self.error(&e.to_string()),
        }
    }

    fn tool_find_callers(&self, args: &Value) -> ToolResult {
        let symbol = match args.get("symbol").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.error("Missing 'symbol' argument"),
        };
        let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;

        let symbols = self.codesift.find_symbol(symbol);
        if symbols.is_empty() {
            return ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("Symbol '{}' not found", symbol),
                }],
                is_error: Some(true),
            };
        }

        let mut output = format!("Callers of '{}':\n", symbol);
        for sym in &symbols {
            let callers = self.codesift.get_callers_upto(sym.id, depth);
            if callers.is_empty() {
                output.push_str("  (no callers found)\n");
            } else {
                for (caller_id, d) in callers {
                    if let Some(caller) = self.codesift.get_symbol(caller_id) {
                        output.push_str(&format!("{} depth {}: {}\n", "  ".repeat(d), d, caller.name));
                    }
                }
            }
        }

        ToolResult {
            content: vec![ContentBlock::Text { text: output }],
            is_error: None,
        }
    }

    fn tool_find_callees(&self, args: &Value) -> ToolResult {
        let symbol = match args.get("symbol").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.error("Missing 'symbol' argument"),
        };
        let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;

        let symbols = self.codesift.find_symbol(symbol);
        if symbols.is_empty() {
            return ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("Symbol '{}' not found", symbol),
                }],
                is_error: Some(true),
            };
        }

        let mut output = format!("Callees of '{}':\n", symbol);
        for sym in &symbols {
            let callees = self.codesift.get_callees_upto(sym.id, depth);
            if callees.is_empty() {
                output.push_str("  (no callees found)\n");
            } else {
                for (callee_id, d) in callees {
                    if let Some(callee) = self.codesift.get_symbol(callee_id) {
                        output.push_str(&format!("{} depth {}: {}\n", "  ".repeat(d), d, callee.name));
                    }
                }
            }
        }

        ToolResult {
            content: vec![ContentBlock::Text { text: output }],
            is_error: None,
        }
    }

    fn tool_get_definition(&self, args: &Value) -> ToolResult {
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return self.error("Missing 'name' argument"),
        };

        let symbols = self.codesift.find_symbol(name);

        if symbols.is_empty() {
            return ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("Symbol '{}' not found", name),
                }],
                is_error: Some(true),
            };
        }

        // Return first definition
        let sym = &symbols[0];
        if let Some(file) = self.codesift.get_file(sym.file_id) {
            let source = self.codesift.get_source(sym.file_id, &sym.range);
            let content = source.unwrap_or_default();
            ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("{}:{}:{}\n\n{}", file.path.display(), sym.range.start_line, sym.range.end_line, content),
                }],
                is_error: None,
            }
        } else {
            self.error("Failed to get file")
        }
    }

    fn tool_find_references(&self, args: &Value) -> ToolResult {
        let name = match args.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return self.error("Missing 'name' argument"),
        };

        let symbols = self.codesift.find_symbol(name);

        if symbols.is_empty() {
            return ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("Symbol '{}' not found", name),
                }],
                is_error: Some(true),
            };
        }

        let mut output = format!("References to '{}':\n", name);
        for sym in &symbols {
            if let Some(file) = self.codesift.get_file(sym.file_id) {
                output.push_str(&format!("- {}:{}:{}\n", file.path.display(), sym.range.start_line, sym.range.end_line));
            }
        }

        ToolResult {
            content: vec![ContentBlock::Text { text: output }],
            is_error: None,
        }
    }

    fn error(&self, msg: &str) -> ToolResult {
        ToolResult {
            content: vec![ContentBlock::Text {
                text: msg.to_string(),
            }],
            is_error: Some(true),
        }
    }

    fn format_context_plan(codesift: &CodeSift, plan: &ContextPlan) -> String {
        let mut output = String::new();
        output.push_str(&format!("Context ({} tokens / {} budget)\n", plan.total_tokens, plan.budget.max_tokens));
        output.push_str(&format!("Files: {}, Symbols: {}\n\n", plan.total_files, plan.len()));

        let mut files: std::collections::HashMap<String, Vec<&crate::ContextFragment>> =
            std::collections::HashMap::new();

        for frag in &plan.fragments {
            if let Some(file) = codesift.get_file(frag.file_id) {
                let path = file.path.to_string_lossy().to_string();
                files.entry(path).or_default().push(frag);
            }
        }

        for (path, frags) in &files {
            output.push_str(&format!("=== {} ===\n", path));
            for frag in frags {
                if let Some(name) = &frag.symbol_name {
                    output.push_str(&format!("[{}:{}] {}\n", frag.range.start_line, frag.range.end_line, name));
                }
                output.push_str(&format!("{}\n", frag.content.trim()));
            }
            output.push('\n');
        }

        output
    }
}

/// Parse and handle a raw JSON-RPC request.
pub fn handle_request(codesift: &CodeSift, raw: &str) -> String {
    let request: JsonRpcRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            let response = JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: serde_json::Value::Null,
                result: None,
                error: Some(JsonRpcError {
                    code: error_codes::PARSE_ERROR,
                    message: format!("Parse error: {}", e),
                    data: None,
                }),
            };
            return serde_json::to_string(&response).unwrap_or_default();
        }
    };

    let mut server = McpServer::new(codesift.clone());
    let response = server.handle(request);
    serde_json::to_string(&response).unwrap_or_default()
}
