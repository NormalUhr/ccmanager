# TUI Framed Hybrid (v3) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-05-23-tui-framed-hybrid-design.md`

**Goal:** Bring back the outer rounded frame and 3-line list entries (with conversation preview drawn from the last user question), keep v1's search highlight + cool-blue palette + shared modal helper, and fix the parser bug where a folder rename mid-session keeps showing the old name.

**Architecture:** Two layers of change. (1) Data layer — add `Conversation.last_user_question`, populate it in the parser, and switch the parser's cwd extraction from "first user entry wins" to "latest user entry wins". (2) Render layer — `render_list_mode` and `render_view_mode` each wrap their screen in a single rounded `Block`, with the v1 `header_line()` / `footer_line()` output feeding `Block::title` / `Block::title_bottom`. `list_row_lines` rebuilds each entry as 3 styled `Line`s (header / preview / separator) with right-aligned metadata and a gutter-bar selection indicator. Compact mode collapses to 2 lines (no preview).

**Tech Stack:** Rust, `ratatui` 0.30, `crossterm` 0.29, no new dependencies.

**Workflow rule:** All commits land on `dev`. The final task snapshots to `main` and force-pushes. Use the **exact** commit messages each task specifies. Do NOT push `dev`. Do NOT push tags.

---

## File map

| File | What changes |
|---|---|
| `src/history/mod.rs` | `Conversation` struct: add `last_user_question: Option<String>` field. Update existing test fixtures elsewhere (a `Conversation { … }` literal in `src/tui/ui.rs`'s `list_row_tests` needs the new field added as `None` / `Some(text)`). |
| `src/history/parser.rs` | Populate `last_user_question` from `user_messages.last().cloned()`. Change cwd extraction at the existing line (`if extracted_cwd.is_none() && let Some(cwd_str) = cwd { … }`) to drop the `is_none()` guard so latest wins. Add two unit tests. |
| `src/tui/ui.rs` | Rewrite `list_row_lines` for 3-line entries + right-aligned metadata + gutter-bar selection + preview row from `last_user_question`. Rewrite `render_list_mode` to use one outer `Block` (no band layout, no horizontal rules). Rewrite `render_view_mode` to use one outer `Block` (find bar lives on the last content row inside the frame). Delete `horizontal_rule()`, `override_trailing_word()` (no longer used; `Block::title` accepts the styled `Line` directly so the suffix swap is done at the source instead). Update list-row tests for the new signature + new preview source. |

The four `render_modal()` callers and `viewer.rs` are untouched. `theme.rs` is untouched.

---

## Task 1: Add `last_user_question` field and populate it from the parser

**Files:**
- Modify: `src/history/mod.rs` — `Conversation` struct
- Modify: `src/history/parser.rs` — populate the field
- Modify: `src/tui/ui.rs` — `list_row_tests::make_conv` fixture (add the new field)
- Test: `src/history/parser.rs` — new unit test

- [ ] **Step 1: Add the field to `Conversation`.**

Open `src/history/mod.rs`. Find the `Conversation` struct (around line 46). After `pub duration_minutes: Option<u64>,` (the last field), add:

```rust
    /// Text of the most recent user-authored message. Used as the
    /// preview line in the TUI list ("start of the last question").
    /// `None` for conversations with zero user messages.
    pub last_user_question: Option<String>,
```

- [ ] **Step 2: Write the parser test (failing).**

Append to the existing `#[cfg(test)] mod tests` block at the bottom of `src/history/parser.rs`:

```rust
#[test]
fn extracts_the_last_user_question() {
    let path = std::env::temp_dir().join("ccmanager-last-user-q.jsonl");
    let body = format!(
        "{}\n{}\n{}\n",
        user_msg("hi", None),
        user_msg("what's up", None),
        user_msg("the last question wins", None),
    );
    std::fs::write(&path, body).unwrap();

    let conv = process_conversation_file(path.clone(), None, None)
        .unwrap()
        .expect("parser produced a conversation");
    std::fs::remove_file(&path).ok();

    assert_eq!(
        conv.last_user_question.as_deref(),
        Some("the last question wins"),
        "got: {:?}",
        conv.last_user_question
    );
}
```

(`user_msg` is an existing test helper in this file — see the existing tests around line 480-490 to confirm its signature is `fn user_msg(text: &str, cwd: Option<&str>) -> String`.)

- [ ] **Step 3: Run the test to verify it fails.**

```bash
cargo test --quiet extracts_the_last_user_question 2>&1 | tail -10
```

Expected: compile error — `last_user_question` doesn't exist on `Conversation` yet. (Or — once Step 1 is done — a runtime failure since the parser doesn't populate it.)

- [ ] **Step 4: Populate the field in the parser.**

