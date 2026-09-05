//! MCP server adapter for CodeSift.
//!
//! Provides JSON-RPC 2.0 protocol for AI coding agents to query code context.

use crate::{CodeSift, ContextPlan, TokenBudget};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Default request timeout.
const DEFAULT_TIMEOUT_MS: u64 = 30000;

/// JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
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
    pub const REQUEST_CANCELLED: i32 = -32800;
    pub const REQUEST_TIMEOUT: i32 = -32801;
}

/// MCP tool definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// MCP tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "isError")]
    pub is_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ToolResultMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

/// Progress notification for long operations.
#[derive(Debug, Serialize)]
pub struct ProgressNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: ProgressParams,
}

#[derive(Debug, Serialize)]
pub struct ProgressParams {
    pub token: String,
    pub progress: f64,
    pub total: Option<f64>,
    pub message: Option<String>,
}

/// Workspace information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub path: String,
    pub name: String,
}

/// Session state.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub workspace: Option<Workspace>,
    pub created_at: u64,
    pub request_count: u64,
    pub cancelled_requests: u64,
}

/// Cancellation token for request cancellation.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    request_id: Arc<AtomicU64>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            request_id: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    pub fn set_request_id(&self, id: u64) {
        self.request_id.store(id, Ordering::SeqCst);
    }

    pub fn get_request_id(&self) -> u64 {
        self.request_id.load(Ordering::SeqCst)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// MCP server state.
pub struct McpServer {
    codesift: CodeSift,
    initialized: bool,
    session: Option<Session>,
    cancellation_token: CancellationToken,
    timeout_ms: u64,
}

impl McpServer {
    pub fn new(codesift: CodeSift) -> Self {
        Self {
            codesift,
            initialized: false,
            session: None,
            cancellation_token: CancellationToken::new(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Get the cancellation token for external cancellation.
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    /// Set request timeout in milliseconds.
    pub fn set_timeout(&mut self, ms: u64) {
        self.timeout_ms = ms;
    }

    /// Process a JSON-RPC request and return response. Returns None for
    /// notifications (requests without an `id`), which must not be answered.
    pub fn handle(&mut self, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let is_notification = request.id.is_null();

        if request.jsonrpc != "2.0" {
            if is_notification {
                return None;
            }
            return Some(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: error_codes::INVALID_REQUEST,
                    message: "Invalid JSON-RPC version".into(),
                    data: None,
                }),
            });
        }

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(request.params),
            "shutdown" => self.handle_shutdown(),
            "cancel" => self.handle_cancel(request.params),
            "health" => self.handle_health(),
            "session/status" => self.handle_session_status(),
            "workspace/info" => self.handle_workspace_info(request.params),
            _ => Err((
                error_codes::METHOD_NOT_FOUND,
                format!("Unknown method: {}", request.method),
            )),
        };

        if is_notification {
            return None;
        }

        match result {
            Ok(value) => Some(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: Some(value),
                error: None,
            }),
            Err((code, msg)) => Some(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code,
                    message: msg,
                    data: None,
                }),
            }),
        }
    }

    fn handle_initialize(&mut self, params: Option<Value>) -> Result<Value, (i32, String)> {
        // Check for timeout override in params
        if let Some(ref p) = params {
            if let Some(timeout) = p.get("timeout").and_then(|v| v.as_u64()) {
                self.timeout_ms = timeout;
            }
        }

        self.initialized = true;
        self.session = Some(Session {
            id: format!(
                "session-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            ),
            workspace: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            request_count: 0,
            cancelled_requests: 0,
        });

        Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "codesift",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {},
                "progress": {},
                "cancellation": {}
            }
        }))
    }

    fn handle_tools_list(&self) -> Result<Value, (i32, String)> {
        let tools = self.list_tools();
        Ok(serde_json::json!({ "tools": tools }))
    }

    fn handle_tools_call(&mut self, params: Option<Value>) -> Result<Value, (i32, String)> {
        let params = params.ok_or((error_codes::INVALID_PARAMS, "Missing params".into()))?;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or((error_codes::INVALID_PARAMS, "Missing 'name' param".into()))?;

        let arguments = params
            .get("arguments")
            .and_then(|v| v.as_object())
            .map(|m| serde_json::Value::Object(m.clone()))
            .unwrap_or(serde_json::Value::Null);

        // Update session request count
        if let Some(ref mut session) = self.session {
            session.request_count += 1;
        }

        // Execute with timeout
        let start = Instant::now();
        let result = self.execute_tool(name, &arguments);

        // Check if cancelled
        if self.cancellation_token.is_cancelled() {
            self.cancellation_token.reset();
            return Err((
                error_codes::REQUEST_CANCELLED,
                "Request was cancelled".into(),
            ));
        }

        // Check timeout
        if start.elapsed().as_millis() as u64 > self.timeout_ms {
            return Err((
                error_codes::REQUEST_TIMEOUT,
                format!("Request timed out after {}ms", self.timeout_ms),
            ));
        }

        // Add timing info
        let duration_ms = start.elapsed().as_millis() as u64;
        let meta = ToolResultMeta {
            duration_ms: Some(duration_ms),
            tokens_used: None,
        };

        Ok(serde_json::json!({
            "content": result.content,
            "isError": result.is_error.unwrap_or(false),
            "meta": meta
        }))
    }

    fn handle_shutdown(&self) -> Result<Value, (i32, String)> {
        Ok(serde_json::json!({ "shutdown": true }))
    }

    fn handle_cancel(&mut self, _params: Option<Value>) -> Result<Value, (i32, String)> {
        // Cancel the current request
        self.cancellation_token.cancel();

        if let Some(ref mut session) = self.session {
            session.cancelled_requests += 1;
        }

        Ok(serde_json::json!({ "cancelled": true }))
    }

    fn handle_health(&self) -> Result<Value, (i32, String)> {
        Ok(serde_json::json!({
            "status": "ok",
            "files": self.codesift.file_count(),
            "symbols": self.codesift.symbol_count(),
            "session": self.session.as_ref().map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "requests": s.request_count,
                    "cancelled": s.cancelled_requests
                })
            })
        }))
    }

    fn handle_session_status(&self) -> Result<Value, (i32, String)> {
        match &self.session {
            Some(s) => Ok(serde_json::json!({
                "id": s.id,
                "workspace": s.workspace,
                "created_at": s.created_at,
                "request_count": s.request_count,
                "cancelled_requests": s.cancelled_requests,
                "initialized": self.initialized
            })),
            None => Ok(serde_json::json!({
                "initialized": false
            })),
        }
    }

    fn handle_workspace_info(&self, _params: Option<Value>) -> Result<Value, (i32, String)> {
        let workspace = self.session.as_ref().and_then(|s| s.workspace.clone());

        if let Some(ref ws) = workspace {
            Ok(serde_json::json!({
                "path": ws.path,
                "name": ws.name,
                "files": self.codesift.file_count(),
                "symbols": self.codesift.symbol_count()
            }))
        } else {
            Ok(serde_json::json!({
                "configured": false
            }))
        }
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
                meta: None,
            },
        }
    }

    fn tool_search_code(&self, args: &Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return self.error("Missing 'query' argument"),
        };
        let max_tokens = args
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(2000) as usize;

        let budget = TokenBudget::new(max_tokens);

        match self.codesift.search(query, Some(budget)) {
            Ok(matches) => {
                let file_count = matches
                    .iter()
                    .map(|m| m.file_id)
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                let summary = format!("Found {} matches in {} files", matches.len(), file_count);
                ToolResult {
                    content: vec![ContentBlock::Text { text: summary }],
                    is_error: None,
                    meta: None,
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
                meta: None,
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
            meta: None,
        }
    }

    fn tool_get_context(&self, args: &Value) -> ToolResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return self.error("Missing 'query' argument"),
        };
        let max_tokens = args
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(2000) as usize;
        let max_files = args.get("max_files").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let budget = TokenBudget::new(max_tokens).with_files(max_files);

        match self.codesift.plan_context(query, Some(budget)) {
            Ok(plan) => {
                let output = Self::format_context_plan(&self.codesift, &plan);
                ToolResult {
                    content: vec![ContentBlock::Text { text: output }],
                    is_error: None,
                    meta: Some(ToolResultMeta {
                        duration_ms: None,
                        tokens_used: Some(plan.total_tokens),
                    }),
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
                meta: None,
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
                        output.push_str(&format!(
                            "{} depth {}: {}\n",
                            "  ".repeat(d),
                            d,
                            caller.name
                        ));
                    }
                }
            }
        }

        ToolResult {
            content: vec![ContentBlock::Text { text: output }],
            is_error: None,
            meta: None,
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
                meta: None,
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
                        output.push_str(&format!(
                            "{} depth {}: {}\n",
                            "  ".repeat(d),
                            d,
                            callee.name
                        ));
                    }
                }
            }
        }

        ToolResult {
            content: vec![ContentBlock::Text { text: output }],
            is_error: None,
            meta: None,
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
                meta: None,
            };
        }

        // Return first definition
        let sym = &symbols[0];
        if let Some(file) = self.codesift.get_file(sym.file_id) {
            let source = self.codesift.get_source(sym.file_id, &sym.range);
            let content = source.unwrap_or_default();
            ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!(
                        "{}:{}:{}\n\n{}",
                        file.path.display(),
                        sym.range.start_line,
                        sym.range.end_line,
                        content
                    ),
                }],
                is_error: None,
                meta: None,
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
                meta: None,
            };
        }

        let mut output = format!("References to '{}':\n", name);
        for sym in &symbols {
            if let Some(file) = self.codesift.get_file(sym.file_id) {
                output.push_str(&format!(
                    "- {}:{}:{}\n",
                    file.path.display(),
                    sym.range.start_line,
                    sym.range.end_line
                ));
            }
        }

        ToolResult {
            content: vec![ContentBlock::Text { text: output }],
            is_error: None,
            meta: None,
        }
    }

    fn error(&self, msg: &str) -> ToolResult {
        ToolResult {
            content: vec![ContentBlock::Text {
                text: msg.to_string(),
            }],
            is_error: Some(true),
            meta: None,
        }
    }

    fn format_context_plan(codesift: &CodeSift, plan: &ContextPlan) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "Context ({} tokens / {} budget)\n",
            plan.total_tokens, plan.budget.max_tokens
        ));
        output.push_str(&format!(
            "Files: {}, Symbols: {}\n\n",
            plan.total_files,
            plan.len()
        ));

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
                    output.push_str(&format!(
                        "[{}:{}] {}\n",
                        frag.range.start_line, frag.range.end_line, name
                    ));
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
