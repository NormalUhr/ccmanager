//! ccmanager library crate.
//!
//! The binary (src/main.rs) is a thin entry point; all functionality lives
//! here so it can be shared between the TUI and the web-ui serve mode.

pub mod claude;
pub mod cli;
pub mod config;
pub mod debug;
pub mod debug_log;
pub mod display;
pub mod error;
pub mod history;
pub mod launcher;
pub mod markdown;
pub mod pager;
pub mod syntax;
pub mod tool_format;
pub mod tui;
pub mod update;

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "serve")]
pub mod web;
