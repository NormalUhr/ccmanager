//! Conversation export functionality.
//!
//! This module provides functions to export conversations in different formats:
//! - Ledger format (formatted text with speaker names)
//! - Plain text (simple speaker: message format)
//! - Markdown (with headers for speakers)
//! - JSONL (raw format)
//!
//! Conversations are copied to the system clipboard.

use crate::claude::{self, AgentContent, ContentBlock, LogEntry, UserContent, UserMessage};
use crate::tool_format;
use std::fs::{self, File};
#[cfg(target_os = "linux")]
use std::io::Write as _;
use std::io::{BufRead, BufReader};
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};

/// Export format options
#[derive(Clone, Copy, Debug)]
pub enum ExportFormat {
    Ledger,
    Plain,
    Markdown,
    Jsonl,
}

impl ExportFormat {
    /// Get format from menu option index (0-3)
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(ExportFormat::Ledger),
            1 => Some(ExportFormat::Plain),
            2 => Some(ExportFormat::Markdown),
            3 => Some(ExportFormat::Jsonl),
            _ => None,
        }
    }
}

/// Result of an export operation
pub struct ExportResult {
    pub message: String,
}

/// Options for export content generation
#[derive(Clone, Copy, Debug, Default)]
pub struct ExportOptions {
    pub show_tools: bool,
    pub show_thinking: bool,
}

/// Copy text to the system clipboard.
///
/// On Linux, selects clipboard tools based on the display server: `wl-copy`
/// for Wayland, `xclip`/`xsel` for X11. These persist clipboard data
/// independently of the calling process (unlike arboard, which loses
/// contents when the process exits). Falls back to arboard if no external
/// tool is available.
pub fn copy_to_system_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let candidates = linux_clipboard_candidates();
        for (cmd, args) in &candidates {
            match copy_via_command(cmd, args, text) {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(_)) => continue, // command found but failed, try next
                Err(()) => continue,    // command not found, try next
            }
        }
        // Fall through to arboard
    }

    // arboard fallback (primary method on macOS/Windows)
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => clipboard
            .set_text(text)
            .map_err(|e| format!("Clipboard error: {}", e)),
        Err(e) => Err(format!("Clipboard unavailable: {}", e)),
    }
}

/// Return clipboard tool candidates based on the active display server.
#[cfg(target_os = "linux")]
fn linux_clipboard_candidates() -> Vec<(&'static str, &'static [&'static str])> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();

    let mut candidates = Vec::new();
    if wayland {
        candidates.push(("wl-copy", ["--type", "text/plain;charset=utf-8"].as_slice()));
    }
    if x11 {
        candidates.push(("xclip", ["-selection", "clipboard"].as_slice()));
        candidates.push(("xsel", ["--clipboard", "--input"].as_slice()));
    }
    candidates
}

/// Try to copy text via an external command (e.g. wl-copy, xclip, xsel).
/// Returns `Ok(Ok(()))` on success, `Ok(Err(msg))` if the command ran but failed,
/// or `Err(())` if the command was not found (caller should try next option).
#[cfg(target_os = "linux")]
fn copy_via_command(cmd: &str, args: &[&str], text: &str) -> Result<Result<(), String>, ()> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ())?; // command not available → try next

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }

    match child.wait() {
        Ok(status) if status.success() => Ok(Ok(())),
        Ok(status) => Ok(Err(format!("{} exited with {}", cmd, status))),
        Err(e) => Ok(Err(format!("{} error: {}", cmd, e))),
    }
}

/// Copy conversation to the system clipboard.
///
/// For Plain/Markdown/Ledger formats, emits only the user prompt and the
/// final assistant answer per round; tool calls, tool results, thinking
/// blocks, intermediate narration, and subagent entries are dropped. JSONL
/// is the raw transcript.
pub fn export_to_clipboard(source_path: &Path, format: ExportFormat) -> ExportResult {
    let content = match generate_content(source_path, format) {
        Ok(c) => c,
        Err(e) => {
            return ExportResult {
                message: format!("Failed to read: {}", e),
            };
        }
    };

    match copy_to_system_clipboard(&content) {
        Ok(()) => ExportResult {
            message: "Copied to clipboard".to_string(),
        },
        Err(e) => ExportResult { message: e },
    }
}