In `src/history/parser.rs`, find the `Ok(Some(Conversation { … }))` block (around line 356) that constructs the returned `Conversation`. Add the new field:

```rust
    Ok(Some(Conversation {
        path,
        index: 0,
        timestamp,
        preview: preview_first.clone(),
        preview_first,
        preview_last,
        full_text,
        search_text_lower,
        // ... existing fields, exactly as-is ...
        duration_minutes,
        last_user_question: user_messages.last().cloned(),  // NEW
    }))
```

`user_messages` is a `Vec<String>` already populated during the parse (you can confirm at line ~65 / 136 in the file — every user message text gets pushed). `.last().cloned()` gives `None` for empty conversations or `Some(text)` for the most recent.

- [ ] **Step 5: Update the `list_row_tests::make_conv` fixture.**

In `src/tui/ui.rs`, find `mod list_row_tests` and inside it `fn make_conv(...)`. The fixture constructs a `Conversation { … }` literal listing every field. Add the new field at the end:

```rust
fn make_conv(project: &str, title: &str, mins_ago: i64, message_count: usize) -> Conversation {
    let ts = Local::now() - chrono::Duration::minutes(mins_ago);
    Conversation {
        // ... existing fields ...
        total_tokens: 0,
        duration_minutes: None,
        last_user_question: Some(format!("test question for {}", title)),  // NEW
    }
}
```

The fixture's preview row tests will use this in Task 3; for now just compile.

- [ ] **Step 6: Run the test to verify it passes.**

```bash
cargo build --quiet
cargo test --quiet extracts_the_last_user_question 2>&1 | tail -10
```

Expected: `1 passed`.

- [ ] **Step 7: Run the full suite.**

```bash
cargo fmt --all -- --check && echo "FMT CLEAN"
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean. (You may see fmt drift from the field addition — run `cargo fmt --all` and try again if so.)

- [ ] **Step 8: Commit on `dev`.**

```bash
git add src/history/mod.rs src/history/parser.rs src/tui/ui.rs
git commit -m "history(parser): expose Conversation.last_user_question"
```

---

## Task 2: Parser cwd extraction — latest wins, not first

**Files:**
- Modify: `src/history/parser.rs` — cwd extraction logic (around line 122) + new unit test

- [ ] **Step 1: Write the failing test.**

Append to the parser's `#[cfg(test)] mod tests` block:

```rust
#[test]
fn latest_cwd_wins_when_session_moves() {
    // Simulates renaming the project folder mid-session: the JSONL
    // has user entries with two different cwd values. Today's parser
    // latches on the first one; we want the latest one.
    let path = std::env::temp_dir().join("ccmanager-cwd-latest-wins.jsonl");
    let body = format!(
        "{}\n{}\n",
        user_msg("before rename", Some("/Users/me/Documents/Git/claude-history")),
        user_msg("after rename",  Some("/Users/me/Documents/Git/ccmanager")),
    );
    std::fs::write(&path, body).unwrap();

    let conv = process_conversation_file(path.clone(), None, None)
        .unwrap()
        .expect("parser produced a conversation");
    std::fs::remove_file(&path).ok();

    assert_eq!(
        conv.cwd,
        Some(std::path::PathBuf::from("/Users/me/Documents/Git/ccmanager")),
        "expected latest cwd to win; got {:?}",
        conv.cwd
    );
}
```

- [ ] **Step 2: Run the test, verify it fails.**

```bash
cargo test --quiet latest_cwd_wins_when_session_moves 2>&1 | tail -10
```

Expected: FAIL — `assertion `left == right` failed`, with `left = Some("…/claude-history")` (first-wins) and `right = Some("…/ccmanager")` (latest-wins, expected).

- [ ] **Step 3: Apply the parser fix.**

In `src/history/parser.rs`, find this block (around line 121-126):

```rust
                        // Extract cwd from the first user message that has it
                        if extracted_cwd.is_none()
                            && let Some(cwd_str) = cwd
                        {
                            extracted_cwd = Some(PathBuf::from(cwd_str));
                        }
```

Replace it with:

```rust
                        // Extract cwd from the latest user message that has
                        // one. Each entry's `cwd` reflects where `claude` was
                        // running at the time; the latest is the most
                        // current (e.g. after a mid-session folder rename).
                        if let Some(cwd_str) = cwd {
                            extracted_cwd = Some(PathBuf::from(cwd_str));
                        }
```

- [ ] **Step 4: Run the test, verify it passes.**

```bash
cargo test --quiet latest_cwd_wins_when_session_moves 2>&1 | tail -10
```

Expected: `1 passed`.

- [ ] **Step 5: Confirm other parser tests still pass.**

```bash
cargo test --quiet --package ccmanager --lib history::parser 2>&1 | tail -10
```

