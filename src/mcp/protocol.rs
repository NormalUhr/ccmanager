//! JSON-RPC 2.0 + MCP 2024-11-05 protocol types and constants.
//!
//! We keep this as a tight, purpose-built module rather than pulling in an
//! MCP SDK. Everything Claude Code sends over stdio maps to one of five
//! method names handled in [`super::server`].

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP protocol version we speak. Must match what Claude Code announces
/// during `initialize`; if it requests a different version we still accept
/// it — the spec says the server can reply with the version it supports.
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// Standard JSON-RPC 2.0 error codes we emit.
#[allow(dead_code)]
pub mod error_code {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
}

/// Top-level JSON-RPC request / notification. We lean on `serde_json::Value`
/// for `id` because spec allows integer or string.
#[derive(Debug, Deserialize)]
pub struct Request {
    #[allow(dead_code)]
    pub jsonrpc: String,
    /// Absent for notifications — they don't receive responses.
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Response envelope sent back for every request that has an `id`.
#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
}

#[derive(Debug, Serialize)]
pub struct ErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Response {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
        Response {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(ErrorObject {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// Content block returned inside a `tools/call` result. The only variant
/// we emit is `text` — MCP also allows `image`/`resource`, but tools here
/// return only text payloads (JSON strings or markdown).
#[derive(Debug, Serialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub text: String,
}

impl TextContent {
    pub fn text(s: impl Into<String>) -> Self {
        TextContent {
            kind: "text",
            text: s.into(),
        }
    }
}

/// Body of a `tools/call` response.
#[derive(Debug, Serialize)]
pub struct CallToolResult {
    pub content: Vec<TextContent>,
    #[serde(rename = "isError", skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl CallToolResult {
    pub fn text(s: impl Into<String>) -> Self {
        CallToolResult {
            content: vec![TextContent::text(s)],
            is_error: false,
        }
    }

    pub fn error(s: impl Into<String>) -> Self {
        CallToolResult {
            content: vec![TextContent::text(s)],
            is_error: true,
        }
    }
}
