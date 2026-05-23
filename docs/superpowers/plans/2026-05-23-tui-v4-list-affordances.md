# TUI v4 — List Affordances Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-05-23-tui-v4-list-affordances-design.md`

**Goal:** Add per-row session IDs to the list, surface vim-style search nav in the viewer footer, remove mouse-click-to-enter, and fix the cursor-stuck-at-bottom scroll bug.

**Architecture:** Four scoped changes across two files. (1) `list_row_lines` gains an `id` parameter and renders a right-aligned dim numeral column. (2) `footer_line` gains a `ViewerSearchActive` variant; `render_view_mode` selects it when search is active. (3) The `MouseEventKind::Down(MouseButton::Left)` arm in `run_with_loader` is deleted along with the now-unreachable `handle_list_click` helper. (4) `App` gains `list_state: RefCell<ListState>`, which the renderer borrows mutably instead of building a fresh `ListState::default()` per render — ratatui's auto-scroll then sees the persisted offset and only adjusts when the cursor crosses a viewport edge.

**Tech Stack:** Rust, `ratatui` 0.30, `crossterm` 0.29, no new dependencies.

**Workflow rule:** All commits land on `dev`. Use the EXACT commit messages each task specifies. Do NOT push `dev`. Do NOT push tags. The final task snapshots `dev` onto `main` via amend + `--force-with-lease`.

---

## File map

| File | What changes |
|---|---|
| `src/tui/ui.rs` | `list_row_lines` gains 7th parameter `id: usize`; renders 3-col right-aligned dim ID at start of header line; preview/separator indents shift to align past the ID column. `render_list` accepts a `&mut ListState` from the caller and passes it through to `render_stateful_widget` (instead of building a fresh `ListState::default()`). `footer_line` gains a `FooterState::ViewerSearchActive { current, total }` arm. `render_view_mode` selects the new footer variant when `state.search_mode == ViewSearchMode::Active`. |
| `src/tui/app.rs` | New field `list_state: RefCell<ListState>` on `App`, default-initialized in all three constructors. New `pub fn list_state(&self) -> RefMut<'_, ListState>` accessor. All `select_*` methods and other places that mutate `self.selected` also call `self.list_state.borrow_mut().select(...)` to keep the persistent state in sync. `finish_loading`, `begin_refresh`, and constructors reset `list_state` to default. The `MouseEventKind::Down(MouseButton::Left)` arm in `run_with_loader` (~line 2844) is deleted. The now-unreachable `pub fn handle_list_click` is deleted. |

No new files, no new dependencies.

---

## Task 1: Session ID column in `list_row_lines`

**Files:**
- Modify: `src/tui/ui.rs` — `pub fn list_row_lines` (around line 280) + new tests in `mod list_row_tests`
- The single caller `render_list` updates to compute and pass the ID.

The ID column is a 3-col right-aligned dim numeral (`"  1"`, `" 99"`, `"999"`) prepended to the **header row** of each entry. Preview and separator rows indent past it so columns align. Width auto-sizes to `digits(filtered.len()).max(3)`.

- [ ] **Step 1: Write the failing tests.**

In `src/tui/ui.rs`'s `mod list_row_tests` (existing module — search for it), add three new tests:

```rust
#[test]
fn list_row_id_renders_right_aligned_in_3_cols() {
    let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
    let lines = list_row_lines(
        &Theme::dark(),
        &conv,
        /* selected = */ false,
        /* query    = */ "",
        /* compact  = */ false,
        /* inner_width = */ 80,
        /* id       = */ 1,
        /* id_width = */ 3,
    );
    let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        header.starts_with("  1 "),
        "expected `\"  1 \"` prefix (right-aligned ID, then space), got: {:?}",
        header
    );
}

#[test]
fn list_row_id_two_digit() {
    let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
    let lines = list_row_lines(
        &Theme::dark(),
        &conv,
        /* selected = */ false,
        /* query    = */ "",
        /* compact  = */ false,
        /* inner_width = */ 80,
        /* id       = */ 42,
        /* id_width = */ 3,
    );
    let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        header.starts_with(" 42 "),
        "expected `\" 42 \"` prefix, got: {:?}",
        header
    );
}