Expected: all parser tests pass. (Pay attention: any existing test that asserted on `conv.cwd` with a multi-entry fixture might have been relying on the first-wins behavior. If one fails, evaluate whether it was implicitly testing first-wins — if so, update the assertion; if it was a different concern, debug.)

- [ ] **Step 6: Full sweep.**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 7: Commit.**

```bash
git add src/history/parser.rs
git commit -m "history(parser): use latest cwd, not first — fixes rename-mid-session"
```

---

## Task 3: Rewrite `list_row_lines` for 3-line framed entries

**Files:**
- Modify: `src/tui/ui.rs` — `pub fn list_row_lines` (line 276) + tests

This is the substantive UI change. The new shape:

- Normal mode: 3 lines per entry — header (project + title + right-aligned `<N>msg · <age>`) / preview (`last_user_question`, truncated) / separator (thin `─` rule).
- Compact mode: 2 lines — header / separator. No preview row.
- Selection: gutter bar `▌` (accent color) on row 1, leftmost glyph column.

- [ ] **Step 1: Write the failing tests.**

Add four new test functions inside the existing `mod list_row_tests` in `src/tui/ui.rs`. (The existing 3 tests stay; we add 4 more.)

Before adding tests, update `make_conv` so its fixture sets a known `last_user_question`:

```rust
fn make_conv(project: &str, title: &str, mins_ago: i64, message_count: usize) -> Conversation {
    let ts = Local::now() - chrono::Duration::minutes(mins_ago);
    Conversation {
        path: std::path::PathBuf::from(format!("/fake/{}.jsonl", title)),
        index: 0,
        timestamp: ts,
        preview: String::new(),
        preview_first: String::new(),
        preview_last: String::new(),
        full_text: String::new(),
        search_text_lower: String::new(),
        project_name: Some(project.to_string()),
        project_path: None,
        cwd: None,
        message_count,
        parse_errors: Vec::new(),
        summary: Some(title.to_string()),
        custom_title: None,
        model: None,
        total_tokens: 0,
        duration_minutes: None,
        last_user_question: Some(format!("question text for {}", title)),  // important for the new tests
    }
}
```

Now the new tests. All existing test calls to `list_row_lines(...)` need to include the new `inner_width: u16` argument — add it (use `80` for the existing 3 tests; they pass `false` for `compact`).

```rust
#[test]
fn normal_mode_returns_three_lines() {
    let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
    let lines = list_row_lines(
        &Theme::dark(),
        &conv,
        /* selected = */ false,
        /* query    = */ "",
        /* compact  = */ false,
        /* inner_width = */ 80,
    );
    assert_eq!(lines.len(), 3, "expected header + preview + separator");
}

#[test]
fn compact_mode_returns_two_lines_no_preview() {
    let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
    let lines = list_row_lines(
        &Theme::dark(),
        &conv,
        /* selected = */ false,
        /* query    = */ "",
        /* compact  = */ true,
        /* inner_width = */ 80,
    );
    assert_eq!(lines.len(), 2, "expected header + separator only");
    // No preview row — the second line must be the thin rule, not the question text.
    let second: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        !second.contains("question text"),
        "compact mode must not include preview text, got: {:?}",
        second
    );
}

#[test]
fn preview_row_shows_last_user_question() {
    let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
    let lines = list_row_lines(
        &Theme::dark(),
        &conv,
        /* selected = */ false,
        /* query    = */ "",
        /* compact  = */ false,
        /* inner_width = */ 80,
    );
    let preview: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
    // The preview row should contain the fixture's question text. No "You:" prefix.
    assert!(
        preview.contains("question text for Add F5 refresh"),
        "preview line should contain the last user question, got: {:?}",
        preview
    );
    assert!(!preview.contains("You:"), "no You: prefix expected, got: {:?}", preview);
}

#[test]
fn metadata_is_right_aligned_with_msg_count_and_age() {
    let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
    let lines = list_row_lines(
        &Theme::dark(),
        &conv,
        /* selected = */ false,
        /* query    = */ "",
        /* compact  = */ false,
        /* inner_width = */ 80,
    );
    let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(header.contains("47msg"), "header missing msg count: {:?}", header);
    assert!(header.contains("·"), "header missing middle-dot separator");
    // Right-aligned: the title comes before the metadata, separated by padding.
    let title_pos = header.find("Add F5 refresh").expect("title");
    let msg_pos = header.find("47msg").expect("msg count");
    assert!(title_pos < msg_pos, "metadata should follow the title");
    // The full header should fit within inner_width (80 cols) — i.e., not exceed it.
    let display_width = header.chars().count();
    assert!(display_width <= 80, "header too wide ({}): {:?}", display_width, header);
}

#[test]
fn selected_row_has_gutter_bar() {
    let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
    let lines = list_row_lines(
        &Theme::dark(),
        &conv,
        /* selected = */ true,
        /* query    = */ "",
        /* compact  = */ false,
        /* inner_width = */ 80,
    );
    let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    let trimmed = header.trim_start();
    assert!(
        trimmed.starts_with("▌"),
        "selected row's header must start with the gutter bar ▌, got: {:?}",
        header
    );
}
```

