//! Conversation viewer: `/conversations/{id}` + `/api/viewer/{id}` (htmx).

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::{Router, routing::get};
use serde::Deserialize;

use crate::history::Conversation;
use crate::web::error::{WebError, WebResult};
use crate::web::model::{ToolMode, ViewOpts, WebMessage, WebMessageKind};
use crate::web::render;
use crate::web::routes::list;
use crate::web::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/conversations/{id}", get(full_page))
        .route("/api/viewer/{id}", get(transcript_fragment))
}

#[derive(Deserialize, Default, Debug)]
pub struct ViewerQuery {
    #[serde(default)]
    pub tools: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub timing: Option<String>,
    #[serde(default)]
    pub questions_only: Option<String>,
}

fn parse_opts(q: &ViewerQuery) -> ViewOpts {
    fn as_bool(v: &Option<String>) -> bool {
        matches!(v.as_deref(), Some("1") | Some("on") | Some("true"))
    }
    ViewOpts {
        tools: q
            .tools
            .as_deref()
            .map(ToolMode::from_query)
            .unwrap_or(ToolMode::Off),
        thinking: as_bool(&q.thinking),
        timing: as_bool(&q.timing),
        questions_only: as_bool(&q.questions_only),
    }
}

#[derive(Template)]
#[template(path = "viewer.html")]
struct ViewerPage {
    header: ViewerHeader,
    opts: ViewOpts,
    messages: Vec<WebMessage>,
    read_only: bool,
    #[allow(dead_code)]
    transcript_url: String,
}

#[derive(Template)]
#[template(path = "viewer_transcript.html")]
struct ViewerTranscript {
    messages: Vec<WebMessage>,
    #[allow(dead_code)]
    opts: ViewOpts,
}

struct ViewerHeader {
    session_id: String,
    title: String,
    custom_title: Option<String>,
    project: Option<String>,
    model: Option<String>,
    message_count: usize,
    total_tokens: u64,
    /// Resume command that skips permissions — the fast / default path.
    resume_cmd_skip: String,
    /// Resume command without `--dangerously-skip-permissions` — for users
    /// who want Claude to ask before each action.
    resume_cmd_safe: String,
}

async fn full_page(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ViewerQuery>,
) -> WebResult<Response> {
    let opts = parse_opts(&q);
    let (header, messages) = load_and_render(&state, &id, &opts)?;
    let transcript_url = format!(
        "/api/viewer/{}?tools={}&thinking={}&timing={}&questions_only={}",
        urlencoding::encode(&id),
        opts.tools.as_str(),
        if opts.thinking { "1" } else { "0" },
        if opts.timing { "1" } else { "0" },
        if opts.questions_only { "1" } else { "0" },
    );
    list::render(&ViewerPage {
        header,
        opts,
        messages,
        read_only: state.cfg.read_only,
        transcript_url,
    })
}

async fn transcript_fragment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ViewerQuery>,
) -> WebResult<Response> {
    let opts = parse_opts(&q);
    let (_header, messages) = load_and_render(&state, &id, &opts)?;
    list::render(&ViewerTranscript { messages, opts })
}

fn load_and_render(
    state: &AppState,
    id: &str,
    opts: &ViewOpts,
) -> WebResult<(ViewerHeader, Vec<WebMessage>)> {
    let convs = state.conversations.read().expect("conversations lock");
    let conv = find_by_session(&convs, id).ok_or(WebError::NotFound)?;

    let messages = render::conversation::render_conversation(&conv.path, opts)?;
    let header = ViewerHeader {
        session_id: id.to_string(),
        title: conv
            .custom_title
            .clone()
            .or_else(|| conv.summary.clone())
            .unwrap_or_else(|| id.to_string()),
        custom_title: conv.custom_title.clone(),
        project: conv.project_name.clone(),
        model: conv.model.clone(),
        message_count: conv.message_count,
        total_tokens: conv.total_tokens,
        resume_cmd_skip: format!("claude --resume {} --dangerously-skip-permissions", id),
        resume_cmd_safe: format!("claude --resume {}", id),
    };
    Ok((header, messages))
}

fn find_by_session<'a>(convs: &'a [Conversation], id: &str) -> Option<&'a Conversation> {
    convs.iter().find(|c| {
        c.path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|sid| sid == id)
    })
}

// Template filters exposed to both viewer templates. Askama picks them up by
// module path; we keep them here so template + filter live together.
pub(crate) mod filters {
    use crate::web::model::{WebMessage, WebMessageKind};

    /// Pattern-match helper used as `{% if msg is_user(msg) %}` won't work in
    /// Askama 0.14, so templates call these instead via the `match` block.
    #[allow(dead_code)]
    pub fn kind_name(msg: &&WebMessage) -> ::askama::Result<&'static str> {
        Ok(match msg.kind {
            WebMessageKind::User { .. } => "user",
            WebMessageKind::Assistant { .. } => "claude",
            WebMessageKind::ToolUse { .. } => "tool",
            WebMessageKind::ToolResult { .. } => "tool-result",
            WebMessageKind::Thinking { .. } => "thinking",
        })
    }
}