#[test]
fn list_row_id_three_digit() {
    let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
    let lines = list_row_lines(
        &Theme::dark(),
        &conv,
        /* selected = */ false,
        /* query    = */ "",
        /* compact  = */ false,
        /* inner_width = */ 80,
        /* id       = */ 999,
        /* id_width = */ 3,
    );
    let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        header.starts_with("999 "),
        "expected `\"999 \"` prefix, got: {:?}",
        header
    );
}
```

Update **all 5 existing list_row_tests** to pass the new `id` and `id_width` args (use `id = 1, id_width = 3` everywhere — they're not testing the ID, just compatible with the signature). The 5 existing tests are: `normal_mode_returns_three_lines`, `compact_mode_returns_two_lines_no_preview`, `preview_row_shows_last_user_question`, `metadata_is_right_aligned_with_msg_count_and_age`, `selected_row_has_gutter_bar`, `cjk_title_metadata_still_fits_inner_width`. Add the two new args to each.

- [ ] **Step 2: Run the tests, expect compile failure.**

Run: `cargo test --lib list_row_tests 2>&1 | tail -20`

Expected: compile errors — `list_row_lines` doesn't take 8 arguments yet.

- [ ] **Step 3: Update `list_row_lines` to accept and render the ID.**

In `src/tui/ui.rs`, modify the signature (was 6 args, becomes 8):

```rust
pub fn list_row_lines(
    theme: &Theme,
    conv: &crate::history::Conversation,
    selected: bool,
    query: &str,
    compact: bool,
    inner_width: u16,
    id: usize,
    id_width: usize,
) -> Vec<Line<'static>> {
```

Inside the function, after the existing style declarations, add the ID span at the **start** of the header line. Find the existing header construction (the section that pushes `"  "` + glyph + project + title to `header_spans`). The new shape:

```rust
    // ── Row 1 — header ──────────────────────────────────────────
    let project = conv.project_name.as_deref().unwrap_or("?");
    let title = conv
        .custom_title
        .as_deref()
        .or(conv.summary.as_deref())
        .unwrap_or("(no title)");

    let (age, _) = format_timestamp(conv.timestamp, chrono::Local::now());
    let metadata = format!("{}msg · {}", conv.message_count, age);

    // ID column: right-aligned numeral, fixed width = id_width.
    let id_text = format!("{:>width$}", id, width = id_width);

    // Layout widths inside the inner area:
    //   id (id_width) + gap (1) + glyph (1) + gap (1) + project (10) + gap (2)
    //   = id_width + 15
    let left_width: usize = id_width + 15;
    const RIGHT_MARGIN: usize = 1;

    let glyph = if selected { "▌" } else { " " };

    let title_spans = highlight_query(title, query, title_style, accent_hl);

    let inner = inner_width as usize;
    let meta_width = unicode_width::UnicodeWidthStr::width(metadata.as_str());
    let title_max = inner.saturating_sub(left_width + meta_width + RIGHT_MARGIN + 1);
    let title_visible = unicode_width::UnicodeWidthStr::width(title).min(title_max);

    let used = left_width + title_visible;
    let padding = inner
        .saturating_sub(used + meta_width + RIGHT_MARGIN)
        .max(1);

    let mut header_spans: Vec<Span<'static>> = vec![
        Span::styled(id_text, dim),  // NEW — ID column
        Span::styled(" ", project_style),  // gap after ID
        Span::styled(glyph.to_string(), gutter_style),
        Span::styled(" ", project_style),
        Span::styled(format!("{:<10}", project), project_style),
        Span::styled("  ", project_style),
    ];
    header_spans.extend(truncate_spans(title_spans, title_max));
    header_spans.push(Span::styled(" ".repeat(padding), dim));
    header_spans.push(Span::styled(metadata.clone(), dim));
    let header_line = Line::from(header_spans);
```

Note: the previous `"  "` (2-col indent) at the start of the header is replaced by the ID column + 1-col gap. The total left chrome width changes from 16 (v3) to `id_width + 15` (which is 18 for `id_width = 3`).

Also update the **preview row** indent (was 4 cols) and **separator row** indent (was 2 cols) to align past the ID column. Find these two sections lower in the function:

```rust
    // ── Row 2 — preview ──────────────────────────────────────────
    let preview_indent = id_width + 4;  // was hardcoded 4
    let preview_max = inner.saturating_sub(preview_indent + RIGHT_MARGIN);
    let preview_text = conv.last_user_question.as_deref().unwrap_or("");
    let preview_truncated = truncate_str(preview_text, preview_max);
    let preview_line = Line::from(vec![
        Span::styled(" ".repeat(preview_indent), dim),
        Span::styled(preview_truncated, dim),
    ]);

    // ── Row 3 — separator ───────────────────────────────────────
    let sep_indent = id_width + 2;  // was hardcoded 2
    let sep_line = Line::from(vec![
        Span::styled(" ".repeat(sep_indent), separator_style),
        Span::styled(
            "─".repeat(inner.saturating_sub(sep_indent + 2)),
            separator_style,
        ),
    ]);