Also update the THREE existing tests in this module:
- `unselected_row_has_two_content_lines_plus_blank` — rename/replace. Its old shape (3-line: line1 + line2 + blank) is gone. Either delete this test (the new `normal_mode_returns_three_lines` replaces its spirit) or rewrite to test the new layout. **Delete it** — it asserts the old design.
- `selected_row_starts_with_diamond` — now superseded by `selected_row_has_gutter_bar`. **Delete it**.
- `unselected_row_does_not_start_with_diamond` — superseded. **Delete it**.

Also delete the v1 test `compact_row_is_a_single_line` (since compact is now 2-line, not 1-line). The new `compact_mode_returns_two_lines_no_preview` is its v3 replacement.

So the net change in `list_row_tests`: drop the 4 v1 tests, add the 5 new ones. The module ends up with 5 tests.

- [ ] **Step 2: Run the tests to confirm compile failure.**

```bash
cargo test --quiet list_row_tests 2>&1 | tail -20
```

Expected: compile failure — `list_row_lines` doesn't have the new `inner_width: u16` parameter yet.

- [ ] **Step 3: Rewrite `list_row_lines`.**

Open `src/tui/ui.rs` at line 276. Replace the entire `list_row_lines` function body with this:

```rust
/// Render a single conversation entry inside the framed list.
///
/// Normal mode returns 3 styled lines: header (project + title +
/// right-aligned `<N>msg · <age>`), preview (the conversation's last
/// user question), separator (thin `─` rule). Compact mode returns 2
/// lines (header + separator, no preview).
///
/// `inner_width` is the column width inside the outer frame (i.e.
/// `inner_area.width`), used to right-align the metadata and to
/// truncate the preview/separator to fit.
///
/// Selection is indicated by a gutter bar `▌` in the header row's
/// leftmost glyph column; the bg-tint highlight is applied by the
/// caller via `List::highlight_style`.
pub fn list_row_lines(
    theme: &Theme,
    conv: &crate::history::Conversation,
    selected: bool,
    query: &str,
    compact: bool,
    inner_width: u16,
) -> Vec<Line<'static>> {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Span;

    // Styles
    let gutter_style = Style::default().fg(rgb(theme.accent));
    let project_style = Style::default().fg(rgb(theme.text_muted));
    let title_style = if selected {
        Style::default()
            .fg(rgb(theme.text_primary))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(rgb(theme.text_primary))
    };
    let dim = Style::default().fg(rgb(theme.text_muted));
    let separator_style = Style::default().fg(rgb(theme.separator));
    let accent_hl = Style::default()
        .fg(rgb(theme.accent))
        .add_modifier(Modifier::BOLD);

    // ── Row 1 — header ──────────────────────────────────────────
    let project = conv.project_name.as_deref().unwrap_or("?");
    let title = conv
        .custom_title
        .as_deref()
        .or(conv.summary.as_deref())
        .unwrap_or("(no title)");

    let (age, _) = format_timestamp(conv.timestamp, chrono::Local::now());
    let metadata = format!("{}msg · {}", conv.message_count, age);

    // Layout: "  ▌ project    title…    47msg · 2h ago "
    //         <┘ 2-col indent (or "  ◆" when selected → gutter bar)
    // Left segment widths:
    //   indent (2) + glyph (1) + gap (1) + project col (10) + gap (2) = 16
    // Right margin: 1 col before frame's right border.
    const LEFT_WIDTH: usize = 16; // indent + glyph + gap + project + gap
    const RIGHT_MARGIN: usize = 1;

    let glyph = if selected { "▌" } else { " " };

    // Build the highlighted title spans first (search-as-you-type accent).
    let title_spans = highlight_query(title, query, title_style, accent_hl);

    // Compute how many cols are left for the title between LEFT_WIDTH and
    // the right-aligned metadata.
    let inner = inner_width as usize;
    let meta_width = metadata.chars().count();
    let title_max = inner.saturating_sub(LEFT_WIDTH + meta_width + RIGHT_MARGIN + 1);
    let title_visible = title.chars().count().min(title_max);

    // Padding to push the metadata to the right edge.
    let used = LEFT_WIDTH + title_visible;
    let padding = inner
        .saturating_sub(used + meta_width + RIGHT_MARGIN)
        .max(1);

    let mut header_spans: Vec<Span<'static>> = vec![
        Span::styled("  ", project_style),
        Span::styled(glyph.to_string(), gutter_style),
        Span::styled(" ", project_style),
        Span::styled(format!("{:<10}", project), project_style),
        Span::styled("  ", project_style),
    ];
    // Title (truncated to title_max via the spans builder)
    header_spans.extend(truncate_spans(title_spans, title_max));
    header_spans.push(Span::styled(" ".repeat(padding), dim));
    header_spans.push(Span::styled(metadata.clone(), dim));
    let header_line = Line::from(header_spans);

    if compact {
        let sep_line = Line::from(vec![
            Span::styled("  ", separator_style),
            Span::styled(
                "─".repeat(inner.saturating_sub(4)),
                separator_style,
            ),
        ]);
        return vec![header_line, sep_line];
    }

    // ── Row 2 — preview ──────────────────────────────────────────
    let preview_indent = 4;
    let preview_max = inner.saturating_sub(preview_indent + RIGHT_MARGIN);
    let preview_text = conv
        .last_user_question
        .as_deref()
        .unwrap_or("");
    let preview_truncated = truncate_str(preview_text, preview_max);
    let preview_line = Line::from(vec![
        Span::styled("    ", dim),
        Span::styled(preview_truncated, dim),
    ]);

    // ── Row 3 — separator ───────────────────────────────────────
    let sep_line = Line::from(vec![
        Span::styled("  ", separator_style),
        Span::styled(
            "─".repeat(inner.saturating_sub(4)),
            separator_style,
        ),
    ]);

    vec![header_line, preview_line, sep_line]
}

/// Truncate a string to `max_chars` characters (NOT bytes). Appends `…`
/// if truncated. Returns an empty string when `max_chars == 0`.
fn truncate_str(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    // Reserve 1 char for the ellipsis.
    let keep = max_chars.saturating_sub(1);
    let mut out: String = s.chars().take(keep).collect();
    out.push('…');
    out
}

/// Truncate a sequence of styled spans to a total character width of
/// `max_chars`. Walks the spans left-to-right, taking chars from each,
/// and appends `…` if anything was dropped.
fn truncate_spans(spans: Vec<Span<'static>>, max_chars: usize) -> Vec<Span<'static>> {
    if max_chars == 0 {
        return vec![];
    }
    let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    if total <= max_chars {
        return spans;
    }
    // Reserve 1 char for the ellipsis.
    let keep = max_chars.saturating_sub(1);
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut taken = 0;
    for span in spans {
        let span_chars = span.content.chars().count();
        if taken + span_chars <= keep {
            out.push(span);
            taken += span_chars;
        } else {
            let remaining = keep.saturating_sub(taken);
            if remaining > 0 {
                let chunk: String = span.content.chars().take(remaining).collect();
                out.push(Span::styled(chunk, span.style));
            }
            break;
        }
    }
    // Append the ellipsis in the dim style (assume last styled span had a
    // reasonable style; fall back to default).
    let ellipsis_style = out
        .last()
        .map(|s| s.style)
        .unwrap_or_default();
    out.push(Span::styled("…", ellipsis_style));
    out
}
```

