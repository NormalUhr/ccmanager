//! Shared application state for the web UI.
//!
//! At startup we load every conversation via [`history::load_all_conversations`]
//! and precompute a search index. Both are stored behind `RwLock` so the
//! mutation endpoints (rename, delete) can update them in place without a
//! full reload.

use crate::history::{self, Conversation};
use crate::tui::search::{self, SearchableConversation};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::ServeConfig;

#[derive(Clone)]
pub struct AppState {
    pub conversations: Arc<RwLock<Vec<Conversation>>>,
    pub searchable: Arc<RwLock<Vec<SearchableConversation>>>,
    #[allow(dead_code)]
    pub loaded_at: Arc<RwLock<Instant>>,
    pub cfg: Arc<ServeConfig>,
}

impl AppState {
    pub fn load(cfg: ServeConfig) -> crate::error::Result<Self> {
        // show_last=true so list previews match the TUI default.
        let mut conversations = history::load_all_conversations(true, None)?;
        // Sort by recency (newest first) — mirrors the TUI default ordering.
        conversations.sort_by_key(|c| std::cmp::Reverse(c.timestamp));
        // Re-index after the sort so entries line up with their new position.
        for (i, conv) in conversations.iter_mut().enumerate() {
            conv.index = i;
        }
        let searchable = search::precompute_search_text(&conversations);
        Ok(AppState {
            conversations: Arc::new(RwLock::new(conversations)),
            searchable: Arc::new(RwLock::new(searchable)),
            loaded_at: Arc::new(RwLock::new(Instant::now())),
            cfg: Arc::new(cfg),
        })
    }

    /// Rebuild the searchable index from the current conversations vector.
    /// Call after mutating `conversations` (rename, delete) so search stays
    /// consistent with what's in the list.
    pub fn rebuild_searchable(&self) {
        let convs = self.conversations.read().expect("conversations lock");
        let new_searchable = search::precompute_search_text(&convs);
        let mut guard = self.searchable.write().expect("searchable lock");
        *guard = new_searchable;
    }
}
