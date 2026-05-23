//! JSONL conversation → Vec<WebMessage>. Filled in by task 6.

#![allow(dead_code)]

use super::super::model::{ViewOpts, WebMessage, WebMessageKind};
use crate::claude::{ContentBlock, LogEntry, UserContent};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Stream a JSONL conversation file into a sequence of rendered web messages.
pub fn render_conversation(path: &Path, opts: &ViewOpts) -> std::io::Result<Vec<WebMessage>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut out: Vec<WebMessage> = Vec::new();
    let mut entry_idx: usize = 0;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<LogEntry>(&line) else {
            entry_idx += 1;
            continue;
        };

        // Keep entry_idx stable for anchor URLs even when we skip the entry.
        let current_idx = entry_idx;
        entry_idx += 1;

        match entry {
            LogEntry::User {
                message,
                parent_tool_use_id,
                timestamp,
                ..
            } => {
                // Skip subagent turns unless thinking is shown.
                if parent_tool_use_id.is_some() && !opts.thinking {
                    continue;
                }
                // In questions-only mode drop non-top-level user entries.
                if opts.questions_only && parent_tool_use_id.is_some() {
                    continue;
                }

                let depth = if parent_tool_use_id.is_some() { 1 } else { 0 };
                if let Some(text) = extract_user_text(&message) {
                    out.push(WebMessage {
                        idx: current_idx,
                        kind: WebMessageKind::User {
                            html: super::markdown::render(&text),
                        },
                        timestamp: timestamp.clone(),
                        subagent_depth: depth,
                    });
                }
                // Tool-result blocks are never emitted in questions-only mode.
                if !opts.questions_only
                    && !matches!(opts.tools, super::super::model::ToolMode::Off)
                    && let UserContent::Blocks(blocks) = &message.content
                {
                    for block in blocks {
                        if let ContentBlock::ToolResult { content, .. } = block {
                            out.push(WebMessage {
                                idx: current_idx,
                                kind: WebMessageKind::ToolResult {
                                    content_html: super::tool::render_tool_result(content.as_ref()),
                                    truncated: matches!(
                                        opts.tools,
                                        super::super::model::ToolMode::Truncated
                                    ),
                                },
                                timestamp: timestamp.clone(),
                                subagent_depth: depth,
                            });
                        }
                    }
                }
            }
            LogEntry::Assistant {
                message,
                parent_tool_use_id,
                timestamp,
                ..
            } => {
                if parent_tool_use_id.is_some() && !opts.thinking {
                    continue;
                }
                if opts.questions_only {
                    continue;
                }
                let depth = if parent_tool_use_id.is_some() { 1 } else { 0 };

                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            out.push(WebMessage {
                                idx: current_idx,
                                kind: WebMessageKind::Assistant {
                                    html: super::markdown::render(text),
                                },
                                timestamp: timestamp.clone(),
                                subagent_depth: depth,
                            });
                        }
                        ContentBlock::ToolUse { name, input, .. }
                            if !matches!(opts.tools, super::super::model::ToolMode::Off) =>
                        {
                            out.push(WebMessage {
                                idx: current_idx,
                                kind: WebMessageKind::ToolUse {
                                    name: name.clone(),
                                    summary: String::new(),
                                    body_html: super::tool::render_tool_use(name, input),
                                },
                                timestamp: timestamp.clone(),
                                subagent_depth: depth,
                            });
                        }
                        ContentBlock::Thinking { thinking, .. } if opts.thinking => {
                            out.push(WebMessage {
                                idx: current_idx,
                                kind: WebMessageKind::Thinking {
                                    html: super::markdown::render(thinking),
                                },
                                timestamp: timestamp.clone(),
                                subagent_depth: depth,
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

fn extract_user_text(message: &crate::claude::UserMessage) -> Option<String> {
    match &message.content {
        UserContent::String(s) => {
            let t = s.trim();
            if t.is_empty() { None } else { Some(s.clone()) }
        }
        UserContent::Blocks(blocks) => {
            let texts: Vec<String> = blocks
                .iter()
                .filter_map(|block| {
                    if let ContentBlock::Text { text } = block {
                        let t = text.trim();
                        if t.is_empty() {
                            None
                        } else {
                            Some(text.clone())
                        }
                    } else {
                        None
                    }
                })
                .collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n\n"))
            }
        }
    }
}
