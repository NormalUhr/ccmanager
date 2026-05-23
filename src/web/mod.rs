//! Local web UI for ccmanager.
//!
//! Bound behind the `serve` Cargo feature (default-on). The goal is to offer
//! the same list / viewer / search workflows as the TUI, but in a browser,
//! with server-rendered HTML + htmx for progressive enhancement.
//!
//! # Module layout
//!
//! - [`server`] — axum `Router` construction, binding, shutdown.
//! - [`state`]  — shared `AppState` holding conversations + search index.
//! - [`routes`] — HTTP handlers (list, viewer, export, mutations).
//! - [`render`] — JSONL → HTML conversion (markdown, syntax, tools).
//! - [`model`]  — template view-models.
//! - [`error`]  — `WebError` → HTTP status mapping.

pub mod error;
pub mod model;
pub mod render;
pub mod routes;
pub mod server;
pub mod state;
pub mod static_assets;

#[cfg(test)]
mod tests;

use crate::error::{AppError, Result};

/// Configuration for `ccmanager serve`, built from CLI args.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub host: String,
    pub port: u16,
    pub token: Option<String>,
    pub open: bool,
    pub read_only: bool,
}

impl ServeConfig {
    /// Refuse to start if non-localhost binding has no auth token.
    pub fn validate(&self) -> Result<()> {
        let is_localhost = matches!(self.host.as_str(), "127.0.0.1" | "localhost" | "::1");
        if !is_localhost && self.token.is_none() {
            return Err(AppError::ConfigError(format!(
                "binding to {} exposes your Claude history on the network. \
                 Pass --token <T> to require auth, or bind to 127.0.0.1.",
                self.host
            )));
        }
        Ok(())
    }
}

/// Entry point invoked by `main.rs` when the Serve subcommand is dispatched.
pub fn serve(cfg: ServeConfig) -> Result<()> {
    cfg.validate()?;
    server::run(cfg)
}
