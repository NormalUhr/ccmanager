//! Tool implementations for the MCP server.
//!
//! Each tool is a pure function over `McpState` + the JSON args Claude
//! Code sent. Returning `Ok(String)` becomes a normal text result to
//! Claude; `Err(String)` becomes a tool-level error (isError: true) so
//! Claude sees the message and can recover.
//!
//! All output is size-capped to protect Claude's context.

use super::state::McpState;
use crate::history::Conversation;
use crate::tui::search::{self as tui_search};
use chrono::{DateTime, Local};
use serde_json::{Value, json};
use std::path::Path;

// ---- limits ----
const DEFAULT_SEARCH_LIMIT: usize = 10;
const MAX_SEARCH_LIMIT: usize = 30;
const DEFAULT_SESSION_MAX_CHARS: usize = 50_000;
const MAX_SESSION_MAX_CHARS: usize = 200_000;
const SNIPPET_BYTES: usize = 200;

/// JSON schemas and human descriptions for the MCP `tools/list` response.
/// Descriptions are what Claude reads to decide whether to call a tool,
/// so they're deliberately action-oriented.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "search_history",
            "description": concat!(
                "Fuzzy-search the user's past Claude Code conversations by text. ",
                "Returns up to `limit` matching conversations as JSON — each item ",
                "has session_id, title, project, age, message_count, and a short ",
                "snippet. Call when the user asks about prior work, e.g. ",
                "\"have I hit this before?\", \"what did we discuss about X?\". ",
                "Pass the session_id to get_session to read the full transcript."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Text to search for. Fuzzy, case-insensitive, matches across titles, summaries, and message bodies."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results (default 10, max 30)."
                    },
                    "project": {
                        "type": "string",
                        "description": "Optional: restrict to conversations whose project name contains this substring."
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_session",
            "description": concat!(
                "Retrieve a specific past conversation by session_id and return ",
                "its transcript as markdown. The transcript is dialogue-only — ",
                "user prompts and Claude's final answers per round, with tool ",
                "calls, tool results, and intermediate narration filtered out. ",
                "Use this after search_history to read a promising match in full."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "UUID from search_history or list_recent_sessions."
                    },
                    "max_chars": {
                        "type": "integer",
                        "description": "Truncate the markdown to at most this many characters (default 50000, max 200000)."
                    }
                },
                "required": ["session_id"]
            }
        },
        {
            "name": "list_recent_sessions",
            "description": concat!(
                "List the most recent Claude Code conversations (newest first). ",
                "Same JSON shape as search_history. Useful for \"what was I ",
                "working on yesterday\" or \"show me recent sessions.\""
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max items (default 10, max 30)."
                    },
                    "project": {
                        "type": "string",
                        "description": "Optional: restrict to conversations whose project name contains this substring."
                    }
                }
            }
        }
    ])
}

// ---------- search_history ----------

pub fn search_history(state: &McpState, args: &Value) -> std::result::Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing argument: query".to_string())?;
    let limit = parse_limit(args, DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT);
    let project_filter = args
        .get("project")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    if query.trim().is_empty() {
        return Err("query must not be empty".to_string());
    }

    let now = Local::now();
    let ranked = tui_search::search(&state.conversations, &state.searchable, query, now);

    let results: Vec<Value> = ranked
        .into_iter()
        .filter_map(|idx| state.conversations.get(idx))
        .filter(|c| matches_project(c, project_filter.as_deref()))
        .take(limit)
        .map(|c| conversation_to_row(c, query, now))
        .collect();

    Ok(format_results(&results, query))
}

// ---------- list_recent_sessions ----------

pub fn list_recent_sessions(state: &McpState, args: &Value) -> std::result::Result<String, String> {
    let limit = parse_limit(args, DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT);
    let project_filter = args
        .get("project")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let now = Local::now();
    let results: Vec<Value> = state
        .conversations
        .iter()
        .filter(|c| matches_project(c, project_filter.as_deref()))
        .take(limit)
        .map(|c| conversation_to_row(c, "", now))
        .collect();

    Ok(format_results(&results, ""))
}

// ---------- get_session ----------

