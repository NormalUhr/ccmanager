//! Shared state for the MCP server.
//!
//! Loads once at server startup and is read-only for the lifetime of the
//! process. No locks needed — MCP over stdio is inherently single-threaded
//! and we expose no mutation endpoints.

use crate::error::Result;
use crate::history::{self, Conversation};
use crate::tui::search::{self, SearchableConversation};

pub struct McpState {
    pub conversations: Vec<Conversation>,
    pub searchable: Vec<SearchableConversation>,
}

impl McpState {
    /// Load every conversation from `~/.claude/projects/` and build the
    /// searchable index. Equivalent to what the TUI does at startup, so
    /// search results rank identically across TUI, web UI, and MCP.
    pub fn load() -> Result<Self> {
        // show_last=true so previews match the TUI's default framing.
        let mut conversations = history::load_all_conversations(true, None)?;
        conversations.sort_by_key(|c| std::cmp::Reverse(c.timestamp));
        for (i, conv) in conversations.iter_mut().enumerate() {
            conv.index = i;
        }
        let searchable = search::precompute_search_text(&conversations);
        Ok(McpState {
            conversations,
            searchable,
        })
    }
}