(`truncate_str` and `truncate_spans` are new helpers next to `list_row_lines` — keep them private to the module.)

- [ ] **Step 4: Update `render_list` to pass `inner_width`.**

In `src/tui/ui.rs`, find `fn render_list` (around line 1225). The call site looks like:

```rust
let lines = list_row_lines(theme, conv, Some(i) == selected_idx, app.query(), compact);
```

Change it to:

```rust
let lines = list_row_lines(theme, conv, Some(i) == selected_idx, app.query(), compact, area.width);
```

(`area` here is the rect passed into `render_list`, which is the *inside* of the outer block — that's what `inner_width` means.)

- [ ] **Step 5: Build and run the new list_row_tests.**

```bash
cargo build --quiet 2>&1 | tail -10
cargo test --quiet list_row_tests 2>&1 | tail -20
```

Expected: build clean; **5** tests pass (the 5 we added; the 4 v1 ones were deleted in Step 1).

If the build fails with "no field `last_user_question`" anywhere, you missed a `Conversation { … }` literal — search for the struct construction and add the field everywhere.

- [ ] **Step 6: Full sweep.**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 7: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): 3-line framed list rows with last-user-question preview"
```

---

## Task 4: Rewrite `render_list_mode` to wrap in a single outer `Block`

**Files:**
- Modify: `src/tui/ui.rs` — `fn render_list_mode` (around line 445)

The new shape: one rounded `Block` wraps the whole frame. Its top title carries the brand line (`header_line()` output, fed in as a `Line`). Its bottom title carries the footer key hints (`footer_line()` output). Inside the block (top to bottom): search input row, thin top-rule, list. Inter-row blank gaps disappear (entries already contain their own separator row).

- [ ] **Step 1: Read the current `render_list_mode` to understand state plumbing.**

```bash
sed -n '445,560p' src/tui/ui.rs
```

You'll see it currently builds a 7-band Layout, renders each band via separate `frame.render_widget` calls, plus dialog overlays. The new version is much shorter.

- [ ] **Step 2: Replace `render_list_mode` with the framed version.**

Full replacement of the function:

```rust
fn render_list_mode(frame: &mut Frame, app: &App) {
    let theme = th();
    let area = frame.area();
    let compact = is_compact_layout(area);

    // Header line (idle / search / loading)
    let scope = if app.workspace_filter() && app.has_project_context() {
        match app.current_project_dir_name() {
            Some(encoded) => {
                let path = crate::history::decode_project_dir_name_to_path(encoded);
                let name = crate::history::format_short_name_from_path(&path);
                format!("this project: {}", name)
            }
            None => "this project".to_string(),
        }
    } else {
        "all projects".to_string()
    };
    let header_state = if app.is_loading() {
        HeaderState::Loading {
            scope: &scope,
            so_far: app.conversations().len(),
        }
    } else if !app.query().is_empty() {
        HeaderState::Search {
            scope: &scope,
            matched: app.filtered().len(),
            total: app.conversations().len(),
        }
    } else {
        HeaderState::Idle {
            scope: &scope,
            total: app.conversations().len(),
        }
    };
    let header_title = header_line(theme, &header_state);

    // Footer line (key hints, or status message)
    let footer_state = match app.status_message() {
        Some((msg, instant)) if instant.elapsed() < STATUS_TTL => {
            FooterState::StatusMessage(msg.as_str())
        }
        _ => FooterState::ListIdle,
    };
    let footer_title = footer_line(theme, &footer_state, compact);

    // Outer block: rounded, accent-dim border, header in top title, footer
    // key hints in bottom title.
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(theme.accent_dim)))
        .title(header_title)
        .title_bottom(footer_title);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Inside the frame: search input row + top rule + list.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search input
            Constraint::Length(1), // top rule
            Constraint::Min(0),    // list
        ])
        .split(inner);

    // Search input — note: `Paragraph::new(search_line(...))` only RENDERS
    // the styled line; the terminal cursor must be positioned separately
    // so users can see where they're typing.
    frame.render_widget(Paragraph::new(search_line(theme, app.query())), chunks[0]);

    // Cursor at the end of the query. Prefix "  search ▸ " is 11 display cols.
    let prefix_cols: u16 = 11;
    let cols_before_cursor: usize = app
        .query()
        .chars()
        .take(app.cursor_pos())
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum();
    let max_x = chunks[0].x + chunks[0].width.saturating_sub(1);
    let cursor_x = (chunks[0].x
        + prefix_cols
        + cols_before_cursor.min(u16::MAX as usize) as u16)
        .min(max_x);
    frame.set_cursor_position(Position::new(cursor_x, chunks[0].y));

    // Top rule (between search and list — visually separates them)
    frame.render_widget(horizontal_rule(theme, chunks[1].width), chunks[1]);

    // List
    render_list(frame, app, chunks[2], compact);

    // Confirm-delete and Help still render on top.
    if *app.dialog_mode() == DialogMode::ConfirmDelete {
        render_confirm_dialog(frame, inner);
    }
    if *app.dialog_mode() == DialogMode::Help {
        render_help_overlay(
            frame,
            false,
            false,
            app.keys(),
            app.default_skip_permissions(),
        );
    }
}
```

A few notes:
- `horizontal_rule()` is the existing helper at line ~556 — keep using it for the inner-rule between search and list. Don't delete it yet (Task 6 will, only if it ends up unreferenced after viewer-mode also changes).
- `current_project_dir_name()` returns `Option<&str>` on `App` — confirm this from `src/tui/app.rs`. The wrapper at the start handles the workspace filter scope string.

- [ ] **Step 3: Build the binary.**

```bash
cargo build --quiet 2>&1 | tail -10
```

Expected: clean. If `Block::title_bottom` is not found, double-check that ratatui 0.30 exposes it (it does; method signature: `Block::title_bottom(self, title: impl Into<Line<'a>>)`).

- [ ] **Step 4: Run tests + clippy + fmt.**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 5: Manual smoke test.**

```bash
./target/debug/ccmanager
```

Verify visually:
- Outer rounded frame around the entire screen.
- Top border title: `◈ ccmanager · all projects · N sessions`.
- Bottom border title: `↑↓ nav  · / search  · ⏎ view  · ^R resume  · ? help`.
- Inside frame: search input row at top, thin rule below it, then list entries.
- Each entry is 3 lines: header (project + title + right-aligned `Nmsg · age`), preview (last user question), thin separator rule.
- Selected entry: gutter bar `▌` in accent color on header row; subtle bg tint on all 3 rows.
- Type a query — title and project highlight in accent color; header title updates to `M / N sessions match`.
- `Esc` clears the query.
- `?` opens help overlay (still works).

(If you don't have an interactive terminal, skip the manual test and rely on the build + test passes.)

- [ ] **Step 6: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): list screen wraps in a single rounded Block"
```

---

## Task 5: Rewrite `render_view_mode` to wrap in a single outer `Block`

**Files:**
- Modify: `src/tui/ui.rs` — `fn render_view_mode` (around line 565)

The same outer-frame treatment for the viewer. Header title shows `◈ ccmanager · session <12-char> · <N> turns`. Bottom title shows viewer key hints. Inside the block, content fills the available height. When search-typing mode is active, the inline find bar replaces the last content row.

- [ ] **Step 1: Read the current `render_view_mode` to understand state plumbing.**

```bash
sed -n '565,645p' src/tui/ui.rs
```

- [ ] **Step 2: Replace `render_view_mode` with the framed version.**

```rust
fn render_view_mode(frame: &mut Frame, app: &App, state: &ViewState) {
    let theme = th();
    let area = frame.area();
    let compact = is_compact_layout(area);

    // Header: ◈ ccmanager · session <12-char> · N turns
    let conv = app
        .conversations()
        .iter()
        .find(|c| c.path == state.conversation_path);
    let message_count = conv.map(|c| c.message_count).unwrap_or(0);
    let session_id_short = state
        .conversation_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| stem.chars().take(12).collect::<String>())
        .unwrap_or_else(|| "?".to_string());
    let scope = format!("session {}", session_id_short);
    let header_state = HeaderState::Idle {
        scope: &scope,
        total: message_count,
    };
    let header_title = override_trailing_word(
        header_line(theme, &header_state),
        "sessions",
        "turns",
    );

    // Footer: status message wins; otherwise viewer / message-nav variant.
    let footer_state = match app.status_message() {
        Some((msg, instant)) if instant.elapsed() < STATUS_TTL => {
            FooterState::StatusMessage(msg.as_str())
        }
        _ if state.message_nav_active => FooterState::ViewerMessageNav,
        _ => FooterState::Viewer,
    };
    let footer_title = footer_line(theme, &footer_state, compact);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(theme.accent_dim)))
        .title(header_title)
        .title_bottom(footer_title);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Find bar (when search-typing): takes the last content row.
    let search_active = state.search_mode == ViewSearchMode::Typing;
    if search_active {
        let constraints = [Constraint::Min(0), Constraint::Length(1)];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);
        render_view_content(frame, state, chunks[0]);
        render_search_input(frame, state, chunks[1]);
    } else {
        render_view_content(frame, state, inner);
    }

    // Dialog overlays stay the same.
    match app.dialog_mode() {
        DialogMode::ConfirmDelete => render_confirm_dialog(frame, inner),
        DialogMode::YankMenu { selected } => render_export_menu(frame, *selected),
        DialogMode::RenameInput { buffer } => render_rename_dialog(frame, buffer),
        DialogMode::Help => render_help_overlay(
            frame,
            true,
            app.is_single_file_mode(),
            app.keys(),
            app.default_skip_permissions(),
        ),
        DialogMode::None => {}
    }
}
```

- [ ] **Step 3: Build.**

```bash
cargo build --quiet 2>&1 | tail -10
```

Clean.

- [ ] **Step 4: Run fmt + clippy + tests.**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 5: Manual smoke test the viewer.**

```bash
./target/debug/ccmanager
```

Open a conversation (`Enter`). Verify:
- Outer rounded frame around the viewer.
- Top border: `◈ ccmanager · session abc-1234 · N turns`.
- Bottom border: viewer key hints (`↑↓ scroll  · / search  · e copy  · r rename  · q back  · ? help`).
- Inside: the ledger transcript fills the frame.
- Press `/`: a `find ▸ ` line appears on the LAST row inside the frame (just above the bottom border); type to search; `M / N matches` counter on the right.
- Press `Esc` to leave the find bar.
- Press `q` to return to the list.

- [ ] **Step 6: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): viewer screen wraps in a single rounded Block"
```

