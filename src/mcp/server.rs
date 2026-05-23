//! JSON-RPC loop over stdin/stdout. See [`super`] module doc for the
//! transport contract (stdout is JSON only; stderr for logs).

use super::protocol::{CallToolResult, MCP_PROTOCOL_VERSION, Request, Response, error_code};
use super::state::McpState;
use super::tools;
use crate::error::{AppError, Result};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Top-level event loop. Reads one JSON-RPC message per line until stdin
/// closes (which is how Claude Code signals shutdown), dispatches to the
/// handler, and writes the response line back to stdout.
pub async fn run(state: McpState) -> Result<()> {
    let state = Arc::new(state);
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let stdout = tokio::io::stdout();
    let mut stdout = stdout;

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| AppError::ConfigError(format!("stdin read: {}", e)))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_message(&state, &line).await;
        if let Some(resp) = response {
            let mut buf =
                serde_json::to_vec(&resp).map_err(|e| AppError::ConfigError(e.to_string()))?;
            buf.push(b'\n');
            // If the client (Claude Code) has closed its end of our pipe,
            // treat it as a clean shutdown — not a server error.
            if let Err(e) = stdout.write_all(&buf).await {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    return Ok(());
                }
                return Err(AppError::ConfigError(format!("stdout write: {}", e)));
            }
            if let Err(e) = stdout.flush().await {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    return Ok(());
                }
                return Err(AppError::ConfigError(format!("stdout flush: {}", e)));
            }
        }
    }
    Ok(())
}

/// Parse one JSON-RPC message and dispatch. Returns `None` for
/// notifications (no `id` field in the request, so no response per spec).
/// Exposed for testing so we can drive the full method handler without
/// spinning up stdio.
pub async fn handle_message(state: &Arc<McpState>, raw: &str) -> Option<Response> {
    let req: Request = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("mcp: parse error: {}", e);
            // Parse errors return id=null per JSON-RPC spec.
            return Some(Response::err(
                Value::Null,
                error_code::PARSE_ERROR,
                format!("parse error: {}", e),
            ));
        }
    };

    // Notifications carry no id and expect no response; we acknowledge some
    // (e.g. `notifications/initialized`) and drop the rest silently.
    let is_notification = req.id.is_none();

    let id = req.id.clone().unwrap_or(Value::Null);
    let result = match req.method.as_str() {
        "initialize" => Ok(initialize_result()),
        // Standard notification Claude Code sends after initialize; no-op.
        "notifications/initialized" | "initialized" => {
            return None;
        }
        "tools/list" => Ok(tools_list_result()),
        "tools/call" => call_tool(state, &req.params).await,
        "ping" => Ok(json!({})),
        // These happen to be part of the MCP spec but we don't implement
        // them yet. Respond with an empty list so clients don't break.
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        other => Err((
            error_code::METHOD_NOT_FOUND,
            format!("method not found: {}", other),
        )),
    };

    if is_notification {
        // Methods we don't route should still not produce a response when
        // the client sent a notification-style message.
        return None;
    }

    Some(match result {
        Ok(value) => Response::ok(id, value),
        Err((code, msg)) => Response::err(id, code, msg),
    })
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            // We only advertise tools. No resources/prompts for v1.
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "ccmanager",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": concat!(
            "Read-only access to the user's local Claude Code conversation history ",
            "(at ~/.claude/projects/). Use search_history to find past conversations ",
            "by fuzzy text match, get_session to read one conversation end-to-end, ",
            "and list_recent_sessions for recent activity. The user's instruction ",
            "to consult past history is required — don't call these tools unprompted.",
        ),
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": tools::tool_definitions(),
    })
}

async fn call_tool(
    state: &Arc<McpState>,
    params: &Value,
) -> std::result::Result<Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (error_code::INVALID_PARAMS, "missing `name`".to_string()))?;
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    let result = match name {
        "search_history" => tools::search_history(state, &args),
        "get_session" => tools::get_session(state, &args),
        "list_recent_sessions" => tools::list_recent_sessions(state, &args),
        other => {
            return Err((
                error_code::METHOD_NOT_FOUND,
                format!("unknown tool: {}", other),
            ));
        }
    };

    // Tool errors are surfaced via the `isError: true` field inside the
    // CallToolResult — NOT as JSON-RPC errors, per the MCP spec. That lets
    // Claude see the error message and decide how to recover.
    let call_result = match result {
        Ok(text) => CallToolResult::text(text),
        Err(msg) => CallToolResult::error(msg),
    };
    serde_json::to_value(&call_result).map_err(|e| (error_code::INTERNAL_ERROR, e.to_string()))
}
