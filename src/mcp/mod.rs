//! MCP (Model Context Protocol) stdio server.
//!
//! Gated by the `mcp` Cargo feature (default-on). Entry point is
//! [`serve`] — invoked from `main.rs` when the user runs
//! `ccmanager mcp`. The server is intended to be spawned as a
//! subprocess by Claude Code (registered in `~/.claude.json`), not
//! invoked interactively.
//!
//! # Transport
//!
//! Line-delimited JSON-RPC 2.0 over stdin/stdout. Anything written to
//! stdout that isn't a valid JSON-RPC message corrupts the protocol —
//! every diagnostic goes to stderr.
//!
//! # Tools exposed
//!
//! - `search_history` — fuzzy search past conversations
//! - `get_session`    — fetch one conversation as markdown
//! - `list_recent_sessions` — recent sessions, newest-first
//!
//! The tool layer is pure functions of loaded state; the transport
//! layer just shuttles JSON between Claude Code and those handlers.

pub mod protocol;
pub mod server;
pub mod state;
pub mod tools;

#[cfg(test)]
mod tests;

use crate::error::{AppError, Result};

/// Entry point called from `main.rs` for the `Mcp` subcommand.
///
/// Loads every conversation via [`history::load_all_conversations`],
/// builds the searchable index, and runs the stdio JSON-RPC loop
/// until stdin closes or a fatal I/O error occurs.
pub fn serve() -> Result<()> {
    let state = state::McpState::load()?;
    eprintln!(
        "ccmanager mcp: loaded {} conversations",
        state.conversations.len()
    );

    // Build a small tokio runtime (we don't need multi-threaded because
    // stdio is inherently serial and every tool is a quick in-memory op).
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| AppError::ConfigError(format!("failed to start runtime: {}", e)))?;

    rt.block_on(server::run(state))
}