---

## Task 6: Cleanup pass — drop now-unused helpers, verify CI clean

**Files:**
- Modify: `src/tui/ui.rs`

After Tasks 4 and 5, some helpers may have lost their only callers. Don't delete blindly — check each.

- [ ] **Step 1: Check whether `override_trailing_word` is still used.**

```bash
grep -n "override_trailing_word" src/tui/ui.rs
```

It's still used in `render_view_mode` (Task 5) — the suffix swap from "sessions" to "turns". **Keep it.**

- [ ] **Step 2: Check whether `horizontal_rule` is still used.**

```bash
grep -n "horizontal_rule" src/tui/ui.rs
```

It's used in `render_list_mode` (Task 4) for the rule between search and list. **Keep it.**

- [ ] **Step 3: Run the full lint and test sweep.**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 4: If clippy flags new dead code (functions that lost all callers), address each.**

For each warning, decide:
- Truly dead → delete the function (and its tests).
- Forward-looking pub API → add `#[allow(dead_code)]` with a one-line `// kept for future use, see <task-or-spec>` comment.
- Used but the compiler doesn't see it → debug.

Run `cargo clippy --all-targets --all-features 2>&1 | grep "is never used"` to find them.

- [ ] **Step 5: Commit any cleanup.**

If you made any changes:

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): drop helpers that lost their callers in the v3 refactor"
```

If nothing changed, skip the commit.

---

## Task 7: Wrap-up — CHANGELOG entry, final CI sweep, snapshot to main

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Confirm we're on `dev` and CI is clean.**

```bash
git rev-parse --abbrev-ref HEAD   # MUST print `dev`
cargo fmt --all -- --check && echo "FMT CLEAN"
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 2: Add a CHANGELOG entry.**