```

And the compact-mode separator (earlier in the function):

```rust
    if compact {
        let sep_indent = id_width + 2;
        let sep_line = Line::from(vec![
            Span::styled(" ".repeat(sep_indent), separator_style),
            Span::styled(
                "─".repeat(inner.saturating_sub(sep_indent + 2)),
                separator_style,
            ),
        ]);
        return vec![header_line, sep_line];
    }
```

- [ ] **Step 4: Update `render_list` to compute and pass the ID.**

Find `fn render_list` (search `grep -n "fn render_list\b" src/tui/ui.rs`). Above the items-building loop, compute `id_width`:

```rust
    let id_width = filtered.len().to_string().len().max(3);
```

Then update the `.enumerate().map(...)` to pass `i + 1` as id and `id_width`:

```rust
    let items: Vec<ListItem<'static>> = filtered
        .iter()
        .enumerate()
        .map(|(i, &conv_idx)| {
            let conv = &app.conversations()[conv_idx];
            let lines = list_row_lines(
                theme,
                conv,
                Some(i) == selected_idx,
                app.query(),
                compact,
                area.width,
                i + 1,        // NEW — 1-based ID
                id_width,     // NEW
            );
            ListItem::new(lines)
        })
        .collect();
```

- [ ] **Step 5: Run the tests, verify all pass.**

```bash
cargo build --quiet
cargo test --lib list_row_tests 2>&1 | tail -20
```

Expected: 9 tests pass (6 existing + 3 new).

- [ ] **Step 6: Full CI sweep.**

```bash
cargo fmt --all -- --check && echo "FMT CLEAN"
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean. If fmt complains, run `cargo fmt --all` and retry.

- [ ] **Step 7: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): per-row session ID column in list_row_lines"
```

---

## Task 2: Viewer search footer (`ViewerSearchActive` state)

**Files:**
- Modify: `src/tui/ui.rs` — `pub enum FooterState`, `pub fn footer_line`, and `fn render_view_mode`
- Test: same file, in `mod footer_tests`

- [ ] **Step 1: Write the failing test.**

Append to `mod footer_tests` in `src/tui/ui.rs`:

```rust
#[test]
fn viewer_search_active_shows_n_N_and_count() {
    let line = footer_line(
        &theme(),
        &FooterState::ViewerSearchActive { current: 3, total: 12 },
        /* compact = */ false,
    );
    let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(rendered.contains("n next"), "missing `n next`: {:?}", rendered);
    assert!(rendered.contains("N prev"), "missing `N prev`: {:?}", rendered);
    assert!(rendered.contains("3 / 12 matches"), "missing count: {:?}", rendered);
    assert!(rendered.contains("Esc close"), "missing Esc hint: {:?}", rendered);
}
```

- [ ] **Step 2: Run the test, expect compile failure.**

```bash
cargo test --lib viewer_search_active_shows_n_N_and_count 2>&1 | tail -10
```

Expected: `FooterState::ViewerSearchActive` doesn't exist yet.

- [ ] **Step 3: Add the new `FooterState` variant.**

In `src/tui/ui.rs`, find `pub enum FooterState<'a>` (around line 72). Add the new variant:

```rust
pub enum FooterState<'a> {
    ListIdle,
    Viewer,
    ViewerMessageNav,
    /// Viewer with in-conversation search active. Shows vim-style nav.
    ViewerSearchActive { current: usize, total: usize },
    StatusMessage(&'a str),
}
```

- [ ] **Step 4: Handle the new variant in `footer_line`.**

In the `match state { ... }` block of `pub fn footer_line`, add the new arm. Place it next to `Viewer` and `ViewerMessageNav`:

```rust
        FooterState::ViewerSearchActive { current, total } => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled("n", key),
            Span::styled(" next    ", desc),
            Span::styled("N", key),
            Span::styled(" prev    ", desc),
            Span::styled(format!("{} / {} matches", current, total), desc),
            Span::styled("    ", desc),
            Span::styled("Esc", key),
            Span::styled(" close", desc),
        ]),
```

