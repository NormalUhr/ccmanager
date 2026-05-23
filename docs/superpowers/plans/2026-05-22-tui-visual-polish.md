# TUI Visual Polish — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-05-22-tui-visual-polish-design.md`

**Goal:** Apply the Modern Minimal visual treatment to ccmanager's TUI — drop the outer border, render two-line list entries with muted metadata, add a brand-marked header band and a context-aware footer band, refine modal internals, and consolidate the palette on a cool-blue accent — without changing any keybindings or layout/IA.

**Architecture:** Two-layer refactor inside `src/tui/`. (1) Pure "view-model" helper functions return `ratatui::text::Line` values for the header, footer, list-row, search-input — these are unit-testable. (2) The screen renderers (`render_list_mode`, `render_view_mode`) compose those Lines into a band layout via `ratatui::layout::Layout::vertical`, dropping the outer `Block::default().borders(ALL)` wrap. Modals share a `render_modal()` helper that absorbs the §6 refinements. Color tokens are added to `src/tui/theme.rs`; existing token names are reused where they match.

**Tech Stack:** Rust, `ratatui` 0.30, `crossterm` 0.29, no new dependencies.

**Workflow rule:** All commits land on `dev`. After every task's commit, leave the workspace on `dev`. At the very end (Task 11) the final tree gets snapshot-amended onto `main` and force-pushed — that's the only `main` interaction.

---

## File map