Open `CHANGELOG.md`. Under `## Unreleased`, add (above the existing v1 bullet):

```markdown
- **TUI v3 Framed Hybrid.** The list and viewer screens get an outer
  rounded frame back. The brand line lives in the frame's top title
  strip (`◈ ccmanager · scope · count`); the context-aware key-hint
  footer lives in the bottom title strip. List entries are 3 rows:
  header (project + title + right-aligned `<N>msg · <age>`), preview
  (the **start of the last user question** from the conversation, dim),
  and a thin `─` separator. Selection switches from chevron (`◆`) back
  to a gutter bar (`▌`) on the header row, paired with the existing
  bg-tint. Compact-mode fallback drops to 2 rows (header + separator).
  Viewer mode gets the same framed treatment with a session-id header.
  Search-as-you-type accent highlight, cool blue palette, and the
  shared modal helper carry over from v1 unchanged. See
  `docs/superpowers/specs/2026-05-23-tui-framed-hybrid-design.md`.
- **Fix: rename-mid-session no longer leaves the project under its
  old name.** The parser was extracting `cwd` from the first user
  entry that carried one and ignoring all subsequent values; if the
  user renamed their project folder during a live session, the list
  kept showing the project under the pre-rename name. The parser now
  keeps the latest cwd seen.
```

