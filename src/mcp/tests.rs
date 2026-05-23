//! Tests for the MCP server.
//!
//! Split into:
//!   * tool-level unit tests (pure functions on McpState)
//!   * JSON-RPC protocol tests that drive [`super::server::handle_message`]
//!     with the exact bytes Claude Code would send.

#![cfg(test)]

use super::server::handle_message;
use super::state::McpState;
use super::tools;
use serde_json::{Value, json};
use std::sync::Arc;

// ---------- fixtures ----------

fn build_state_with_fixtures() -> (tempfile::TempDir, McpState) {
    let tmp = tempfile::TempDir::new().unwrap();
    let projects = tmp.path().join("projects");
    let proj_dir = projects.join("-fixtures-proj");
    std::fs::create_dir_all(&proj_dir).unwrap();

    // Three sessions. Stable session IDs so tests can reference them.
    write_session(
        &proj_dir,
        "11111111-1111-1111-1111-111111111111",
        "how do I debug an OOM in the rl training loop?",
        "Start by checking the memory usage with nvidia-smi during training.",
        Some("debugging training OOM"),
    );
    write_session(
        &proj_dir,
        "22222222-2222-2222-2222-222222222222",
        "can you help me add a caching layer to the API?",
        "Sure — we can use Redis for session cache; here's a sketch.",
        None,
    );
    write_session(
        &proj_dir,
        "33333333-3333-3333-3333-333333333333",
        "recommend a python web framework",
        "FastAPI is a solid modern choice for Python.",
        None,
    );

    unsafe {
        std::env::set_var("CLAUDE_CONFIG_DIR", tmp.path());
    }
    let state = McpState::load().expect("load");
    (tmp, state)
}

fn write_session(
    dir: &std::path::Path,
    session_id: &str,
    user: &str,
    claude: &str,
    custom_title: Option<&str>,
) {
    let mut body = String::new();
    body.push_str(&format!(
        "{{\"type\":\"user\",\"message\":{{\"role\":\"user\",\"content\":{:?}}},\"timestamp\":\"2026-04-22T00:00:00Z\"}}\n",
        user
    ));
    body.push_str(&format!(
        "{{\"type\":\"assistant\",\"message\":{{\"id\":\"m\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"text\",\"text\":{:?}}}],\"model\":\"claude-opus-4-7\",\"stop_reason\":\"end_turn\",\"stop_sequence\":null,\"usage\":{{\"input_tokens\":0,\"output_tokens\":0}}}},\"timestamp\":\"2026-04-22T00:00:01Z\"}}\n",
        claude
    ));
    if let Some(t) = custom_title {
        body.push_str(&format!(
            "{{\"type\":\"custom-title\",\"customTitle\":{:?},\"sessionId\":\"{}\"}}\n",
            t, session_id
        ));
    }
    std::fs::write(dir.join(format!("{}.jsonl", session_id)), body).unwrap();
}

// ---------- tool-level unit tests ----------

#[test]
fn search_history_finds_by_content() {
    let (_tmp, state) = build_state_with_fixtures();
    let out = tools::search_history(&state, &json!({ "query": "OOM" })).expect("ok");
    // Should only surface the OOM session.
    assert!(
        out.contains("11111111"),
        "OOM session should match: {}",
        out
    );
    assert!(!out.contains("33333333"), "python session shouldn't match");
}

#[test]
fn search_history_surfaces_custom_title() {
    let (_tmp, state) = build_state_with_fixtures();
    let out = tools::search_history(&state, &json!({ "query": "debugging" })).expect("ok");
    assert!(out.contains("debugging training OOM"));
    assert!(out.contains("11111111"));
}

#[test]
fn search_history_requires_query() {
    let (_tmp, state) = build_state_with_fixtures();
    let err = tools::search_history(&state, &json!({})).unwrap_err();
    assert!(err.contains("query"), "error should mention query: {}", err);
}

#[test]
fn search_history_respects_limit() {
    let (_tmp, state) = build_state_with_fixtures();
    let out = tools::search_history(&state, &json!({ "query": "the", "limit": 1 })).expect("ok");
    // JSON array with exactly one object.
    let start = out.find('[').unwrap();
    let parsed: Value = serde_json::from_str(&out[start..]).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

#[test]
fn search_history_empty_when_no_match() {
    let (_tmp, state) = build_state_with_fixtures();
    let out =
        tools::search_history(&state, &json!({ "query": "kubernetes-foo-zzyyy" })).expect("ok");
    assert!(out.contains("No conversations matched"), "got:\n{}", out);
}

#[test]
fn get_session_renders_markdown() {
    let (_tmp, state) = build_state_with_fixtures();
    let out = tools::get_session(
        &state,
        &json!({ "session_id": "22222222-2222-2222-2222-222222222222" }),
    )
    .expect("ok");
    // Header + dialogue
    assert!(out.contains("session: 22222222"));
    assert!(out.contains("## You"));
    assert!(out.contains("caching layer"));
    assert!(out.contains("## Claude"));
    assert!(out.contains("Redis"));
}

#[test]
fn get_session_respects_max_chars() {
    let (_tmp, state) = build_state_with_fixtures();
    let out = tools::get_session(
        &state,
        &json!({ "session_id": "22222222-2222-2222-2222-222222222222", "max_chars": 120 }),
    )
    .expect("ok");
    // Truncation note must appear in sub-max-chars output.
    assert!(
        out.contains("[truncated"),
        "expected truncation note in:\n{}",
        out
    );
}

#[test]
fn get_session_unknown_id_returns_tool_error() {
    let (_tmp, state) = build_state_with_fixtures();
    let err = tools::get_session(&state, &json!({ "session_id": "deadbeef" })).unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
fn list_recent_sessions_returns_recent_first() {
    let (_tmp, state) = build_state_with_fixtures();
    let out = tools::list_recent_sessions(&state, &json!({})).expect("ok");
    assert!(out.contains("Found 3 recent"));
    // All three session IDs present.
    assert!(out.contains("11111111"));
    assert!(out.contains("22222222"));
    assert!(out.contains("33333333"));
}

// ---------- JSON-RPC protocol tests ----------

async fn rpc_call(state: &Arc<McpState>, raw: &str) -> Option<Value> {
    let resp = handle_message(state, raw).await?;
    Some(serde_json::to_value(&resp).unwrap())
}

#[tokio::test]
async fn initialize_returns_server_info_and_protocol_version() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"claude","version":"1"}}}"#;
    let resp = rpc_call(&state, req).await.expect("response");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let result = &resp["result"];
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "ccmanager");
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["instructions"].as_str().unwrap().contains("history"));
}

