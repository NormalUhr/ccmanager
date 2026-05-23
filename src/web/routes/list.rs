//! Conversation list: `/conversations` (full page) + `/api/list` (htmx fragment).

use askama::Template;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{Router, routing::get};
use chrono::{DateTime, Local};
use serde::Deserialize;

use crate::tui::search::{self as tui_search};
use crate::web::error::{WebError, WebResult};
use crate::web::model::ListRow;
use crate::web::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(|| async { Redirect::permanent("/conversations") }))
        // Unified list endpoint: full page for a browser nav, fragment for
        // an htmx swap. Decision is driven by the HX-Request header.
        .route("/conversations", get(list_handler))
        // Compat: prior htmx target. Redirects so old bookmarks / links
        // still resolve. No query params preserved here — the caller
        // should be using /conversations directly.
        .route(
            "/api/list",
            get(|| async { Redirect::permanent("/conversations") }),
        )
}

#[derive(Deserialize, Default)]
pub struct ListQuery {
    #[serde(default)]
    pub q: String,
}

#[derive(Template)]
#[template(path = "list.html")]
struct ListPage {
    q: String,
    total: usize,
    rows: Vec<ListRow>,
}

#[derive(Template)]
#[template(path = "list_rows.html")]
struct ListRows {
    q: String,
    rows: Vec<ListRow>,
}

/// Unified `/conversations` handler.
///
/// Htmx requests (recognized by the `HX-Request: true` header htmx adds to
/// AJAX calls) get only the rows fragment so the browser swaps in place.
/// Full browser navigations — including back/forward — get the whole page,
/// so URL state (`?q=…`) fully restores both the search input and the
/// results without any JS.
async fn list_handler(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
    headers: HeaderMap,
) -> WebResult<Response> {
    let is_htmx = headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("true"));

    let rows = compute_rows(&state, &q.q);
    if is_htmx {
        render(&ListRows { q: q.q, rows })
    } else {
        render(&ListPage {
            q: q.q,
            total: rows.len(),
            rows,
        })
    }
}

fn compute_rows(state: &AppState, query: &str) -> Vec<ListRow> {
    let convs = state.conversations.read().expect("conversations lock");
    let searchable = state.searchable.read().expect("searchable lock");
    let now = Local::now();

    let ordered_indices: Vec<usize> = if query.trim().is_empty() {
        // Default order: newest first, already sorted at load time.
        (0..convs.len()).collect()
    } else {
        tui_search::search(&convs, &searchable, query, now)
    };

    ordered_indices
        .into_iter()
        .take(500) // cap for huge histories; paging lands in v0.3
        .map(|idx| to_row(&convs[idx], now))
        .collect()
}

fn to_row(conv: &crate::history::Conversation, now: DateTime<Local>) -> ListRow {
    let session_id = conv
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();
    let relative_url = format!("/conversations/{}", urlencoding::encode(&session_id));

    // One-line preview — pick whichever side the parser cached.
    let preview = first_nonempty_line(&conv.preview);

    ListRow {
        session_id,
        project_name: conv.project_name.clone(),
        custom_title: conv.custom_title.clone(),
        model: conv.model.clone(),
        message_count: conv.message_count,
        total_tokens: conv.total_tokens,
        timestamp: conv.timestamp,
        age_short: format_age(conv.timestamp, now),
        preview,
        relative_url,
    }
}

fn first_nonempty_line(preview: &str) -> String {
    for line in preview.lines() {
        let t = line.trim();
        if !t.is_empty() {
            // Trim preview to a sensible width for the list row.
            let truncated: String = t.chars().take(140).collect();
            return truncated;
        }
    }
    String::new()
}

fn format_age(ts: DateTime<Local>, now: DateTime<Local>) -> String {
    let delta = now.signed_duration_since(ts);
    if delta.num_seconds() < 60 {
        return "now".into();
    }
    if delta.num_minutes() < 60 {
        return format!("{}m ago", delta.num_minutes());
    }
    if delta.num_hours() < 24 {
        return format!("{}h ago", delta.num_hours());
    }
    if delta.num_days() < 30 {
        return format!("{}d ago", delta.num_days());
    }
    ts.format("%Y-%m-%d").to_string()
}

/// Minimal Askama → axum HTML response adapter (askama_axum was dropped in
/// 0.14; this is the same 6-line helper that crate provided).
pub(crate) fn render<T: Template>(template: &T) -> WebResult<Response> {
    match template.render() {
        Ok(body) => Ok((StatusCode::OK, Html(body)).into_response()),
        Err(e) => Err(WebError::Template(e)),
    }
}