(Style variables `key` and `desc` are already defined at the top of the function.)

- [ ] **Step 5: Run the test, verify it passes.**

```bash
cargo test --lib viewer_search_active_shows_n_N_and_count 2>&1 | tail -10
```

Expected: 1 passed.

- [ ] **Step 6: Wire the new variant into `render_view_mode`.**

Find `fn render_view_mode` in `src/tui/ui.rs` (search `grep -n "fn render_view_mode" src/tui/ui.rs`). Look for the existing `footer_state` match block:

```rust
    let footer_state = match app.status_message() {
        Some((msg, instant)) if instant.elapsed() < STATUS_TTL => {
            FooterState::StatusMessage(msg.as_str())
        }
        _ if state.message_nav_active => FooterState::ViewerMessageNav,
        _ => FooterState::Viewer,
    };
```

Replace with a version that prefers `ViewerSearchActive` when the viewer's search mode is `Active`:

```rust
    let footer_state = match app.status_message() {
        Some((msg, instant)) if instant.elapsed() < STATUS_TTL => {
            FooterState::StatusMessage(msg.as_str())
        }
        _ if state.search_mode == ViewSearchMode::Active
            && !state.search_matches.is_empty() =>
        {
            FooterState::ViewerSearchActive {
                current: state.current_match + 1,
                total: state.search_matches.len(),
            }
        }
        _ if state.message_nav_active => FooterState::ViewerMessageNav,
        _ => FooterState::Viewer,
    };
```

Notes on priority order:
- StatusMessage wins (3s TTL — explicit user feedback).
- ViewerSearchActive wins next when matches exist (current count + total).
- ViewerMessageNav next.
- Viewer (default) last.

The `state.current_match` is 0-based in the data model; we display 1-based to match `n / total` user-facing convention.

- [ ] **Step 7: Full CI sweep.**

```bash
cargo build --quiet
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 8: Commit.**

```bash
git add src/tui/ui.rs
git commit -m "tui(ui): viewer search footer exposes n/N navigation"
```

---

## Task 3: Remove mouse click-to-enter on the list

**Files:**
- Modify: `src/tui/app.rs` — `run_with_loader` event match arm (around line 2844) + delete `pub fn handle_list_click`

- [ ] **Step 1: Locate the mouse arm.**

```bash
grep -n "MouseButton::Left\|handle_list_click" src/tui/app.rs
```

There should be 2-3 hits: the arm in `run_with_loader` (around line 2844) and the `handle_list_click` definition (around line 2432).

- [ ] **Step 2: Read the existing arm.**

```bash
sed -n '2835,2855p' src/tui/app.rs
```

You'll see something like:

```rust
                Event::Mouse(m) => {
                    match m.kind {
                        MouseEventKind::ScrollDown => {
                            app.scroll_mouse(3, viewport_height);
                        }
                        MouseEventKind::ScrollUp => {
                            app.scroll_mouse(-3, viewport_height);
                        }
                        MouseEventKind::Down(MouseButton::Left)
                            if app.handle_list_click(m.row, frame_area) =>
                        {
                            app.enter_view_mode(content_width);
                            break;
                        }
                        _ => {}
                    }
                    continue;
                }
```

- [ ] **Step 3: Delete the `MouseEventKind::Down(MouseButton::Left)` arm.**

Replace the whole `Event::Mouse(m)` block with this slimmer version (keeps the two scroll arms, drops the click arm):

```rust
                Event::Mouse(m) => {
                    match m.kind {
                        MouseEventKind::ScrollDown => {
                            app.scroll_mouse(3, viewport_height);
                        }
                        MouseEventKind::ScrollUp => {
                            app.scroll_mouse(-3, viewport_height);
                        }
                        _ => {}
                    }
                    continue;
                }