/// Extract the text content of a single message by its entry index in the JSONL file.
/// Returns the message text suitable for clipboard copying.
pub fn extract_message_text(
    source_path: &Path,
    entry_index: usize,
    options: ExportOptions,
) -> Result<String, String> {
    let file = File::open(source_path).map_err(|e| format!("Failed to read: {}", e))?;
    let reader = BufReader::new(file);
    let mut current_index: usize = 0;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<LogEntry>(&line) else {
            continue;
        };

        if current_index == entry_index {
            return Ok(format_entry_for_clipboard(&entry, options));
        }
        current_index += 1;
    }

    Err("Message not found".to_string())
}

/// Format a single log entry as text for clipboard
fn format_entry_for_clipboard(entry: &LogEntry, options: ExportOptions) -> String {
    let mut output = String::new();
    match entry {
        LogEntry::User {
            message,
            parent_tool_use_id,
            ..
        } => {
            if let Some(text) = extract_user_text(message) {
                output.push_str(&text);
            }
            if options.show_tools
                && let UserContent::Blocks(blocks) = &message.content
            {
                for block in blocks {
                    if let ContentBlock::ToolResult { content, .. } = block {
                        let content_str = format_tool_result_for_export(content.as_ref());
                        if !output.is_empty() {
                            output.push_str("\n\n");
                        }
                        output.push_str(&content_str);
                    }
                }
            }
            let _ = parent_tool_use_id;
        }
        LogEntry::Assistant {
            message,
            parent_tool_use_id,
            ..
        } => {
            for block in &message.content {
                match block {
                    ContentBlock::Text { text } => {
                        if !output.is_empty() {
                            output.push_str("\n\n");
                        }
                        output.push_str(text);
                    }
                    ContentBlock::ToolUse { name, input, .. } if options.show_tools => {
                        if !output.is_empty() {
                            output.push_str("\n\n");
                        }
                        let formatted = format_tool_call_for_export(name, input);
                        output.push_str(&formatted);
                    }
                    ContentBlock::Thinking { thinking, .. } if options.show_thinking => {
                        if !output.is_empty() {
                            output.push_str("\n\n");
                        }
                        output.push_str(thinking);
                    }
                    _ => {}
                }
            }
            let _ = parent_tool_use_id;
        }
        LogEntry::Progress { data, .. } => {
            if let Some(agent_progress) = claude::parse_agent_progress(data) {
                let AgentContent::Blocks(blocks) = &agent_progress.message.message.content;
                for block in blocks {
                    match block {
                        ContentBlock::Text { text } => {
                            if !output.is_empty() {
                                output.push_str("\n\n");
                            }
                            output.push_str(text);
                        }
                        ContentBlock::ToolUse { name, input, .. } if options.show_tools => {
                            if !output.is_empty() {
                                output.push_str("\n\n");
                            }
                            output.push_str(&format_tool_call_for_export(name, input));
                        }
                        ContentBlock::ToolResult { content, .. } if options.show_tools => {
                            if !output.is_empty() {
                                output.push_str("\n\n");
                            }
                            output.push_str(&format_tool_result_for_export(content.as_ref()));
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
    output
}

/// Generate content in the specified format.
///
/// For readable formats (Plain/Markdown/Ledger), only the user prompt and the
/// final assistant answer per round are emitted — intermediate narration, tool
/// calls, tool results, thinking blocks, and subagent entries are filtered
/// out. JSONL is returned raw.
fn generate_content(source_path: &Path, format: ExportFormat) -> std::io::Result<String> {
    match format {
        ExportFormat::Jsonl => fs::read_to_string(source_path),
        ExportFormat::Plain => generate_plain(source_path),
        ExportFormat::Markdown => generate_markdown(source_path),
        ExportFormat::Ledger => generate_ledger(source_path),
    }
}

/// One conversational round: a top-level user prompt plus Claude's final
/// (non-intermediate) answer. Rounds are the unit of filtered export.
struct Round {
    user_text: String,
    /// Last non-empty top-level assistant `Text` block seen in this round.
    /// None if the round has no assistant reply yet (last round in progress).
    final_claude_text: Option<String>,
}

/// Stream a JSONL conversation and group entries into rounds, keeping only the
/// user prompt and the final assistant answer per round. Subagent entries
/// (those with `parent_tool_use_id`), tool uses, tool results, thinking blocks,
/// and intermediate assistant narration are all dropped.
///
/// A new round starts on each top-level `User` entry whose content yields
/// prompt text (bare tool-result user entries do not start a round). Within a
/// round, each top-level `Assistant` entry's non-empty `Text` blocks overwrite
/// `final_claude_text`, so the last one wins.
fn collect_rounds(path: &Path) -> std::io::Result<Vec<Round>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut rounds: Vec<Round> = Vec::new();
    let mut current: Option<Round> = None;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<LogEntry>(&line) else {
            continue;
        };
        match entry {
            LogEntry::User {
                message,
                parent_tool_use_id,
                ..
            } => {
                if parent_tool_use_id.is_some() {
                    continue;
                }
                if let Some(text) = extract_user_text(&message) {
                    if let Some(r) = current.take() {
                        rounds.push(r);
                    }
                    current = Some(Round {
                        user_text: text,
                        final_claude_text: None,
                    });
                }
            }
            LogEntry::Assistant {
                message,
                parent_tool_use_id,
                ..
            } => {
                if parent_tool_use_id.is_some() {
                    continue;
                }
                let Some(round) = current.as_mut() else {
                    continue;
                };
                for block in &message.content {
                    if let ContentBlock::Text { text } = block
                        && !text.trim().is_empty()
                    {
                        round.final_claude_text = Some(text.clone());
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(r) = current.take() {
        rounds.push(r);
    }
    Ok(rounds)
}

/// Generate plain text format (simple "Speaker: message" lines).
pub fn generate_plain(path: &Path) -> std::io::Result<String> {
    let mut output = String::new();
    for round in collect_rounds(path)? {
        output.push_str(&format!("You: {}\n\n", round.user_text));
        if let Some(answer) = round.final_claude_text {
            output.push_str(&format!("Claude: {}\n\n", answer));
        }
    }
    Ok(output)
}

/// Generate markdown format (with `## You` / `## Claude` headers).
pub fn generate_markdown(path: &Path) -> std::io::Result<String> {
    let mut output = String::new();
    for round in collect_rounds(path)? {
        output.push_str(&format!("## You\n\n{}\n\n", round.user_text));
        if let Some(answer) = round.final_claude_text {
            output.push_str(&format!("## Claude\n\n{}\n\n", answer));
        }
    }
    Ok(output)
}

/// Total line width for ledger export (including name column and separator)
const LEDGER_WIDTH: usize = 90;

/// Generate ledger-style format (formatted like the TUI viewer).
pub fn generate_ledger(path: &Path) -> std::io::Result<String> {
    const NAME_WIDTH: usize = 9;
    let content_width = LEDGER_WIDTH - NAME_WIDTH - 3;

    let mut output = String::new();
    for round in collect_rounds(path)? {
        let wrapped_user = wrap_plain_text(&round.user_text, content_width);
        append_ledger_block(&mut output, "You", &wrapped_user, NAME_WIDTH);
        output.push('\n');
        if let Some(answer) = round.final_claude_text {
            let rendered = crate::markdown::render_markdown_plain(&answer, content_width);
            let rendered = rendered.trim_end();
            append_ledger_block(&mut output, "Claude", rendered, NAME_WIDTH);
            output.push('\n');
        }
    }
    Ok(output)
}

/// Append a ledger-formatted block to the output
fn append_ledger_block(output: &mut String, speaker: &str, text: &str, name_width: usize) {
    for (i, line) in text.lines().enumerate() {
        if i == 0 {
            output.push_str(&format!(
                "{:>width$} │ {}\n",
                speaker,
                line,
                width = name_width
            ));
        } else {
            output.push_str(&format!("{:>width$} │ {}\n", "", line, width = name_width));
        }
    }
}

/// Extract text from a user message, handling command messages
fn extract_user_text(message: &UserMessage) -> Option<String> {
    match &message.content {
        UserContent::String(s) => process_command_text(s),
        UserContent::Blocks(blocks) => {
            for block in blocks {
                if let ContentBlock::Text { text } = block
                    && let Some(processed) = process_command_text(text)
                {
                    return Some(processed);
                }
            }
            None
        }
    }
}

/// Process command message text, extracting content from XML tags
fn process_command_text(text: &str) -> Option<String> {
    let trimmed = text.trim();

    // Handle <local-command-stdout> tags
    if trimmed.starts_with("<local-command-stdout>") && trimmed.ends_with("</local-command-stdout>")
    {
        let inner = &trimmed
            ["<local-command-stdout>".len()..trimmed.len() - "</local-command-stdout>".len()];
        if inner.trim().is_empty() {
            return None;
        }
        return Some(inner.trim().to_string());
    }

    // Handle <command-name> tags
    if let Some(start) = trimmed.find("<command-name>")
        && let Some(end) = trimmed.find("</command-name>")
    {
        let content_start = start + "<command-name>".len();
        if content_start < end {
            let command_name = &trimmed[content_start..end];

            // Also extract command args if present
            if let Some(args_start) = trimmed.find("<command-args>")
                && let Some(args_end) = trimmed.find("</command-args>")
            {
                let args_content_start = args_start + "<command-args>".len();
                if args_content_start < args_end {
                    let args = trimmed[args_content_start..args_end].trim();
                    if !args.is_empty() {
                        return Some(format!("{} {}", command_name, args));
                    }
                }
            }

            return Some(command_name.to_string());
        }
    }

    Some(text.to_string())
}

/// Default width for non-ledger export (no wrapping needed for markdown export)
const EXPORT_WIDTH: usize = usize::MAX;

/// Format a tool call for export (used by single-message clipboard copy)
fn format_tool_call_for_export(name: &str, input: &serde_json::Value) -> String {
    let formatted = tool_format::format_tool_call(name, input, EXPORT_WIDTH);
    match formatted.body {
        Some(body) => format!("{}\n{}", formatted.header, body),
        None => formatted.header,
    }
}

/// Wrap plain text to max_width, preserving existing line breaks
fn wrap_plain_text(text: &str, max_width: usize) -> String {
    let mut result = String::new();
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        if line.is_empty() {
            continue;
        }
        let wrapped: Vec<_> = textwrap::wrap(line, max_width)
            .into_iter()
            .map(|cow| cow.into_owned())
            .collect();
        for (j, w) in wrapped.iter().enumerate() {
            if j > 0 {
                result.push('\n');
            }
            result.push_str(w);
        }
    }
    result
}

/// Format tool result content for export
fn format_tool_result_for_export(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => {
            // Handle array of content blocks
            let texts: Vec<&str> = arr
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                .collect();
            if !texts.is_empty() {
                texts.join("\n\n")
            } else {
                serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "<error>".to_string())
            }
        }
        Some(value) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "<error>".to_string())
        }
        None => "<no content>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_plain_text_preserves_short_lines() {
        let result = wrap_plain_text("short line", 80);
        assert_eq!(result, "short line");
    }

    #[test]
    fn test_wrap_plain_text_wraps_long_line() {
        let long = "word ".repeat(20); // 100 chars
        let result = wrap_plain_text(long.trim(), 40);
        for line in result.lines() {
            assert!(line.len() <= 40, "Line exceeds max_width: {:?}", line);
        }
        // All words should be preserved
        assert_eq!(result.matches("word").count(), 20);
    }

    #[test]
    fn test_wrap_plain_text_preserves_existing_newlines() {
        let text = "line one\nline two\nline three";
        let result = wrap_plain_text(text, 80);
        assert_eq!(result.lines().count(), 3);
    }

    #[test]
    fn test_wrap_plain_text_preserves_empty_lines() {
        let text = "line one\n\nline three";
        let result = wrap_plain_text(text, 80);
        assert_eq!(result, "line one\n\nline three");
    }

    #[test]
    fn test_append_ledger_block_format() {
        let mut output = String::new();
        append_ledger_block(&mut output, "Claude", "Hello\nWorld", 9);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("   Claude │ Hello"));
        assert!(lines[1].starts_with("          │ World"));
    }

    #[test]
    fn test_ledger_line_width() {
        // Verify that a wrapped line fits within LEDGER_WIDTH
        let name_width = 9;
        let content_width = LEDGER_WIDTH - name_width - 3;
        let long_text = "word ".repeat(20);
        let wrapped = wrap_plain_text(long_text.trim(), content_width);
        let mut output = String::new();
        append_ledger_block(&mut output, "Claude", &wrapped, name_width);
        for line in output.lines() {
            // Count display width (name + " │ " + content)
            let width = line.chars().count();
            assert!(
                width <= LEDGER_WIDTH,
                "Ledger line exceeds {} chars (got {}): {:?}",
                LEDGER_WIDTH,
                width,
                line
            );
        }
    }

    #[test]
    fn test_ledger_markdown_rendering() {
        // Verify that markdown is rendered (not raw) in ledger export
        let content_width = LEDGER_WIDTH - 9 - 3;
        let rendered =
            crate::markdown::render_markdown_plain("This has **bold** and `code`", content_width);
        // Should not contain markdown formatting markers for bold
        assert!(
            !rendered.contains("**"),
            "Should strip bold markers: {:?}",
            rendered
        );
        // Should contain backticks for inline code
        assert!(
            rendered.contains("`code`"),
            "Should keep inline code backticks: {:?}",
            rendered
        );
        // Should not contain ANSI codes
        assert!(
            !rendered.contains("\x1b"),
            "Should not contain ANSI codes: {:?}",
            rendered
        );
    }

    /// Build a JSONL fixture from an array of entry JSON values and return the path.
    /// Caller is responsible for removing the file.
    fn write_jsonl_fixture(name: &str, entries: &[serde_json::Value]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        let body: String = entries
            .iter()
            .map(|e| format!("{}\n", e))
            .collect::<Vec<_>>()
            .concat();
        std::fs::write(&path, body).unwrap();
        path
    }

    fn user_entry(text: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": text,
            },
            "timestamp": "2024-01-01T00:00:00Z"
        })
    }

    fn assistant_text_entry(text: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "m",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": text}],
                "model": "test",
                "stop_reason": "end_turn",
                "stop_sequence": null,
                "usage": {"input_tokens": 0, "output_tokens": 0}
            },
            "timestamp": "2024-01-01T00:00:00Z"
        })
    }

    fn assistant_tool_use_entry(name: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "m",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "tu_1",
                    "name": name,
                    "input": {"command": "echo hi"}
                }],
                "model": "test",
                "stop_reason": "tool_use",
                "stop_sequence": null,
                "usage": {"input_tokens": 0, "output_tokens": 0}
            },
            "timestamp": "2024-01-01T00:00:00Z"
        })
    }

    fn user_tool_result_entry() -> serde_json::Value {
        serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "tu_1",
                    "content": "hi\n"
                }]
            },
            "timestamp": "2024-01-01T00:00:00Z"
        })
    }

    #[test]
    fn test_generate_ledger_wraps_and_renders() {
        let long_text = "This is a **really long** sentence that should definitely wrap because it contains many words and exceeds the content width of the ledger format which is 68 characters.";
        let path = write_jsonl_fixture(
            "ccmanager-test-ledger.jsonl",
            &[user_entry("hi"), assistant_text_entry(long_text)],
        );
        let result = generate_ledger(&path).unwrap();
        std::fs::remove_file(&path).ok();

        for line in result.lines() {
            if line.is_empty() {
                continue;
            }
            assert!(
                line.chars().count() <= LEDGER_WIDTH,
                "Ledger line exceeds {} chars: {:?}",
                LEDGER_WIDTH,
                line
            );
        }
        assert!(result.contains("You"), "should include user speaker");
        assert!(result.contains("Claude"), "should include claude speaker");
        assert!(!result.contains("\x1b"), "no ANSI codes");
        assert!(!result.contains("**"), "bold markers stripped");
    }

    #[test]
    fn test_filter_keeps_only_final_claude_answer_per_round() {
        // A round with: user prompt, intermediate narration, a tool use, a
        // tool result, more narration, then the final answer. Another round
        // follows. The filtered export should contain only the two user
        // prompts and the two final answers — no intermediate text, no tool
        // content.
        let path = write_jsonl_fixture(
            "ccmanager-test-filter.jsonl",
            &[
                user_entry("first question"),
                assistant_text_entry("Let me check that file for you."),
                assistant_tool_use_entry("Read"),
                user_tool_result_entry(),
                assistant_text_entry("Looking at the results now..."),
                assistant_text_entry("The final answer to question one."),
                user_entry("second question"),
                assistant_text_entry("The final answer to question two."),
            ],
        );

        let md = generate_markdown(&path).unwrap();
        let plain = generate_plain(&path).unwrap();
        let ledger = generate_ledger(&path).unwrap();
        std::fs::remove_file(&path).ok();

        for (label, out) in [("markdown", &md), ("plain", &plain), ("ledger", &ledger)] {
            assert!(
                out.contains("first question") && out.contains("second question"),
                "{label} should keep user prompts, got:\n{out}"
            );
            assert!(
                out.contains("The final answer to question one.")
                    && out.contains("The final answer to question two."),
                "{label} should keep final answers, got:\n{out}"
            );
            assert!(
                !out.contains("Let me check"),
                "{label} should drop intermediate narration, got:\n{out}"
            );
            assert!(
                !out.contains("Looking at the results"),
                "{label} should drop intermediate narration, got:\n{out}"
            );
            assert!(
                !out.contains("tool_use")
                    && !out.contains("Tool Result")
                    && !out.contains("tool_result"),
                "{label} should drop tool output, got:\n{out}"
            );
        }

        // Markdown has the exact structure we expect
        assert!(md.contains("## You\n\nfirst question"));
        assert!(md.contains("## Claude\n\nThe final answer to question one."));
        assert!(md.contains("## You\n\nsecond question"));
        assert!(md.contains("## Claude\n\nThe final answer to question two."));
    }

    #[test]
    fn test_filter_drops_subagent_entries() {
        // A subagent (Task tool) produces entries with parent_tool_use_id set.
        // These should be dropped wholesale — they're intermediate steps.
        let path = std::env::temp_dir().join("ccmanager-test-subagent.jsonl");
        let subagent_assistant = serde_json::json!({
            "type": "assistant",
            "parentUuid": "p",
            "parent_tool_use_id": "tu_parent",
            "message": {
                "id": "m",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "I am a subagent talking"}],
                "model": "test",
                "stop_reason": "end_turn",
                "stop_sequence": null,
                "usage": {"input_tokens": 0, "output_tokens": 0}
            },
            "timestamp": "2024-01-01T00:00:00Z"
        });
        let body = format!(
            "{}\n{}\n{}\n",
            user_entry("q"),
            subagent_assistant,
            assistant_text_entry("final")
        );
        std::fs::write(&path, body).unwrap();

        let md = generate_markdown(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert!(
            !md.contains("subagent talking"),
            "subagent text must be dropped:\n{md}"
        );
        assert!(md.contains("final"), "final answer must be kept:\n{md}");
    }
}