pub fn get_session(state: &McpState, args: &Value) -> std::result::Result<String, String> {
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing argument: session_id".to_string())?;
    let max_chars = args
        .get("max_chars")
        .and_then(|v| v.as_u64())
        .map(|n| (n as usize).min(MAX_SESSION_MAX_CHARS))
        .unwrap_or(DEFAULT_SESSION_MAX_CHARS);

    let conv = state
        .conversations
        .iter()
        .find(|c| session_id_of(&c.path).as_deref() == Some(session_id))
        .ok_or_else(|| format!("session_id not found: {}", session_id))?;

    // Reuse the same clean dialogue-only export as `ccmanager e` and
    // the web UI. Claude doesn't need to wade through tool noise to
    // understand what was discussed.
    let mut body = crate::tui::export::generate_markdown(&conv.path)
        .map_err(|e| format!("failed to render session: {}", e))?;

    // Header: metadata the user would see on the list row.
    let title = conv
        .custom_title
        .as_deref()
        .or(conv.summary.as_deref())
        .unwrap_or(session_id);
    let header = format!(
        "# {}\nsession: {}\nproject: {}\nmodel: {}\nmessages: {}\ntokens: {}\n\n",
        title,
        session_id,
        conv.project_name.as_deref().unwrap_or("(unknown)"),
        conv.model.as_deref().unwrap_or("(unknown)"),
        conv.message_count,
        conv.total_tokens,
    );

    let mut out = header;
    if body.len() > max_chars.saturating_sub(out.len()) {
        let take = max_chars.saturating_sub(out.len());
        // Chop cleanly at a codepoint boundary, then add a truncation note.
        let end = floor_char_boundary(&body, take);
        body.truncate(end);
        out.push_str(&body);
        out.push_str(&format!(
            "\n\n---\n[truncated at {} chars; call get_session again with a larger max_chars up to {} to see more]\n",
            out.len(),
            MAX_SESSION_MAX_CHARS
        ));
    } else {
        out.push_str(&body);
    }
    Ok(out)
}

// ---------- helpers ----------

fn parse_limit(args: &Value, default: usize, max: usize) -> usize {
    args.get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| (n as usize).clamp(1, max))
        .unwrap_or(default)
}

fn matches_project(conv: &Conversation, needle: Option<&str>) -> bool {
    let Some(needle) = needle else { return true };
    if needle.is_empty() {
        return true;
    }
    conv.project_name
        .as_deref()
        .is_some_and(|p| p.to_lowercase().contains(&needle.to_lowercase()))
}

fn session_id_of(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

fn conversation_to_row(conv: &Conversation, query: &str, now: DateTime<Local>) -> Value {
    let sid = session_id_of(&conv.path).unwrap_or_else(|| "?".into());
    let title = conv
        .custom_title
        .as_deref()
        .or(conv.summary.as_deref())
        .unwrap_or(&sid);
    let snippet = extract_snippet(conv, query);
    json!({
        "session_id": sid,
        "title": title,
        "project": conv.project_name.as_deref().unwrap_or(""),
        "age": format_age(conv.timestamp, now),
        "message_count": conv.message_count,
        "model": conv.model.as_deref().unwrap_or(""),
        "snippet": snippet,
    })
}

fn extract_snippet(conv: &Conversation, query: &str) -> String {
    // Try to find a line containing any query word; fall back to the first
    // non-empty preview line, then the full_text prefix.
    if !query.trim().is_empty() {
        let needle = query.to_lowercase();
        for line in conv.full_text.lines().chain(conv.preview.lines()) {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if t.to_lowercase().contains(&needle) {
                return truncate_chars(t, SNIPPET_BYTES);
            }
        }
    }
    for line in conv.preview.lines() {
        let t = line.trim();
        if !t.is_empty() {
            return truncate_chars(t, SNIPPET_BYTES);
        }
    }
    truncate_chars(conv.full_text.trim(), SNIPPET_BYTES)
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (taken, ch) in s.chars().enumerate() {
        if taken >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn format_age(ts: DateTime<Local>, now: DateTime<Local>) -> String {
    let d = now.signed_duration_since(ts);
    if d.num_seconds() < 60 {
        "just now".into()
    } else if d.num_minutes() < 60 {
        format!("{}m ago", d.num_minutes())
    } else if d.num_hours() < 24 {
        format!("{}h ago", d.num_hours())
    } else if d.num_days() < 30 {
        format!("{}d ago", d.num_days())
    } else {
        ts.format("%Y-%m-%d").to_string()
    }
}

fn format_results(results: &[Value], query: &str) -> String {
    let pretty = serde_json::to_string_pretty(results).unwrap_or_else(|_| "[]".to_string());
    if results.is_empty() {
        if query.is_empty() {
            "No conversations found in ~/.claude/projects/.".to_string()
        } else {
            format!("No conversations matched query: {:?}", query)
        }
    } else {
        let header = if query.is_empty() {
            format!("Found {} recent conversations:\n\n", results.len())
        } else {
            format!(
                "Found {} matching conversations for query {:?}. Call get_session(session_id) to read one in full.\n\n",
                results.len(),
                query,
            )
        };
        format!("{}{}", header, pretty)
    }
}

/// `str::floor_char_boundary` is unstable on stable Rust; inline a tiny
/// equivalent to keep the MSRV loose.
fn floor_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
