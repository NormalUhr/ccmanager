//! Append a `custom-title` entry to a conversation JSONL file.
//!
//! Uses the same on-disk format as Claude Code's `/rename` slash command:
//! `{"type":"custom-title","customTitle":"...","sessionId":"..."}`. The
//! parser already reads these entries and uses the last one, so appending
//! without rewriting the file is sufficient.

use crate::error::{AppError, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Append a `custom-title` line to the given JSONL conversation file.
///
/// `title` is trimmed by the caller; pass an empty string to clear the title
/// (matching parser semantics). The write uses `serde_json` to serialize the
/// fields so embedded quotes, backslashes, and control characters are safely
/// escaped.
pub fn write_custom_title(path: &Path, session_id: &str, title: &str) -> Result<()> {
    let entry = serde_json::json!({
        "type": "custom-title",
        "customTitle": title,
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
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn appends_a_well_formed_custom_title_line() {
        let path = write_tmp(
            "ccmanager-rename-basic.jsonl",
            "{\"type\":\"summary\",\"summary\":\"hi\",\"leafUuid\":\"x\"}\n",
        );
        write_custom_title(&path, "abc-123", "My renamed session").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        let last = content.lines().last().unwrap();
        let v: serde_json::Value = serde_json::from_str(last).unwrap();
        assert_eq!(v["type"], "custom-title");
        assert_eq!(v["customTitle"], "My renamed session");
        assert_eq!(v["sessionId"], "abc-123");
    }

    #[test]
    fn escapes_quotes_and_newlines_safely() {
        let path = write_tmp("ccmanager-rename-escape.jsonl", "");
        // A title that would break naive string interpolation
        let title = "has \"quotes\" and a\nnewline and backslash \\";
        write_custom_title(&path, "sid", title).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        // The file must still be exactly one line (no real newline in the payload)
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "must be a single JSONL line");
        let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(v["customTitle"], title);
    }

    #[test]
    fn parser_picks_up_the_appended_title() {
        // End-to-end: write a minimal JSONL, append a custom-title, re-parse,
        // and verify the Conversation.custom_title is set.
        let body = r#"{"type":"user","message":{"role":"user","content":"hello"},"timestamp":"2024-01-01T00:00:00Z"}
{"type":"assistant","message":{"id":"m","type":"message","role":"assistant","content":[{"type":"text","text":"hi"}],"model":"test","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}},"timestamp":"2024-01-01T00:00:00Z"}
"#;
        let path = write_tmp("ccmanager-rename-roundtrip.jsonl", body);
        write_custom_title(&path, "session-xyz", "Round trip title").unwrap();

        let conv = process_conversation_file(path.clone(), None, None)
            .unwrap()
            .expect("parser should produce a conversation");
        std::fs::remove_file(&path).ok();
        assert_eq!(conv.custom_title.as_deref(), Some("Round trip title"));
    }

    #[test]
    fn empty_title_clears_previous() {
        let body = r#"{"type":"custom-title","customTitle":"old title","sessionId":"s"}
{"type":"user","message":{"role":"user","content":"hi"},"timestamp":"2024-01-01T00:00:00Z"}
"#;
        let path = write_tmp("ccmanager-rename-clear.jsonl", body);
        write_custom_title(&path, "s", "").unwrap();

        let conv = process_conversation_file(path.clone(), None, None)
            .unwrap()
            .expect("parser should produce a conversation");
        std::fs::remove_file(&path).ok();
        // Parser treats empty/whitespace custom_title as "clear"
        assert!(
            conv.custom_title.is_none(),
            "empty title should clear previous, got {:?}",
            conv.custom_title
        );
    }
}