```

- [ ] **Step 4: Delete the `handle_list_click` helper.**

Find `pub fn handle_list_click` in `src/tui/app.rs` (around line 2432) and delete the entire function (including its doc comment if any).

```bash
grep -n "pub fn handle_list_click" src/tui/app.rs
```

Use the line number to locate. The function is ~60 lines long. Delete from the `pub fn handle_list_click` line through its closing `}`.

- [ ] **Step 5: Verify no references remain.**

```bash
grep -rn "handle_list_click" src/ tests/
```

Expected: zero matches.

If matches remain (maybe a test or a `cfg(test)` use), look at each and either delete it or fix it. There shouldn't be any — the function was only used by the mouse arm we just removed.

- [ ] **Step 6: If `MouseButton` import is now unused, remove it.**

```bash
grep -n "MouseButton" src/tui/app.rs
```

If there's an import line like `use crossterm::event::{... MouseButton ...}` and no other use of `MouseButton`, remove it from the import list. Run the build — clippy or the compiler will tell you if it's unused.

- [ ] **Step 7: Full CI sweep.**

```bash
cargo build --quiet
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean. If clippy complains about an unused import, fix it (Step 6 covers `MouseButton`).

- [ ] **Step 8: Commit.**

```bash
git add src/tui/app.rs
git commit -m "tui(app): remove mouse click-to-enter on the list (Enter only)"
```

---

## Task 4: Persistent `ListState` for cursor-relative scrolling

**Files:**
- Modify: `src/tui/app.rs` — add `list_state: RefCell<ListState>` field, accessor, and sync calls
- Modify: `src/tui/ui.rs` — `render_list` accepts a `&mut ListState`; `render_list_mode` passes `&mut app.list_state().borrow_mut()` (or equivalent)

This is the biggest task by line count but conceptually simple: stop creating a fresh `ListState` per render, persist it on `App`, and keep it in sync when selection changes.

- [ ] **Step 1: Add the field and imports to `App`.**

In `src/tui/app.rs`:

```bash
grep -n "^use\|^pub struct App\b" src/tui/app.rs | head -20
```