- [ ] **Step 3: Commit.**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): TUI v3 framed-hybrid + parser cwd fix"
```

- [ ] **Step 4: Squash-snapshot to `main`.**

```bash
git checkout main
git checkout dev -- .
git add -A
git commit --amend --no-edit
git push --force-with-lease origin main
git checkout dev
```

- [ ] **Step 5: Verify final state.**

```bash
git log --oneline main          # should still be one commit
git diff dev main --stat        # empty (trees identical)
git rev-parse --abbrev-ref HEAD # back on dev
./target/debug/ccmanager --version
```

`ccmanager --version` should still print `ccmanager 1.0.0` (no version bump in this round).

---

## Self-review notes

I went through this plan once with fresh eyes; one adjustment made inline:

- **Compact-mode fallback was 2 lines in the v3 spec (§11), and now also 2 lines in the v3 plan's `list_row_lines` rewrite (Task 3, Step 3).** The v1 plan had compact = 1 line; v3 says compact = 2 lines. The test in Task 3 asserts `lines.len() == 2` in compact mode, matching the spec.

- **`horizontal_rule()` is still needed** (Task 4 uses it for the inner rule between search and list). Task 6's cleanup step is now explicitly "check, don't delete" rather than "delete".

- **Test fixture for `Conversation`** — Task 1 Step 5 updates `make_conv` to include `last_user_question`. Task 3's tests use that updated fixture. The test ordering is correct: Task 1 introduces the field, Task 3 exercises it via the fixture.

- **Spec §3 says the preview row uses `last_user_question` and has no "You:" prefix.** Task 3, Step 1 (`preview_row_shows_last_user_question`) asserts on both: content present AND no `You:` prefix.

- **Spec §14 (parser cwd fix)** is covered by Task 2 with its own test.

- **Spec §11 compact fallback** is covered by Task 3's `compact_mode_returns_two_lines_no_preview` test.

- **No tests for the integration of `render_list_mode` / `render_view_mode`** — these are ratatui rendering paths that require a TestBackend setup that the existing codebase doesn't currently have. Smoke tests in Tasks 4 and 5 are the verification. If a regression appears, we add a TestBackend-based test in a follow-up.

All §1-§14 of the spec are covered by Tasks 1-6. Task 7 is workflow wrap-up.