Files this plan touches (in roughly the order they're modified):

- `src/tui/theme.rs` — adjust the palette: change the `accent` hex to cool blue, add `text_tertiary` and `bg_tint` tokens, keep everything else as-is.
- `src/tui/ui.rs` — bulk of the work:
  - New helper functions: `header_line()`, `footer_line()`, `search_line()`, `list_row_lines()` — pure, return `Line` / `Vec<Line>`. Live in `src/tui/ui.rs` for now (split into a `view_model.rs` later if it ever needs to grow).
  - `render_list_mode()` — rewrite the layout to use the new band composition.
  - `render_view_mode()` — same band composition.
  - `render_search_bar()` — replace box with inline styled line.
  - `render_confirm_dialog()`, `render_export_menu()`, `render_rename_dialog()`, `render_help_overlay()` — refactor to share a `render_modal()` helper.
- `src/tui/viewer.rs` — color the speaker label (`You` / `Claude`) in accent when assembling ledger lines.
- `src/tui/app.rs` — add a small `compact_layout()` helper that returns `bool` based on the current frame area; tweak `viewport_height` math because entries are 3 rows (2 + blank) instead of 1.
- `src/history/mod.rs` — verify `message_count` is exposed on `Conversation` (it is — line 63). No new fields needed; "turns" = `message_count`. "Resumed Nx" is dropped from the first pass — counting cross-file copies is non-trivial and the spec marks it optional ("resumed Nx" line only when count > 0, which is "never" until we compute it). A follow-up plan can add it.

A note on `Conversation` fields: the spec asked to show a "resumed Nx" count. After scouting the code, the `Conversation` struct does not carry a resume-count field today, and synthesizing one would mean scanning across project directories for copies sharing a `session_id`. That's a feature, not a polish change. **This plan drops "resumed Nx" from the list-row metadata.** Everything else in §3 is honored. If you decide later you want it, it's a separate plan.

---

## Task 1: Palette refresh

**Files:**
- Modify: `src/tui/theme.rs` (both `Theme::dark()` and `Theme::light()` builders)
- Test: existing tests in `src/tui/theme.rs` (if any) + new unit test asserting the accent values

- [ ] **Step 1: Read the current theme.rs to confirm field shape.**

The struct already has `accent`, `accent_dim`, `text_primary`, `text_secondary`, `text_muted`, `selection_bg`, `overlay_bg`. We're going to (a) repaint `accent` to cool blue in both themes, (b) add one new field `text_tertiary` for the "more-dim" tier the spec mentions, (c) keep everything else.

- [ ] **Step 2: Add `text_tertiary` field to the `Theme` struct.**

Edit `src/tui/theme.rs` — in the `pub struct Theme { … }` block, after `pub text_muted: Rgb,` add:

```rust
    /// More dimmed than text_muted. Used for tertiary metadata
    /// (placeholder text, footer key descriptions).
    pub text_tertiary: Rgb,
```

- [ ] **Step 3: Populate `text_tertiary` and repaint `accent` in `Theme::dark()`.**

Find `fn dark() -> Self { Self { … } }` in `src/tui/theme.rs`. Update:

```rust
            accent: (108, 184, 255),      // was: (78, 201, 176) — cool blue, replaces teal
            accent_dim: (74, 130, 184),   // ~60% strength of the new accent
            …
            text_muted: (100, 100, 100),
            text_tertiary: (74, 74, 74),  // NEW — more dim than text_muted
            …
```

(Leave the other field initialisations as they were.)

- [ ] **Step 4: Populate `text_tertiary` and repaint `accent` in `Theme::light()`.**

Same file, in `fn light()`:

```rust
            accent: (10, 111, 199),       // cool blue for light theme
            accent_dim: (58, 130, 184),
            …
            text_muted: (130, 130, 130),
            text_tertiary: (170, 170, 170), // NEW
            …
```

(Light-theme exact values can be tuned by eye — the constraint is `text_tertiary > text_muted > text_secondary > text_primary` in brightness.)

- [ ] **Step 5: Add a unit test asserting the accent is cool-blue, not teal.**

Append to `src/tui/theme.rs` (before the existing `#[cfg(test)]` block if there is one, else create one):

```rust
#[cfg(test)]
mod palette_tests {
    use super::*;

    #[test]
    fn accent_is_cool_blue_in_both_themes() {
        // Cool blue = high blue channel, low red channel. Teal would
        // have a high green channel and low red — this guards against
        // accidentally reverting to the old teal accent.
        for theme in [Theme::dark(), Theme::light()] {
            let (r, g, b) = theme.accent;
            assert!(
                b > r && b > g,
                "accent should be blue-dominant, got rgb({},{},{})",
                r, g, b
            );
        }
    }

    #[test]
    fn text_tertiary_is_dimmer_than_text_muted_in_dark() {
        // In a dark theme, "dimmer" means closer to the background.
        // We pick the avg-channel as a rough proxy.
        let t = Theme::dark();
        let avg = |(r, g, b): Rgb| (r as u32 + g as u32 + b as u32) / 3;
        assert!(
            avg(t.text_tertiary) < avg(t.text_muted),
            "text_tertiary must be dimmer than text_muted (dark theme)"
        );
    }
}
```

- [ ] **Step 6: Build and test.**

Run: `cargo build --quiet && cargo test --quiet palette_tests 2>&1 | tail -10`

Expected: build clean, 2 palette tests pass.

- [ ] **Step 7: Commit on dev.**

```bash
git add src/tui/theme.rs
git commit -m "tui(theme): cool-blue accent + text_tertiary token"
```

---

## Task 2: `header_line()` helper

**Files:**
- Modify: `src/tui/ui.rs` (top — group near existing helpers)
- Test: same file, `#[cfg(test)]` block at the bottom

Pure function returning a `ratatui::text::Line` for the header band.

- [ ] **Step 1: Write the failing test.**

At the bottom of `src/tui/ui.rs`, in the `#[cfg(test)]` mod tests block (create one if it doesn't exist), add:

```rust
#[cfg(test)]
mod header_tests {
    use super::*;
    use crate::tui::theme::Theme;

    fn theme() -> Theme { Theme::dark() }

    #[test]
    fn idle_state_shows_total_count() {
        let line = header_line(&theme(), &HeaderState::Idle { scope: "all projects", total: 47 });
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("◈ ccmanager"));
        assert!(rendered.contains("all projects"));
        assert!(rendered.contains("47 sessions"));
    }

    #[test]
    fn search_active_shows_matched_fraction() {
        let line = header_line(
            &theme(),
            &HeaderState::Search { scope: "all projects", matched: 5, total: 47 },
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("5 / 47 sessions match"));
    }

    #[test]
    fn loading_state_shows_so_far_count() {
        let line = header_line(
            &theme(),
            &HeaderState::Loading { scope: "all projects", so_far: 12 },
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("loading… 12 so far"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails (helper doesn't exist yet).**

Run: `cargo test --quiet header_tests 2>&1 | tail -10`

Expected: compile error — `header_line` not found, `HeaderState` not found.

- [ ] **Step 3: Implement `HeaderState` enum and `header_line()`.**

Near the top of `src/tui/ui.rs` (after the `use` block), add:

```rust
/// State of the header band — drives the right-hand metadata.
pub enum HeaderState<'a> {
    Idle { scope: &'a str, total: usize },
    Search { scope: &'a str, matched: usize, total: usize },
    Loading { scope: &'a str, so_far: usize },
}

/// Render the header band as a single styled line:
///   `◈ ccmanager  ·  <scope>  ·  <count metadata>`
pub fn header_line(theme: &Theme, state: &HeaderState<'_>) -> Line<'static> {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::Span;

    let accent = Style::default()
        .fg(rgb(theme.accent))
        .add_modifier(Modifier::BOLD);
    let primary = Style::default().fg(rgb(theme.text_primary));
    let dim = Style::default().fg(rgb(theme.text_muted));
    let sep = || Span::styled("  ·  ", dim);

    let (scope_text, right): (&str, String) = match state {
        HeaderState::Idle { scope, total } => (scope, format!("{} sessions", total)),
        HeaderState::Search { scope, matched, total } => {
            (scope, format!("{} / {} sessions match", matched, total))
        }
        HeaderState::Loading { scope, so_far } => {
            (scope, format!("loading… {} so far", so_far))
        }
    };

    Line::from(vec![
        Span::styled("  ◈ ccmanager", accent),
        sep(),
        Span::styled(scope_text.to_string(), primary),
        sep(),
        Span::styled(right, dim),
    ])
}
```

Add a small helper `fn rgb((r, g, b): (u8, u8, u8)) -> Color { Color::Rgb(r, g, b) }` near the top of the file if it doesn't already exist (search for it first — it likely already does; if so, reuse).

- [ ] **Step 4: Run the test to verify it passes.**

Run: `cargo test --quiet header_tests 2>&1 | tail -10`

Expected: 3 passed.

- [ ] **Step 5: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): header_line() helper + HeaderState enum"
```

---

## Task 3: `footer_line()` helper

**Files:**
- Modify: `src/tui/ui.rs`
- Test: same file

- [ ] **Step 1: Write the failing test.**

In `src/tui/ui.rs`'s `#[cfg(test)] mod ...` block (or add a new `mod footer_tests`):

```rust
#[cfg(test)]
mod footer_tests {
    use super::*;
    use crate::tui::theme::Theme;

    fn theme() -> Theme { Theme::dark() }

    #[test]
    fn list_idle_has_all_six_key_hints() {
        let line = footer_line(&theme(), &FooterState::ListIdle);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        for hint in ["↑↓", "/", "⏎", "^R", "F5", "?"] {
            assert!(rendered.contains(hint), "missing hint {:?} in {:?}", hint, rendered);
        }
    }

    #[test]
    fn viewer_state_omits_resume_and_refresh() {
        let line = footer_line(&theme(), &FooterState::Viewer);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!rendered.contains("^R"));
        assert!(!rendered.contains("F5"));
        assert!(rendered.contains("e copy"));
        assert!(rendered.contains("r rename"));
    }

    #[test]
    fn status_message_replaces_hints() {
        let line = footer_line(
            &theme(),
            &FooterState::StatusMessage("Refreshed: 47 conversations"),
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("Refreshed: 47 conversations"));
        assert!(!rendered.contains("↑↓"));
    }

    #[test]
    fn message_nav_swaps_in_y_copy_message() {
        let line = footer_line(&theme(), &FooterState::ViewerMessageNav);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("J K"));
        assert!(rendered.contains("y copy message"));
        assert!(!rendered.contains("e copy"));
    }
}
```

- [ ] **Step 2: Run test, confirm failure.**

Run: `cargo test --quiet footer_tests 2>&1 | tail -10`

Expected: compile error — `footer_line` / `FooterState` not found.

- [ ] **Step 3: Implement `FooterState` and `footer_line()`.**

In `src/tui/ui.rs`, after `HeaderState`:

```rust
pub enum FooterState<'a> {
    ListIdle,
    Viewer,
    ViewerMessageNav,
    StatusMessage(&'a str),
}

/// Render the footer band as a single styled line.
///
/// Keys are rendered in `text_primary`, their one-word descriptions in
/// `text_muted`; status messages take over the whole line in `accent`.
pub fn footer_line(theme: &Theme, state: &FooterState<'_>) -> Line<'static> {
    use ratatui::style::Style;
    use ratatui::text::Span;

    let key = Style::default().fg(rgb(theme.text_primary));
    let desc = Style::default().fg(rgb(theme.text_muted));
    let accent = Style::default().fg(rgb(theme.accent));

    match state {
        FooterState::StatusMessage(msg) => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled(msg.to_string(), accent),
        ]),
        FooterState::ListIdle => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled("↑↓", key),
            Span::styled(" nav    ", desc),
            Span::styled("/", key),
            Span::styled("  search    ", desc),
            Span::styled("⏎", key),
            Span::styled(" view    ", desc),
            Span::styled("^R", key),
            Span::styled(" resume    ", desc),
            Span::styled("F5", key),
            Span::styled(" refresh    ", desc),
            Span::styled("?", key),
            Span::styled(" help", desc),
        ]),
        FooterState::Viewer => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled("↑↓", key),
            Span::styled(" scroll    ", desc),
            Span::styled("/", key),
            Span::styled("  search    ", desc),
            Span::styled("e", key),
            Span::styled(" copy    ", desc),
            Span::styled("r", key),
            Span::styled(" rename    ", desc),
            Span::styled("q", key),
            Span::styled(" back    ", desc),
            Span::styled("?", key),
            Span::styled(" help", desc),
        ]),
        FooterState::ViewerMessageNav => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled("J K", key),
            Span::styled(" message    ", desc),
            Span::styled("y", key),
            Span::styled(" copy message    ", desc),
            Span::styled("Esc", key),
            Span::styled(" exit nav    ", desc),
            Span::styled("?", key),
            Span::styled(" help", desc),
        ]),
    }
}
```

- [ ] **Step 4: Run tests.**

Run: `cargo test --quiet footer_tests 2>&1 | tail -10`

Expected: 4 passed.

- [ ] **Step 5: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): footer_line() helper + FooterState enum"
```

---

## Task 4: `search_line()` helper

**Files:**
- Modify: `src/tui/ui.rs`
- Test: same file

- [ ] **Step 1: Write the failing test.**

```rust
#[cfg(test)]
mod search_line_tests {
    use super::*;
    use crate::tui::theme::Theme;

    #[test]
    fn empty_query_shows_placeholder() {
        let line = search_line(&Theme::dark(), "");
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("search ▸"));
        assert!(rendered.contains("(fuzzy across all transcripts)"));
    }

    #[test]
    fn nonempty_query_omits_placeholder() {
        let line = search_line(&Theme::dark(), "deploy");
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("search ▸"));
        assert!(rendered.contains("deploy"));
        assert!(!rendered.contains("(fuzzy"));
    }
}
```

- [ ] **Step 2: Run, expect compile failure.**

Run: `cargo test --quiet search_line_tests 2>&1 | tail -10`

Expected: `search_line` not found.

- [ ] **Step 3: Implement.**

In `src/tui/ui.rs`:

```rust
/// Render the search-input band as a single styled line:
///   `  search ▸ <query>` (or, if query is empty, with a placeholder).
pub fn search_line(theme: &Theme, query: &str) -> Line<'static> {
    use ratatui::style::Style;
    use ratatui::text::Span;

    let label = Style::default().fg(rgb(theme.text_muted));
    let arrow = Style::default().fg(rgb(theme.accent));
    let primary = Style::default().fg(rgb(theme.text_primary));
    let placeholder = Style::default().fg(rgb(theme.text_tertiary));

    let mut spans = vec![
        Span::styled("  search ", label),
        Span::styled("▸", arrow),
        Span::styled(" ", label),
    ];
    if query.is_empty() {
        spans.push(Span::styled("(fuzzy across all transcripts)", placeholder));
    } else {
        spans.push(Span::styled(query.to_string(), primary));
    }
    Line::from(spans)
}
```

- [ ] **Step 4: Run tests.**

Run: `cargo test --quiet search_line_tests 2>&1 | tail -10`

Expected: 2 passed.

- [ ] **Step 5: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): search_line() helper"
```

---

## Task 5: `list_row_lines()` helper

**Files:**
- Modify: `src/tui/ui.rs`
- Test: same file

Pure function returning `Vec<Line>` (the two content lines plus a blank gap). Caller is responsible for the bg-tint highlight, which works on the Style of each Span (`bg` field).

- [ ] **Step 1: Write the failing test.**

```rust
#[cfg(test)]
mod list_row_tests {
    use super::*;
    use crate::history::Conversation;
    use crate::tui::theme::Theme;
    use chrono::TimeZone;

    fn make_conv(project: &str, title: &str, mins_ago: i64, message_count: usize) -> Conversation {
        let now = chrono::Local::now();
        let ts = now - chrono::Duration::minutes(mins_ago);
        Conversation {
            // fill in the fields the production struct requires.
            // The point of this test isn't to enumerate every Conversation field;
            // construct via the existing test-helper if there is one. See
            // src/history/parser.rs::tests for a pattern.
            path: std::path::PathBuf::from(format!("/fake/{}.jsonl", title)),
            project_name: Some(project.to_string()),
            project_path: None,
            timestamp: ts,
            message_count,
            // ...defaults for the rest...
            ..Default::default()  // if Conversation derives Default; if not,
                                  // use the existing test helper or extend.
        }
    }

    #[test]
    fn unselected_row_has_two_content_lines_plus_blank() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(&Theme::dark(), &conv, /* selected = */ false);
        assert_eq!(lines.len(), 3, "expected: line1 + line2 + blank");
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first.contains("ccmanager"));
        assert!(first.contains("Add F5 refresh"));
        let second: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(second.contains("47 turns"));
    }

    #[test]
    fn selected_row_starts_with_diamond() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(&Theme::dark(), &conv, /* selected = */ true);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        // First non-space character of the selected row should be ◆.
        let trimmed = first.trim_start();
        assert!(trimmed.starts_with("◆"), "expected diamond at start: {:?}", first);
    }

    #[test]
    fn unselected_row_does_not_start_with_diamond() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(&Theme::dark(), &conv, /* selected = */ false);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let trimmed = first.trim_start();
        assert!(!trimmed.starts_with("◆"));
    }
}
```

NOTE: `Conversation` may not implement `Default`. Before implementing, check `src/history/mod.rs` — adapt the test fixture to use whatever construction pattern already exists in the codebase (search `Conversation {` in test files to find one). The assertions above are what matter, not the exact construction.

- [ ] **Step 2: Run, expect failure.**

Run: `cargo test --quiet list_row_tests 2>&1 | tail -10`

Expected: compile error — `list_row_lines` not found (and/or the test fixture needs adjustment).

- [ ] **Step 3: Implement `list_row_lines()`.**

In `src/tui/ui.rs`:

```rust
use crate::history::Conversation;

/// Render a single conversation entry as 3 lines:
///   line 1: `  ◆ <project>   <title>`        (selection glyph + bold title)
///   line 2: `     <age>  ·  <N> turns`        (metadata, all dimmed)
///   line 3: empty gap line
///
/// The caller applies the bg-tint via a separate Block highlight; this
/// function only produces the foreground content.
pub fn list_row_lines(
    theme: &Theme,
    conv: &Conversation,
    selected: bool,
) -> Vec<Line<'static>> {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Span;

    let project = conv.project_name.as_deref().unwrap_or("?");
    let title = conv
        .custom_title
        .as_deref()
        .or(conv.summary.as_deref())
        .unwrap_or("(no title)");

    let glyph = if selected { "◆" } else { " " };
    let glyph_style = Style::default().fg(rgb(theme.accent));
    let project_style = Style::default().fg(rgb(theme.text_muted));
    let title_style = Style::default()
        .fg(rgb(theme.text_primary))
        .add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() });
    let dim = Style::default().fg(rgb(theme.text_muted));

    let line1 = Line::from(vec![
        Span::styled("  ", project_style),
        Span::styled(glyph.to_string(), glyph_style),
        Span::styled(" ", project_style),
        Span::styled(format!("{:<10}", project), project_style),
        Span::styled("  ", project_style),
        Span::styled(title.to_string(), title_style),
    ]);

    let age = format_age(conv.timestamp); // reuse existing helper; see below
    let line2 = Line::from(vec![
        Span::styled("    ", dim),
        Span::styled(age, dim),
        Span::styled("  ·  ", dim),
        Span::styled(format!("{} turns", conv.message_count), dim),
    ]);

    vec![line1, line2, Line::from("")]
}
```

`format_age` already exists in the codebase — search for it (the `format_short_relative_time` or similar function in `src/tui/ui.rs` or `src/history/`). If it lives elsewhere, import or move it to a small `src/tui/format.rs` module. **Do not invent a new format**; reuse what the codebase already produces ("2h ago", "yesterday", etc.).

- [ ] **Step 4: Adjust the test fixture to match real `Conversation` construction, then re-run.**

Run: `cargo test --quiet list_row_tests 2>&1 | tail -10`

Iterate on the test fixture (use a helper from `src/history/parser.rs::tests` or build by hand) until all 3 list-row tests pass.

Expected: 3 passed.

- [ ] **Step 5: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): list_row_lines() helper (2-line entries)"
```

---

## Task 6: Rewrite `render_list_mode` to use the new band layout

**Files:**
- Modify: `src/tui/ui.rs:113` (`fn render_list_mode`)

This is the integration step. We replace the boxed layout with vertical bands using `ratatui::layout::Layout::vertical` and call the helpers from Tasks 2-5.

- [ ] **Step 1: Read the existing `render_list_mode` (lines 113-170 area) to understand current state.**

Run: `sed -n '113,170p' src/tui/ui.rs` and study how it currently uses `Block::default().borders(Borders::ALL)`, what areas it splits into, and how it passes data to `render_list`.

- [ ] **Step 2: Replace `render_list_mode` with the band-layout version.**

The new body (full replacement of the function):

```rust
fn render_list_mode(frame: &mut Frame, app: &App) {
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::widgets::Paragraph;

    let theme = theme();
    let area = frame.area();

    // Five bands:  header (1)  ·  rule (1)  ·  search (1)  ·  list (min)
    //              ·  rule (1)  ·  footer (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // header
            Constraint::Length(1),   // top rule
            Constraint::Length(1),   // search input
            Constraint::Length(1),   // blank gap above list
            Constraint::Min(0),      // list
            Constraint::Length(1),   // bottom rule
            Constraint::Length(1),   // footer
        ])
        .split(area);

    // 1. Header
    let scope = if app.workspace_filter() {
        format!("this project: {}", app.current_project_display_name())
    } else {
        "all projects".to_string()
    };
    let header_state = if app.is_loading() {
        HeaderState::Loading { scope: &scope, so_far: app.conversations().len() }
    } else if !app.query().is_empty() {
        HeaderState::Search {
            scope: &scope,
            matched: app.filtered().len(),
            total: app.conversations().len(),
        }
    } else {
        HeaderState::Idle { scope: &scope, total: app.conversations().len() }
    };
    frame.render_widget(Paragraph::new(header_line(theme, &header_state)), chunks[0]);

    // 2. Top rule
    frame.render_widget(horizontal_rule(theme, area.width), chunks[1]);

    // 3. Search input
    frame.render_widget(Paragraph::new(search_line(theme, app.query())), chunks[2]);

    // 4. (blank gap above list — empty)

    // 5. List
    render_list(frame, app, chunks[4]);

    // 6. Bottom rule
    frame.render_widget(horizontal_rule(theme, area.width), chunks[5]);

    // 7. Footer
    let footer_state = match app.status_message() {
        Some(msg) if !msg.expired() => FooterState::StatusMessage(msg.text()),
        _ => FooterState::ListIdle,
    };
    frame.render_widget(Paragraph::new(footer_line(theme, &footer_state)), chunks[6]);
}

