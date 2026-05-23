//! View models passed from route handlers into askama templates.
//!
//! Askama templates reference fields on these structs; any rename here is
//! caught at compile time by the template's macro-expanded code.

use chrono::{DateTime, Local};

/// A single row in the conversation list.
pub struct ListRow {
    pub session_id: String,
    pub project_name: Option<String>,
    pub custom_title: Option<String>,
    pub model: Option<String>,
    pub message_count: usize,
    pub total_tokens: u64,
    pub timestamp: DateTime<Local>,
    pub age_short: String,
    pub preview: String,
    pub relative_url: String,
}

/// One rendered message inside a conversation.
#[allow(dead_code)]
pub struct WebMessage {
    pub idx: usize,
    pub kind: WebMessageKind,
    pub timestamp: Option<String>,
    pub subagent_depth: u8,
}

#[allow(dead_code)]
pub enum WebMessageKind {
    User {
        html: String,
    },
    Assistant {
        html: String,
    },
    ToolUse {
        name: String,
        summary: String,
        body_html: String,
    },
    ToolResult {
        content_html: String,
        truncated: bool,
    },
    Thinking {
        html: String,
    },
}

/// Viewer options echoed back to the template so toggle buttons stay in sync.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewOpts {
    pub tools: ToolMode,
    pub thinking: bool,
    pub timing: bool,
    pub questions_only: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolMode {
    #[default]
    Off,
    Truncated,
    Full,
}

impl ToolMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ToolMode::Off => "off",
            ToolMode::Truncated => "trn",
            ToolMode::Full => "full",
        }
    }

    pub fn from_query(s: &str) -> Self {
        match s {
            "trn" | "truncated" => ToolMode::Truncated,
            "full" => ToolMode::Full,
            _ => ToolMode::Off,
        }
    }
}
