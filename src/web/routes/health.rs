use axum::{Router, routing::get};

use crate::web::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/healthz", get(|| async { "ok" }))
}
