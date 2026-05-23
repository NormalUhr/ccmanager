//! Mutation + download endpoints: rename, delete, export.
//!
//! Mutations (rename, delete) are refused in read-only mode. Export is a
//! read-only endpoint and is always enabled. All three update the shared
//! search index when they affect state.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Form, Router, routing::get, routing::post};
use serde::Deserialize;
use std::path::PathBuf;

use crate::history::{self, Conversation};
use crate::tui::export::{ExportFormat, generate_ledger, generate_markdown, generate_plain};
use crate::web::error::{WebError, WebResult};
use crate::web::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/conversations/{id}/rename", post(rename))
        .route("/conversations/{id}/delete", post(delete))
        .route("/conversations/{id}/export.{ext}", get(export_download))
}

#[derive(Deserialize, Debug)]
pub struct RenameForm {
    pub title: String,
}

#[derive(Deserialize, Debug)]
pub struct DeleteForm {
    /// User must type the session ID to confirm — protects against misclicks.
    pub confirm: String,
}

async fn rename(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<RenameForm>,
) -> WebResult<Response> {
    if state.cfg.read_only {
        return Err(WebError::BadRequest(
            "server started with --read-only; rename disabled".into(),
        ));
    }

    let path = resolve_path(&state, &id)?;
    let trimmed = form.title.trim();
    history::rename::write_custom_title(&path, &id, trimmed).map_err(WebError::from)?;

    // Update in-memory + search index.
    {
        let mut convs = state.conversations.write().expect("conversations lock");
        if let Some(conv) = convs
            .iter_mut()
            .find(|c| session_id_of(c).as_deref() == Some(id.as_str()))
        {
            conv.custom_title = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }
    }
    state.rebuild_searchable();

    Ok(Redirect::to(&format!("/conversations/{}", urlencoding::encode(&id))).into_response())
}

async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<DeleteForm>,
) -> WebResult<Response> {
    if state.cfg.read_only {
        return Err(WebError::BadRequest(
            "server started with --read-only; delete disabled".into(),
        ));
    }
    if form.confirm.trim() != id {
        return Err(WebError::BadRequest(format!(
            "confirmation did not match session id {}",
            id
        )));
    }

    history::delete_session_by_uuid(&id).map_err(WebError::from)?;

    {
        let mut convs = state.conversations.write().expect("conversations lock");
        convs.retain(|c| session_id_of(c).as_deref() != Some(id.as_str()));
        for (i, c) in convs.iter_mut().enumerate() {
            c.index = i;
        }
    }
    state.rebuild_searchable();

    Ok(Redirect::to("/conversations").into_response())
}

async fn export_download(
    State(state): State<AppState>,
    Path((id, ext)): Path<(String, String)>,
) -> WebResult<Response> {
    let path = resolve_path(&state, &id)?;
    let format = match ext.as_str() {
        "md" => ExportFormat::Markdown,
        "txt" => ExportFormat::Plain,
        "ledger" => ExportFormat::Ledger,
        "jsonl" => ExportFormat::Jsonl,
        other => {
            return Err(WebError::BadRequest(format!(
                "unsupported export format '.{}' — expected md|txt|ledger|jsonl",
                other
            )));
        }
    };

    let (body, content_type) = match format {
        ExportFormat::Markdown => (generate_markdown(&path)?, "text/markdown; charset=utf-8"),
        ExportFormat::Plain => (generate_plain(&path)?, "text/plain; charset=utf-8"),
        ExportFormat::Ledger => (generate_ledger(&path)?, "text/plain; charset=utf-8"),
        ExportFormat::Jsonl => (
            std::fs::read_to_string(&path)?,
            "application/x-ndjson; charset=utf-8",
        ),
    };
    let filename = format!("conversation-{}.{}", id, ext);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        Body::from(body),
    )
        .into_response())
}

fn session_id_of(c: &Conversation) -> Option<String> {
    c.path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

fn resolve_path(state: &AppState, id: &str) -> WebResult<PathBuf> {
    let convs = state.conversations.read().expect("conversations lock");
    convs
        .iter()
        .find(|c| session_id_of(c).as_deref() == Some(id))
        .map(|c| c.path.clone())
        .ok_or(WebError::NotFound)
}