/// Render a thin horizontal rule across `width` columns.
fn horizontal_rule(theme: &Theme, width: u16) -> Paragraph<'static> {
    use ratatui::style::Style;
    let rule = "─".repeat(width as usize);
    Paragraph::new(Span::styled(rule, Style::default().fg(rgb(theme.separator))))
}
```

NOTE: The exact API for `app.status_message()` may differ; adapt to the actual `App` method shape. The point of the structure is right; the data plumbing is whatever the existing `App` already exposes (search the codebase for current callers of `render_list_status_bar` to learn the shape).

- [ ] **Step 3: Rewrite `render_list` (around line 1245) to emit two-line entries using `list_row_lines()`.**

The current `render_list` probably uses a `List` widget with `ListItem`s. We need to either:
- (A) keep using `List`/`ListItem` but pass 3-line `ListItem`s and let ratatui's `List` highlight the selected one, **or**
- (B) iterate manually with `Paragraph`s, drawing the selected row's bg via a `Block::default().style(...)`.

Pick (A) if `ListItem::new(Vec<Line>)` accepts multiple lines (it does in ratatui 0.30). Mapping:

```rust
fn render_list(frame: &mut Frame, app: &App, area: Rect) {
    use ratatui::widgets::{List, ListItem, ListState};

    let theme = theme();
    let convs = app.filtered_conversations(); // existing accessor
    let selected = app.selected_index();      // existing accessor

    let items: Vec<ListItem<'static>> = convs
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let lines = list_row_lines(theme, c, /* selected = */ i == selected);
            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default().bg(rgb(theme.selection_bg)));

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}
```

- [ ] **Step 4: Build & smoke-test interactively.**

```bash
cargo build --quiet
./target/debug/ccmanager
```

Open ccmanager. Verify visually:
- Header band at top with `◈ ccmanager · all projects · N sessions`.
- Thin horizontal rule below header.
- Search input as `  search ▸ ` with placeholder.
- List rows are 2-line entries with blank gap; selected row highlighted with bg tint.
- Bottom rule + footer with key hints.
- No outer border anywhere.

Type a few chars to search. The header should switch to `5 / 47 sessions match` (or similar). `Esc` clears.

- [ ] **Step 5: Run tests.**

Run: `cargo test --quiet 2>&1 | grep "test result"`

Expected: all tests pass.

- [ ] **Step 6: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): list screen uses band layout, drop outer border"
```

