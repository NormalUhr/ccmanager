//! Static assets embedded into the binary at compile time.
//!
//! Files live in `web-static/` at the repo root. At build time `rust-embed`
//! inlines them; at serve time we look them up and respond with the right
//! Content-Type. No external CDN calls ever.

use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Router, routing::get};
use rust_embed::RustEmbed;

use crate::web::state::AppState;

#[derive(RustEmbed)]
#[folder = "web-static/"]
struct Assets;

pub fn router() -> Router<AppState> {
    Router::new().route("/static/{*path}", get(serve_asset))
}

async fn serve_asset(Path(path): Path<String>) -> Response {
    match Assets::get(&path) {
        Some(asset) => {
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            (
                [
                    (header::CONTENT_TYPE, mime),
                    (
                        header::CACHE_CONTROL,
                        "public, max-age=31536000, immutable".into(),
                    ),
                ],
                asset.data,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
