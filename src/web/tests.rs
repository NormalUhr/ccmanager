//! Integration tests for the web server.
//!
//! These spin up a fresh `AppState` backed by a tempdir of fixture JSONL
//! files (mimicking `~/.claude/projects/<encoded-dir>/<session>.jsonl`), then
//! drive the axum `Router` directly with `tower::ServiceExt::oneshot`. No
//! network, no port binding — fast and hermetic.

#![cfg(test)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::ServeConfig;
use super::state::AppState;

// ---------- fixtures ----------

/// Build a fixture directory, point CLAUDE_CONFIG_DIR at it, return its path.
/// The caller must keep the returned TempDir alive for the test duration.
fn fixture_dir() -> (tempfile::TempDir, AppState) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let projects = tmp.path().join("projects");
    let proj_dir = projects.join("-fixtures-proj");
    std::fs::create_dir_all(&proj_dir).unwrap();

    // Two sessions with distinct prose so search can disambiguate.
    write_session(
        &proj_dir,
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "question about orange fruit",
        "oranges are round and citrus",
    );
    write_session(
        &proj_dir,
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "question about banana fruit",
        "bananas are yellow and curved",
    );

    // Point the history loader at our fixture.
    unsafe {
        std::env::set_var("CLAUDE_CONFIG_DIR", tmp.path());
    }

    let state = AppState::load(ServeConfig {
        host: "127.0.0.1".into(),
        port: 0,
        token: None,
        open: false,
        read_only: false,
    })
    .expect("load");

    (tmp, state)
}

fn write_session(dir: &std::path::Path, session_id: &str, user: &str, claude: &str) {
    let user_line = format!(
        r#"{{"type":"user","message":{{"role":"user","content":"{}"}},"timestamp":"2024-01-01T00:00:00Z"}}"#,
        user.replace('"', "\\\"")
    );
    let claude_line = format!(
        r#"{{"type":"assistant","message":{{"id":"m","type":"message","role":"assistant","content":[{{"type":"text","text":"{}"}}],"model":"test","stop_reason":"end_turn","stop_sequence":null,"usage":{{"input_tokens":0,"output_tokens":0}}}},"timestamp":"2024-01-01T00:00:01Z"}}"#,
        claude.replace('"', "\\\"")
    );
    let body = format!("{user_line}\n{claude_line}\n");
    let path = dir.join(format!("{}.jsonl", session_id));
    std::fs::write(path, body).unwrap();
}

fn build_app(state: AppState) -> axum::Router {
    use super::routes;
    use super::static_assets;
    axum::Router::new()
        .merge(routes::health::router())
        .merge(routes::list::router())
        .merge(routes::viewer::router())
        .merge(routes::mutations::router())
        .merge(static_assets::router())
        .with_state(state)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, String) {
    let res = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

// ---------- tests ----------

#[tokio::test]
async fn healthz_returns_ok() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let (status, body) = get(&app, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.trim(), "ok");
}

#[tokio::test]
async fn root_redirects_to_conversations() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let res = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(loc, "/conversations");
}

#[tokio::test]
async fn list_page_renders_both_sessions() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let (status, body) = get(&app, "/conversations").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("aaaaaaaa"),
        "first session should appear: {}",
        &body[..200]
    );
    assert!(body.contains("bbbbbbbb"), "second session should appear");
}

#[tokio::test]
async fn search_filters_results() {
    // Exercises the fragment path used by htmx as the user types.
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/conversations?q=orange")
                .header("HX-Request", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("aaaaaaaa"), "orange session should appear");
    assert!(
        !body.contains("bbbbbbbb"),
        "banana session should be filtered out:\n{}",
        body
    );
}

#[tokio::test]
async fn search_empty_state_is_graceful() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/conversations?q=xxzzyyno")
                .header("HX-Request", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("empty-state") || body.contains("no conversations"));
}

#[tokio::test]
async fn viewer_404_for_unknown_session() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let (status, _body) = get(&app, "/conversations/deadbeef").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn viewer_renders_markdown_safely() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let (status, body) = get(&app, "/conversations/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").await;
    assert_eq!(status, StatusCode::OK);
    // User text and Claude response both present
    assert!(body.contains("orange fruit"));
    assert!(body.contains("citrus"));
    // No raw HTML injection slipped through (defense-in-depth)
    assert!(!body.contains("<script>alert"));
}

#[tokio::test]
async fn export_markdown_has_right_content_type() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/conversations/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/export.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.starts_with("text/markdown"));
    let disp = res
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(disp.contains("attachment"));
}