At the top of the file, ensure these imports are present (add what's missing):

```rust
use ratatui::widgets::ListState;
use std::cell::{RefCell, RefMut};
```

In the `pub struct App { ... }` block, after the last field, add:

```rust
    /// Persistent ratatui list state. Survives across renders so the
    /// list's scroll offset doesn't reset to 0 each frame — that was
    /// the bug where moving up after scrolling moved the page instead
    /// of the cursor.
    list_state: RefCell<ListState>,
```

- [ ] **Step 2: Add the field initializer to all three constructors.**

```bash
grep -n "fn new\b\|fn new_loading\|fn new_single_file" src/tui/app.rs
```

In each constructor's `Self { ... }` block, add (at the end, before the closing brace):

```rust
            list_state: RefCell::new(ListState::default()),
```

- [ ] **Step 3: Add the public accessor.**

In the `impl App { ... }` block, near the other simple accessors, add:

```rust
    /// Mutable accessor for the persistent ListState — used by the
    /// renderer in `render_list`. Borrowed mutably for the duration of
    /// a single render call; safe because the TUI is single-threaded.
    pub fn list_state(&self) -> RefMut<'_, ListState> {
        self.list_state.borrow_mut()
    }
```

- [ ] **Step 4: Sync `list_state` with `self.selected` in all the mutators.**

Find every place that writes to `self.selected`:

```bash
grep -n "self.selected = " src/tui/app.rs
```

There are roughly 10-12 sites. For EACH site, immediately after the assignment, add a sync call. The pattern:

```rust
            self.selected = Some(idx);
            // Keep the persistent list state in sync.
            self.list_state.borrow_mut().select(self.selected);
```

For sites that set `None`:

```rust
            self.selected = None;
            self.list_state.borrow_mut().select(None);
```

Sites to update (rough list — verify with the grep):
- `append_conversations` (when `self.selected.is_none()` becomes `Some(0)`)
- `finish_loading` (the new selection assignment)
- The `update_filter` flow (selected becomes 0 or None)
- `select_prev`, `select_next`, `select_first`, `select_last`, `select_page_up`, `select_page_down`, `select_half_page_down`, `select_half_page_up`
- `remove_selected_from_list` (if it sets selected)
- `find_or_load_uuid` (if it sets selected)
- Any other `self.selected =` site

**Strategy**: do a single grep, then add the sync call after each assignment. There's no behavior subtlety — it's mechanical.

- [ ] **Step 5: Reset `list_state` on filter changes and refresh.**

When the displayed set of items changes underneath the list, the offset is meaningless and should reset to 0. The two places this matters are filter changes (search query updated) and the F5 refresh flow.

For filter changes — find `fn update_filter` or wherever the search query reapplies. After the line `self.filtered = filtered;` (or equivalent), and before/after setting `self.selected`, add:

```rust
            // Filter changed — reset the list's scroll offset so the new
            // top is visible. (Calling .select() below also adjusts offset
            // to keep selection visible, but we want offset=0 for a clean
            // top-of-list view.)
            *self.list_state.borrow_mut() = ListState::default();
            self.list_state.borrow_mut().select(self.selected);
```

For the F5 refresh — find `begin_refresh`. At the end of the function:

```rust
            *self.list_state.borrow_mut() = ListState::default();
            self.list_state.borrow_mut().select(self.selected);
```

(`begin_refresh` clears conversations and resets selected — we want the same for list_state.)

- [ ] **Step 6: Update `render_list` to take a `&mut ListState`.**

In `src/tui/ui.rs`, find `fn render_list`. The signature today:

```rust
fn render_list(frame: &mut Frame, app: &App, area: Rect, compact: bool) {
```

Change to (the caller will need to pass the borrow):

```rust
fn render_list(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    compact: bool,
    list_state: &mut ListState,
) {
```

Replace this block in the body:

```rust
    let mut state = ListState::default();
    state.select(selected_idx);
    frame.render_stateful_widget(list, area, &mut state);
```

with:

```rust
    list_state.select(selected_idx);
    frame.render_stateful_widget(list, area, list_state);
```

- [ ] **Step 7: Update `render_list_mode` to thread the borrow through.**

Find `fn render_list_mode` (search `grep -n "fn render_list_mode" src/tui/ui.rs`). Find the `render_list(frame, app, chunks[..], compact);` call (around the end of the function).

Just before the call, take the borrow:

```rust
    let mut list_state = app.list_state();
    render_list(frame, app, chunks[4], compact, &mut list_state);
```

The `RefMut` will be dropped at the end of the function, releasing the borrow.

If `chunks[..]` index isn't `4` in the v3 layout, use whichever index is the "list" chunk in your current `render_list_mode`. Confirm by reading the layout:

```bash
grep -n "render_list(frame, app" src/tui/ui.rs
```

- [ ] **Step 8: Build the binary.**

```bash
cargo build --quiet 2>&1 | tail -10
```

Clean. If errors:
- Missing import of `ListState` in `app.rs` → add it (Step 1).
- Borrow checker complaint about `self.list_state.borrow_mut()` → ensure no other borrow is held at the same line; in particular, `self.selected` is `Option<usize>` (not a reference) so this should be fine.

- [ ] **Step 9: Run tests and CI.**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 10: Manual smoke test (best-effort).**

```bash
./target/debug/ccmanager
```

The user's exact reproducer:
1. Top of the list. Press `↓` to move the cursor down through the visible rows.
2. Keep pressing `↓` — the cursor reaches the visual bottom of the viewport, then the page scrolls (cursor stays at visual bottom).
3. Now press `↑`. **The cursor should move UP within the visible viewport** (not the page going up).
4. Keep pressing `↑` — the cursor walks up through the viewport.
5. When the cursor reaches the visual top of the viewport, the next `↑` makes the page scroll up.

If step 3 still moves the page instead of the cursor, the persistence isn't working — re-check that `list_state` is a struct field (not a local) and that `render_list` mutates the borrow rather than reassigning a fresh state.

- [ ] **Step 11: Commit.**

```bash
git add src/tui/app.rs src/tui/ui.rs
git commit -m "tui: persist ListState across renders for free-cursor scrolling"
```

---

## Task 5: Wrap-up — CHANGELOG, CI sweep, snapshot to `main`

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Confirm `dev` and CI green.**

```bash
git rev-parse --abbrev-ref HEAD   # must print `dev`
cargo fmt --all -- --check && echo "FMT CLEAN"
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
cargo test --quiet 2>&1 | grep "test result"
```

All clean.

- [ ] **Step 2: Add CHANGELOG entry.**

Open `CHANGELOG.md`. Under `## Unreleased` (above the v3 bullets), add:

```markdown
- **TUI v4 list affordances.** Four small refinements to v3:
  - Each visible list row gets a right-aligned dim ID column
    (1, 2, … N) reflecting filtered list order — easy to scan and
    refer to ("the 3rd one").
  - The viewer's in-conversation search (`/` + type + Enter + `n`/`N`)
    now surfaces its vim-style navigation in the footer: `n next  ·
    N prev  ·  M / N matches  ·  Esc close`.
  - Mouse left-click on a list row no longer enters the session;
    only Enter does. Mouse-wheel scrolling is unchanged. Fixes the
    "I keep mis-clicking and accidentally opening sessions" pain
    point.
  - Cursor-vs-page scrolling fixed: the selected row now moves
    freely within the visible viewport, and the page only scrolls
    when the cursor crosses the top or bottom edge. (Root cause:
    `ListState` was rebuilt fresh each render, so ratatui's
    auto-scroll always pinned the cursor to the bottom when the
    selection was below the viewport. The state now persists across
    renders.)
  See `docs/superpowers/specs/2026-05-23-tui-v4-list-affordances-design.md`
  for details.
```

- [ ] **Step 3: Commit the CHANGELOG.**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): TUI v4 list affordances"
```

- [ ] **Step 4: Snapshot to `main`.**

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
git log --oneline main          # ONE commit
git diff dev main --stat        # empty (trees match)
git rev-parse --abbrev-ref HEAD # dev
./target/debug/ccmanager --version
```

