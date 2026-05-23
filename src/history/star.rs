//! Append a `star` marker entry to a conversation JSONL file.
//!
//! Schema mirrors the `custom-title` pattern used by `rename.rs`:
//! `{"type":"star","starred":<bool>,"sessionId":"<id>"}`. The parser
//! reads every such entry and the LAST one wins, so toggling is a
//! pure append — no file rewrite needed, no race with concurrent
//! Claude Code writes.

use crate::error::{AppError, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Append a `star` line to the given JSONL conversation file.
///
/// `starred = true` marks the conversation as starred; `starred = false`
/// un-stars it. Idempotent at the parser level: re-adding the same
/// state still resolves to that state (latest wins).
pub fn write_star_marker(path: &Path, session_id: &str, starred: bool) -> Result<()> {
    let entry = serde_json::json!({
        "type": "star",
        "starred": starred,
        "sessionId": session_id,
    });
    let mut line = serde_json::to_string(&entry)?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .append(true)
        .create(false)
        .open(path)
        .map_err(AppError::Io)?;
    file.write_all(line.as_bytes()).map_err(AppError::Io)?;
    file.flush().map_err(AppError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::process_conversation_file;
    use super::*;

    fn write_tmp(name: &str, body: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("{}-{}", name, std::process::id()));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn appends_a_well_formed_star_line() {
        let path = write_tmp(
            "ccmanager-star-basic.jsonl",
            r#"{"type":"summary","summary":"hi","leafUuid":"x"}
"#,
        );
        write_star_marker(&path, "abc-123", true).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        let last = content.lines().last().unwrap();
        let v: serde_json::Value = serde_json::from_str(last).unwrap();
        assert_eq!(v["type"], "star");
        assert_eq!(v["starred"], true);
        assert_eq!(v["sessionId"], "abc-123");
    }

    #[test]
    fn unstar_writes_starred_false() {
        let path = write_tmp("ccmanager-star-unstar.jsonl", "");
        write_star_marker(&path, "sid", true).unwrap();
        write_star_marker(&path, "sid", false).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "expected 2 appended lines, got {:?}", lines);
        let last: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(last["starred"], false);
    }

    #[test]
    fn parser_picks_up_the_appended_star() {
        // End-to-end: write a minimal JSONL, append a star marker, re-parse,
        // verify Conversation.starred reflects the marker.
        let body = r#"{"type":"user","message":{"role":"user","content":"hello"},"timestamp":"2024-01-01T00:00:00Z"}
{"type":"assistant","message":{"id":"m","type":"message","role":"assistant","content":[{"type":"text","text":"hi"}],"model":"test","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}},"timestamp":"2024-01-01T00:00:00Z"}
"#;
        let path = write_tmp("ccmanager-star-roundtrip.jsonl", body);
        write_star_marker(&path, "session-xyz", true).unwrap();

        let conv = process_conversation_file(path.clone(), None, None)
            .unwrap()
            .expect("parser should produce a conversation");
        std::fs::remove_file(&path).ok();
        assert!(conv.starred, "parser should see the appended star marker");
    }
}