---

## Task 7: Rewrite `render_view_mode` to use the same band layout

**Files:**
- Modify: `src/tui/ui.rs:303` (`fn render_view_mode`)
- Modify: `src/tui/ui.rs:361` (`fn render_view_header`) — repurpose

- [ ] **Step 1: Replace `render_view_mode` with the band-layout version.**

```rust
fn render_view_mode(frame: &mut Frame, app: &App, state: &ViewState) {
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::widgets::Paragraph;

    let theme = theme();
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // header
            Constraint::Length(1),  // top rule
            Constraint::Min(0),     // content
            Constraint::Length(1),  // bottom rule
            Constraint::Length(1),  // footer
        ])
        .split(area);

    // Header: `◈ ccmanager · session abc-123 · 47 turns`
    let session_id_short = state
        .session_id()
        .chars()
        .take(12)
        .collect::<String>();
    let scope = format!("session {}", session_id_short);
    let header_state = HeaderState::Idle {
        scope: &scope,
        total: state.message_count(),
    };
    // Override the trailing word: in viewer mode we say "turns", not "sessions".
    let line = header_line(theme, &header_state);
    let line = override_trailing_word(line, "sessions", "turns");
    frame.render_widget(Paragraph::new(line), chunks[0]);

    frame.render_widget(horizontal_rule(theme, area.width), chunks[1]);

    render_view_content(frame, state, chunks[2]);

    frame.render_widget(horizontal_rule(theme, area.width), chunks[3]);

    let footer_state = if state.message_nav_active() {
        FooterState::ViewerMessageNav
    } else {
        FooterState::Viewer
    };
    let footer = match app.status_message() {
        Some(msg) if !msg.expired() => Paragraph::new(footer_line(theme, &FooterState::StatusMessage(msg.text()))),
        _ => Paragraph::new(footer_line(theme, &footer_state)),
    };
    frame.render_widget(footer, chunks[4]);
}

/// Replace the last `Span` of a `Line` whose text ends with `from_suffix`
/// with one that ends with `to_suffix` instead. Used by viewer-mode
/// header to render "turns" instead of "sessions".
fn override_trailing_word(mut line: Line<'static>, from_suffix: &str, to_suffix: &str) -> Line<'static> {
    if let Some(last) = line.spans.last_mut()
        && last.content.ends_with(from_suffix)
    {
        let s = last.content.to_string();
        let new = s.trim_end_matches(from_suffix).to_string() + to_suffix;
        last.content = new.into();
    }
    line
}
```