`ccmanager --version` reports `ccmanager 1.0.0`.

---

## Self-review notes

I reviewed the plan once with fresh eyes against the spec:

- **§1 covered** by Task 1 (ID column rendering + new tests + caller updates).
- **§2 covered** by Task 2 (`FooterState::ViewerSearchActive` variant, `footer_line` branch, `render_view_mode` selects it).
- **§3 covered** by Task 3 (delete mouse arm + delete `handle_list_click`).
- **§4 covered** by Task 4 (RefCell field, sync at every `self.selected =` site, render through `&mut ListState`).
- **§5 covered** by all tasks together.

Adjustments made inline during review:

- **Indent math in `list_row_lines`** — initially I had the new ID column add 3 cols on top of v3's 2-col indent. The header would have become `"   1  ▌ project..."` (3-col ID + 2-col gap + glyph). That double-indents the gutter bar away from the ID, which looked awkward. Switched to `"  1 ▌ project..."` (3-col ID, 1-col gap, then glyph) — the ID column REPLACES the leading 2-col indent rather than being prepended to it. The plan's `left_width = id_width + 15` accounts for this.

- **Compact mode separator indent** — the v3 compact-mode separator used a hardcoded `"  "` (2-col indent) + a `"─".repeat(inner - 4)` rule. After ID indenting, the separator's left indent becomes `id_width + 2` and the rule length becomes `inner.saturating_sub(id_width + 4)`. Same change as for normal-mode separator; both updated in the same step.

- **Filter-change ListState reset (Step 5 of Task 4)** — initially I had the reset only on F5 refresh. Then realized: when search filters the list down, the selected index resets to 0, but if `list_state.offset` was at 47, ratatui's auto-scroll would then move offset back to 0 anyway (since 0 < 47). So technically the explicit reset is redundant in the "selected goes to 0" case. BUT — there are edge cases where `selected` stays the same numeric value but refers to a different conversation after filtering. Safer to reset offset to 0 on filter change to avoid surprising scroll positions. Kept the reset.

- **Borrow checker for `app.list_state()`** — the accessor returns `RefMut<'_, ListState>`. The render_list_mode call site does:
  ```rust
  let mut list_state = app.list_state();
  render_list(frame, app, chunks[4], compact, &mut list_state);
  ```
  The `RefMut` is dropped at end of scope. Inside `render_list`, we pass `list_state` (the RefMut) to `render_stateful_widget`. Wait — `render_stateful_widget` takes `&mut State`, and `RefMut` derefs to `&mut T`. So passing `&mut *list_state` or just `list_state.deref_mut()` works. Actually the cleanest is: `render_list(... &mut list_state)` where `&mut list_state` is `&mut RefMut<ListState>` — that doesn't deref. Better: change render_list's param to take `&mut ListState` directly and call site does `&mut *list_state`:
  ```rust
  let mut list_state = app.list_state();
  render_list(frame, app, chunks[4], compact, &mut *list_state);
  ```
  Updated Step 7 to reflect this. The `*` derefs the `RefMut<ListState>` to `ListState`; `&mut` re-borrows mutably.

  Actually wait — `RefMut<T>` already implements `DerefMut`, so `&mut *list_state` is the standard idiom. Yes, this is correct.

No spec gaps. No type inconsistencies (function signatures all line up: `list_row_lines` 8 args matching across def and call sites; `render_list` adds `&mut ListState` param; `FooterState::ViewerSearchActive` has the same fields everywhere).