#[tokio::test]
async fn initialized_notification_gets_no_response() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    // `notifications/initialized` has no id and expects no response.
    let resp = handle_message(
        &state,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    )
    .await;
    assert!(resp.is_none(), "notification must not produce a response");
}

#[tokio::test]
async fn tools_list_returns_three_tools() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    let resp = rpc_call(&state, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
        .await
        .unwrap();
    let tools_arr = resp["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools_arr
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"search_history"));
    assert!(names.contains(&"get_session"));
    assert!(names.contains(&"list_recent_sessions"));
    // Each tool has a description and inputSchema
    for t in tools_arr {
        assert!(t["description"].is_string());
        assert_eq!(t["inputSchema"]["type"], "object");
    }
}

#[tokio::test]
async fn tools_call_search_history_returns_text_content() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    let req = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search_history","arguments":{"query":"OOM"}}}"#;
    let resp = rpc_call(&state, req).await.unwrap();
    assert!(resp.get("error").is_none(), "unexpected error: {:?}", resp);
    let content = resp["result"]["content"][0].clone();
    assert_eq!(content["type"], "text");
    assert!(content["text"].as_str().unwrap().contains("11111111"));
    assert_eq!(resp["result"]["isError"].as_bool(), None); // default false, serialized as absent
}

#[tokio::test]
async fn tools_call_unknown_returns_jsonrpc_method_not_found() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    let req = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"not_a_tool","arguments":{}}}"#;
    let resp = rpc_call(&state, req).await.unwrap();
    let err = resp.get("error").expect("error");
    assert_eq!(err["code"], -32601);
    assert!(err["message"].as_str().unwrap().contains("not_a_tool"));
}

#[tokio::test]
async fn tools_call_missing_args_surfaces_as_is_error() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    // search_history with no query — should come back as isError: true
    // inside a result, NOT as a JSON-RPC error.
    let req = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"search_history","arguments":{}}}"#;
    let resp = rpc_call(&state, req).await.unwrap();
    assert!(resp.get("error").is_none(), "should not be jsonrpc error");
    let result = &resp["result"];
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("query")
    );
}

#[tokio::test]
async fn malformed_json_returns_parse_error() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    let resp = rpc_call(&state, "not json{").await.unwrap();
    let err = resp.get("error").expect("error");
    assert_eq!(err["code"], -32700);
    assert_eq!(resp["id"], Value::Null);
}

#[tokio::test]
async fn ping_returns_empty_result() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    let resp = rpc_call(&state, r#"{"jsonrpc":"2.0","id":99,"method":"ping"}"#)
        .await
        .unwrap();
    assert_eq!(resp["id"], 99);
    assert_eq!(resp["result"], json!({}));
}

#[tokio::test]
async fn unknown_method_returns_method_not_found() {
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);
    let resp = rpc_call(
        &state,
        r#"{"jsonrpc":"2.0","id":7,"method":"no/such/method"}"#,
    )
    .await
    .unwrap();
    assert_eq!(resp["error"]["code"], -32601);
}

#[tokio::test]
async fn end_to_end_flow() {
    // Full exchange Claude Code would perform: initialize → initialized →
    // tools/list → tools/call(search_history) → tools/call(get_session).
    let (_tmp, state) = build_state_with_fixtures();
    let state = Arc::new(state);

    rpc_call(
        &state,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
    )
    .await
    .unwrap();
    assert!(
        handle_message(
            &state,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
        .await
        .is_none()
    );

    let list = rpc_call(&state, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
        .await
        .unwrap();
    assert!(list["result"]["tools"].is_array());

    let search = rpc_call(
        &state,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search_history","arguments":{"query":"caching"}}}"#,
    )
    .await
    .unwrap();
    let text = search["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("22222222"));

    let get = rpc_call(
        &state,
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_session","arguments":{"session_id":"22222222-2222-2222-2222-222222222222"}}}"#,
    )
    .await
    .unwrap();
    let text = get["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("## Claude"));
    assert!(text.contains("Redis"));
}