The `state.session_id()` and `state.message_count()` may not be methods today — check `ViewState`'s shape in `src/tui/app.rs` and use whichever fields are there. Path stem + `state.rendered_lines.len()` are acceptable fallbacks if no clean count exists.

- [ ] **Step 2: Remove the old `render_view_header` if it's now unused.**

Run: `grep -n "render_view_header" src/tui/ui.rs`

If it has no remaining callers, delete it. If it does (e.g., some other place renders just the header), leave it; it'll be cleaned up in a follow-up.

- [ ] **Step 3: Build & smoke-test the viewer.**

```bash
cargo build --quiet
./target/debug/ccmanager
```

Open a conversation. Verify:
- Header: `◈ ccmanager · session <12-char> · N turns`.
- Thin rules above and below the content.
- Footer: viewer keys (`↑↓ scroll  /  search  e copy  r rename  q back  ? help`).
- No outer border.

- [ ] **Step 4: Build + test.**

```bash
cargo build --quiet
cargo test --quiet 2>&1 | grep "test result"
```

- [ ] **Step 5: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): viewer screen uses band layout"
```

---

## Task 8: Color the speaker label in the ledger

**Files:**
- Modify: `src/tui/viewer.rs`

- [ ] **Step 1: Find where speaker labels are rendered.**

```bash
grep -n '"You"\|"Claude"\|speaker' src/tui/viewer.rs | head
```

Look for the spot that emits the speaker prefix (e.g., a `Span::raw("Claude:")` or similar).

- [ ] **Step 2: Wrap the speaker label in an accent-colored span.**

For each spot, change:

```rust
spans.push(Span::raw("Claude:"));
```

to:

```rust
spans.push(Span::styled(
    "Claude:",
    Style::default().fg(rgb(theme.accent)),
));
```

(`theme` is available via the existing `theme()` accessor; if not, import it.)

Do the same for the "You:" label.

- [ ] **Step 3: Visual check.**

```bash
cargo build --quiet
./target/debug/ccmanager <pick-any-conversation>
```

Speaker labels should now read in cool blue. Everything else in the ledger should look unchanged.

- [ ] **Step 4: Build + test.**

```bash
cargo test --quiet 2>&1 | grep "test result"
```

- [ ] **Step 5: Commit.**

```bash
git add src/tui/viewer.rs
git commit -m "tui(viewer): accent the speaker labels"
```

---

## Task 9: Inline search input in the viewer

**Files:**
- Modify: `src/tui/ui.rs:703` (`fn render_search_input` for the viewer)

The viewer's inline find bar (when `/` is pressed inside a transcript) currently uses a boxed widget. Replace with an inline styled line.

- [ ] **Step 1: Read the current function (around lines 703-?).**

- [ ] **Step 2: Replace it with the inline version.**

```rust
fn render_search_input(frame: &mut Frame, state: &ViewState, area: Rect) {
    use ratatui::style::Style;
    use ratatui::text::Span;
    use ratatui::widgets::Paragraph;

    let theme = theme();
    let label = Style::default().fg(rgb(theme.text_muted));
    let arrow = Style::default().fg(rgb(theme.accent));
    let primary = Style::default().fg(rgb(theme.text_primary));

    let matches_info = if state.search_query().is_empty() {
        String::new()
    } else {
        format!("    {} / {} matches", state.current_match() + 1, state.match_count())
    };

    let line = Line::from(vec![
        Span::styled("  find ", label),
        Span::styled("▸", arrow),
        Span::styled(" ", label),
        Span::styled(state.search_query().to_string(), primary),
        Span::styled(matches_info, label),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
```

Adapt the `state.search_query()` / `state.current_match()` / `state.match_count()` calls to whatever the `ViewState` struct already exposes.

- [ ] **Step 3: Smoke-test the find bar.**

```bash
cargo build --quiet
./target/debug/ccmanager  # open a conv, press / to search
```

Verify the find bar is a single inline line, no box.

- [ ] **Step 4: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): inline find bar in viewer"
```

---

## Task 10: Refactor modal dialogs to share a `render_modal()` helper

**Files:**
- Modify: `src/tui/ui.rs` (`render_confirm_dialog`, `render_export_menu`, `render_rename_dialog`, `render_help_overlay`)

The four current modals all draw a `Block::default().borders(ALL).border_type(BorderType::Rounded)` and add their own padding. Extract a single helper.

- [ ] **Step 1: Add `render_modal()` helper.**

```rust
/// Layout for the small modal popovers (confirm, export menu, rename,
/// help overlay). Caller provides the title, the body (a list of lines
/// — the option list, or the help-shortcut table, etc.), and the
/// dismiss hint that goes into the bottom border ("Esc cancel · ⏎ select").
///
/// The modal is centered on `frame.area()` and sized to `(width, height)`.
/// Border is rounded, accent-dim color; title is full accent.
pub fn render_modal(
    frame: &mut Frame,
    title: &str,
    body: Vec<Line<'static>>,
    hint: &str,
    width: u16,
    height: u16,
) {
    use ratatui::layout::Rect;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Span;
    use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

    let theme = theme();
    let area = frame.area();
    let rect = Rect {
        x: (area.width.saturating_sub(width)) / 2,
        y: (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    frame.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(theme.accent_dim)))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title.to_string(),
                Style::default()
                    .fg(rgb(theme.accent))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_bottom(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                hint.to_string(),
                Style::default().fg(rgb(theme.text_muted)),
            ),
            Span::raw(" "),
        ]));

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let body_para = Paragraph::new(body);
    frame.render_widget(body_para, inner);
}
```

- [ ] **Step 2: Refactor `render_confirm_dialog` to use `render_modal`.**

```rust
fn render_confirm_dialog(frame: &mut Frame, area: Rect) {
    let _ = area; // render_modal computes its own area
    let body = vec![
        Line::from(""),
        Line::from("  Delete this conversation?"),
        Line::from(""),
        Line::from("  ◆ y  yes, delete"),
        Line::from("    n  no, cancel"),
    ];
    render_modal(
        frame,
        "Confirm delete",
        body,
        "y confirm  ·  n / Esc cancel",
        46,
        7,
    );
}
```

- [ ] **Step 3: Refactor `render_export_menu`, `render_rename_dialog`, `render_help_overlay` to use `render_modal`.**

For each, read the current implementation, extract its body content into a `Vec<Line>`, and call `render_modal` with title + body + hint + dimensions. Keep the option-selection styling (use the same bg-tint trick from the list: selected option gets `Style::default().bg(rgb(theme.selection_bg))`).

- [ ] **Step 4: Visual smoke-test each modal.**

```bash
./target/debug/ccmanager
```

- Press `?` → help overlay should match the new style.
- Select a conversation, press `Ctrl+X` → confirm dialog.
- In the viewer, press `e` → export menu.
- In the viewer, press `r` → rename dialog.

All four should have: muted border, accent-color title, hint in bottom border, selected option highlighted by bg-tint.

- [ ] **Step 5: Build + test.**

```bash
cargo build --quiet
cargo test --quiet 2>&1 | grep "test result"
```

- [ ] **Step 6: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): shared render_modal() helper for all four dialogs"
```

---

## Task 11: Compact-mode fallback for small terminals

**Files:**
- Modify: `src/tui/ui.rs` (`render_list_mode`, `list_row_lines`, `footer_line`)

- [ ] **Step 1: Add a `is_compact_layout()` helper near the top of `src/tui/ui.rs`.**

```rust
/// Returns true when the terminal is too small to comfortably show
/// the full Modern Minimal layout. In compact mode, list entries
/// collapse to one line and the footer shows fewer hints.
fn is_compact_layout(area: Rect) -> bool {
    area.height < 20 || area.width < 60
}
```

- [ ] **Step 2: Make `list_row_lines` accept a `compact: bool` parameter.**

Update the signature:

```rust
pub fn list_row_lines(
    theme: &Theme,
    conv: &Conversation,
    selected: bool,
    compact: bool,
) -> Vec<Line<'static>> {
    // ...
    if compact {
        // One-line layout: glyph + project + title + age, all on one row.
        let line = Line::from(vec![
            Span::styled(if selected { "  ◆ " } else { "    " }, glyph_style),
            Span::styled(format!("{:<10}", project), project_style),
            Span::styled("  ", project_style),
            Span::styled(title.to_string(), title_style),
            Span::styled(format!("    {}", format_age(conv.timestamp)), dim),
        ]);
        return vec![line];
    }
    // ... existing 2-line behavior ...
}
```

Update existing tests + callers to pass `compact: false`. Add tests for `compact: true` (1 line, no blank gap).

- [ ] **Step 3: Make `footer_line` accept a `compact: bool`.**

When compact, return a shorter `ListIdle` variant:

```rust
// In FooterState::ListIdle branch:
if compact {
    return Line::from(vec![
        Span::styled("  ", desc),
        Span::styled("↑↓", key),
        Span::styled(" ", desc),
        Span::styled("/", key),
        Span::styled(" search ", desc),
        Span::styled("⏎", key),
        Span::styled(" view ", desc),
        Span::styled("?", key),
        Span::styled(" help", desc),
    ]);
}
```

Update tests.

- [ ] **Step 4: Thread compact-mode through `render_list_mode` and `render_view_mode`.**

In `render_list_mode`:

```rust
let area = frame.area();
let compact = is_compact_layout(area);
// pass `compact` through to list_row_lines and footer_line
```

Same in `render_view_mode`.

- [ ] **Step 5: Smoke-test by shrinking the terminal.**

```bash
./target/debug/ccmanager
```

Resize the terminal narrow (< 60 cols) or short (< 20 rows). Verify list entries collapse to one line and footer shortens.

- [ ] **Step 6: Build + test.**

```bash
cargo build --quiet
cargo test --quiet 2>&1 | grep "test result"
```

- [ ] **Step 7: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): compact-mode fallback for small terminals"
```

---

## Task 12: Empty / loading / no-history states

**Files:**
- Modify: `src/tui/ui.rs` (`render_list` — the list-rendering function)

- [ ] **Step 1: Detect the three empty states in `render_list`.**

After computing `convs = app.filtered_conversations()`:

```rust
if convs.is_empty() {
    let theme = theme();
    let dim = Style::default().fg(rgb(theme.text_muted));
    let msg = if app.conversations().is_empty() && !app.is_loading() {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  you don't have any Claude Code conversations yet",
                dim,
            )),
        ]
    } else if !app.query().is_empty() {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  no conversations match your search",
                dim,
            )),
            Line::from(Span::styled(
                "        press Esc to clear it",
                dim,
            )),
        ]
    } else {
        // Loading and nothing has arrived yet — let the header's "loading… N so far" carry the message.
        vec![]
    };
    let para = ratatui::widgets::Paragraph::new(msg)
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(para, area);
    return;
}
```

- [ ] **Step 2: Smoke-test.**

```bash
./target/debug/ccmanager
```

Type a nonsense query like `xqzqxq`. The empty-state message should appear centered, dim.

If you don't have a way to test the no-history state safely, skip the manual smoke test for that path (covered by the conditional logic).

- [ ] **Step 3: Build + test.**

```bash
cargo build --quiet
cargo test --quiet 2>&1 | grep "test result"
```

- [ ] **Step 4: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): empty / no-match / no-history states"
```

---

## Task 13: Wrap-up — final cleanup, ensure CI is green, snapshot to main

**Files:**
- Various — final pass

- [ ] **Step 1: Confirm rustfmt + clippy are clean.**

```bash
cargo fmt --all -- --check && echo "fmt CLEAN"
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
```

Both must show clean.

- [ ] **Step 2: Run the full test suite.**

```bash
cargo test --quiet 2>&1 | grep "test result"
```

All `ok`. Numbers should be roughly: 294 + N (where N is new helper-tests) + 9 integration + 1 doctest.

- [ ] **Step 3: Full manual smoke test.**

Walk through every flow in a normal-size terminal:
- Launch, see list with new chrome.
- Type a query; header shows fraction.
- `Esc` to clear.
- `Enter` on a row → viewer; header shows session info.
- `q` back to list.
- `Tab` toggles workspace scope; header updates.
- `F5` refreshes (status message shows in footer).
- `?` shows help overlay (new modal style).
- `r` opens rename modal.
- `Ctrl+X` opens confirm modal.
- `e` opens export modal.
- Shrink terminal below thresholds; verify compact mode kicks in.

Then resume a conversation (`Ctrl+R`) and confirm the new tab opens — should be unchanged from before.

- [ ] **Step 4: CHANGELOG entry.**

Edit `CHANGELOG.md`. Under `## Unreleased`, add:

