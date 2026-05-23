//! Axum server: construct the `Router`, bind the listener, block until shutdown.

use super::state::AppState;
use super::{ServeConfig, routes, static_assets};
use crate::error::{AppError, Result};
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

pub fn run(cfg: ServeConfig) -> Result<()> {
    let state = AppState::load(cfg.clone())?;

    let conv_count = state.conversations.read().expect("lock").len();
    eprintln!("ccmanager: loaded {} conversations", conv_count);

    // Build a tokio runtime ourselves rather than switching main() to async —
    // keeps the TUI path and startup cost clean for users who never use serve.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("ccmanager-web")
        .build()
        .map_err(|e| AppError::ConfigError(format!("failed to start runtime: {}", e)))?;

    rt.block_on(async_main(cfg, state))
}

async fn async_main(cfg: ServeConfig, state: AppState) -> Result<()> {
    let app = build_router(state.clone());

    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| AppError::ConfigError(format!("failed to bind {}: {}", addr, e)))?;

    let local_url = format!("http://{}:{}/", cfg.host, cfg.port);
    eprintln!("ccmanager: listening on {}", local_url);
    if cfg.read_only {
        eprintln!("ccmanager: read-only mode (rename/delete disabled)");
    }

    if cfg.open {
        // Best-effort browser open; don't fail serving on error.
        let url = local_url.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = webbrowser::open(&url) {
                eprintln!("ccmanager: couldn't auto-open browser: {}", e);
            }
        });
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| AppError::ConfigError(format!("server error: {}", e)))?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(routes::health::router())
        .merge(routes::list::router())
        .merge(routes::viewer::router())
        .merge(routes::mutations::router())
        .merge(static_assets::router())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut stream) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            stream.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    eprintln!("\nccmanager: shutting down");
}