#[tokio::test]
async fn export_rejects_unknown_extension() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let (status, _body) = get(
        &app,
        "/conversations/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/export.docx",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rename_updates_disk_and_index() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state.clone());
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/conversations/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/rename")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=rustacean+special"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    // In-memory state updated
    let convs = state.conversations.read().unwrap();
    let conv = convs
        .iter()
        .find(|c| {
            c.path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("aaaaaaaa"))
        })
        .unwrap();
    assert_eq!(conv.custom_title.as_deref(), Some("rustacean special"));
}

#[tokio::test]
async fn rename_refused_in_read_only_mode() {
    let (_tmp, mut state) = fixture_dir();
    // Rebuild with read_only=true (simpler than a second loader call)
    state.cfg = std::sync::Arc::new(ServeConfig {
        read_only: true,
        ..(*state.cfg).clone()
    });
    let app = build_app(state);
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/conversations/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/rename")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("title=nope"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_requires_matching_confirmation() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state.clone());
    // Wrong confirm string
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/conversations/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/delete")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("confirm=not-matching"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // The session should still exist
    let convs = state.conversations.read().unwrap();
    assert!(
        convs
            .iter()
            .any(|c| c.path.to_string_lossy().contains("aaaaaaaa"))
    );
}

// ---------- HX-Request detection + search state preservation ----------

#[tokio::test]
async fn list_full_page_includes_layout_chrome() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let (status, body) = get(&app, "/conversations").await;
    assert_eq!(status, StatusCode::OK);
    // Full page has the base layout (header, html doctype, nav chrome).
    assert!(
        body.starts_with("<!doctype html>"),
        "expected doctype:\n{}",
        &body[..80]
    );
    assert!(body.contains("<header"));
    assert!(body.contains("search-form"));
}

#[tokio::test]
async fn list_htmx_request_returns_fragment_only() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/conversations?q=orange")
                .header("HX-Request", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), 10 * 1024 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8_lossy(&bytes);
    // Fragment response: no layout chrome, no doctype, no <header>.
    assert!(
        !body.contains("<!doctype html>"),
        "fragment must not include doctype:\n{}",
        &body[..body.len().min(200)]
    );
    assert!(
        !body.contains("<header"),
        "fragment must not include layout header"
    );
    // But it does contain the filtered rows.
    assert!(body.contains("aaaaaaaa"), "orange session should appear");
    assert!(
        !body.contains("bbbbbbbb"),
        "banana session should be filtered out"
    );
}

#[tokio::test]
async fn list_preserves_search_in_input_value_on_refresh() {
    // Simulates browser refresh / back-navigation to /conversations?q=foo:
    // server must pre-fill the search input so the user doesn't re-type.
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let (status, body) = get(&app, "/conversations?q=banana").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("value=\"banana\""),
        "search input must be pre-filled with q:\n{}",
        body.split('\n')
            .find(|l| l.contains("search-input"))
            .unwrap_or("(no input line)")
    );
    // And the filtered row is present, non-matches absent.
    assert!(body.contains("bbbbbbbb"));
    assert!(!body.contains("aaaaaaaa"));
}

#[tokio::test]
async fn search_form_points_at_canonical_url_with_replace_hint() {
    // Makes sure the search form wiring survives future template refactors.
    // Without these two attributes, `Cmd+[` back-navigation won't restore
    // the query.
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let (_status, body) = get(&app, "/conversations").await;
    assert!(
        body.contains("hx-get=\"/conversations\""),
        "search form must hx-get the canonical list URL"
    );
    assert!(
        body.contains("hx-replace-url=\"true\""),
        "search form must ask htmx to replace history URL"
    );
}

#[tokio::test]
async fn api_list_redirects_to_canonical_url() {
    let (_tmp, state) = fixture_dir();
    let app = build_app(state);
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/list")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(loc, "/conversations");
}

#[test]
fn serve_config_rejects_non_localhost_without_token() {
    let cfg = ServeConfig {
        host: "0.0.0.0".into(),
        port: 7878,
        token: None,
        open: false,
        read_only: false,
    };
    assert!(cfg.validate().is_err());

    let cfg_ok = ServeConfig {
        host: "0.0.0.0".into(),
        port: 7878,
        token: Some("sekret".into()),
        open: false,
        read_only: false,
    };
    assert!(cfg_ok.validate().is_ok());

    let cfg_localhost = ServeConfig {
        host: "127.0.0.1".into(),
        port: 7878,
        token: None,
        open: false,
        read_only: false,
    };
    assert!(cfg_localhost.validate().is_ok());
}