```markdown
- **TUI visual polish (Modern Minimal).** The list and viewer screens
  drop their outer borders, gain a brand-marked header band
  (`◈ ccmanager · scope · count`) and a context-aware footer with
  key hints, and use two-line entries with muted metadata. Modals
  share a single visual template with refined internals. Accent
  color repainted to cool blue (matches the existing web UI). A
  compact fallback kicks in on terminals smaller than 60×20.
  See `docs/superpowers/specs/2026-05-22-tui-visual-polish-design.md`
  for the full design.
```

Commit:

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): TUI visual polish entry"
```

- [ ] **Step 5: Squash-snapshot to `main` (per the project's git workflow rule).**

```bash
git checkout main
git checkout dev -- .
git add -A
git commit --amend --no-edit
git push --force-with-lease origin main
git checkout dev
```

- [ ] **Step 6: Verify CI on the new main commit.**

Watch `https://github.com/NormalUhr/ccmanager/actions` — the CI workflow should run rustfmt + clippy + tests on both macOS and Ubuntu. All green.

---

## Self-review notes

I went through this plan once after writing it. Adjustments made inline:

- **Resume-count dropped from §3 list-row metadata.** The `Conversation` struct doesn't carry a resume-count today, and synthesizing one requires cross-file work that's a feature not polish. The spec said "resumed Nx [optional]"; this plan honors §3 minus that line. Listed up in the **File map** section so the deviation is visible.
- **`format_age()` referenced but not defined here.** It already exists in the codebase — Task 5's text tells the engineer to find and reuse it rather than inventing a parallel format. Plan covers this with an explicit "**Do not invent a new format**" note.
- **Test fixture for `Conversation` in Task 5** is intentionally schematic. The engineer needs to mirror the existing parser-test fixture; this plan flags that explicitly rather than inventing a synthetic constructor that may not match the real struct.
- **Modal padding** spec mentioned reducing padding from "2-col / 1-row to 1-col / 0-row" — concrete padding values are encoded inline in `render_modal()`'s body (the explicit `Span::raw(" ")` padding around the title; no top blank line). The padding numbers in the spec match the implementation in the plan.
- **Header in viewer mode** uses an `override_trailing_word` helper because reusing `header_line` saves duplicating the rendering logic but the trailing word differs ("turns" vs "sessions"). The helper is small (≤10 lines) and lives next to the rest of the helpers.
- **`is_compact_layout()` thresholds** match the spec's §9 thresholds (height < 20, width < 60). Encoded as a single function — change once if we want to tune later.

All §1–§9 of the spec are covered by Tasks 1–12. Task 13 is workflow wrap-up.
