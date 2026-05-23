# ccmanager TUI v3 — Framed Hybrid

**Status:** approved (brainstorming round, 2026-05-23)
**Scope:** the terminal interface (list and viewer screens), not the web UI.
**Predecessor spec:** `2026-05-22-tui-visual-polish-design.md` (v1
"Modern Minimal" — currently shipped on `dev` and `origin/main`).

## Goal

Bring back the **outer rounded frame** and the **dense 3-line list
entries with conversation previews** the user missed from the
pre-polish layout, while keeping the parts of v1 that worked well
(search-as-you-type accent highlight, cool-blue palette, the refined
shared `render_modal` for the four dialogs, the brand-marked header).

Where v1 traded chrome for whitespace, v3 trades whitespace for
information density.

## Goals and non-goals

**Goals**

- An outer rounded `Block` wraps the whole list and viewer screens.
- The brand line (`◈ ccmanager · scope · count`) moves into the
  block's **top title strip**; the key-hint footer moves into the
  block's **bottom title strip**. No standalone bands.
- List entries are three rows: header (project + title + right-aligned
  metadata), preview (single-line snippet of the conversation),
  separator (thin `─` rule).
- Selection indicator is the gutter bar `▌` (accent color) on the
  selected row's first line, paired with a subtle background tint
  across all three of the selected row's content rows.
- Inline metadata on the entry header is `<N>msg · <age>` (right-aligned).
- Viewer mode receives the same outer frame, with the viewer-specific
  header (`◈ ccmanager · session <12-char-id> · <N> turns`) in the
  top title and viewer-specific key hints in the bottom title.

**Carried forward from v1 (unchanged)**

- Cool-blue accent (`#6cb8ff` dark / `#0a6fc7` light), `text_tertiary`
  token, palette tests.
- `render_modal()` and the four modals it serves
  (confirm/export/rename/help).
- Search highlighting via `highlight_query()` — accent color (bold)
  applied to matched substrings in the title and project name.
- Cursor positioning at the end of the search query in list mode.
- Speaker labels (`You`/`Claude`) in accent color inside the viewer.
- Inline find bar (`find ▸ <query>     M / N matches`) for in-viewer
  search.

**Non-goals**

- No new keybindings.
- No new dependencies.
- No animations, no transitions.
- No web UI changes.
- No new fields on `Conversation` (`Conversation.preview_last` is
  already populated by the loader; we use what's there).

## Detailed design

### §1 — Composition

The list screen is a single rounded `Block` wrapping the whole
`frame.area()`. Inside the block, top to bottom:

1. Search input row (1 line) — inline `  search ▸ <query>` styled
   line with the cursor positioned at the query's end.
2. Top rule row (1 line) — a thin `─` separator across the inner
   width.
3. List rows — three rows per entry: header / preview / separator.

The block's **`title`** (top border, left-aligned) carries the brand
line: `◈ ccmanager · <scope> · <count metadata>`. The block's
**`title_bottom`** (bottom border, left-aligned) carries the
context-aware key-hint footer. Both reuse the v1 helpers
(`header_line` and `footer_line`) — they just no longer get rendered
as their own bands; their output is fed into the block's title.

```
╭─ ◈ ccmanager · all projects · 47 sessions ──────────────╮
│  search ▸ deploy                                         │
│ ────────────────────────────────────────────────────── │
│  ▌ ccmanager   Add F5 refresh           47msg · 2h ago  │
│    how do I get F5 to reload conversations live, w…      │
│  ─────────────────────────────────────────────────     │
│    work        Deploy strategy        23msg · yesterday  │
│    should we deploy on Tuesday or wait for the relea…    │
│  ─────────────────────────────────────────────────     │
│    notes       Email template          8msg · yesterday  │
│    write me a welcome email for the new hire on…         │
╰─ ↑↓ nav  · / search  · ⏎ view  · ^R resume  · ? help ──╯
```

Status messages temporarily override the key-hint title (3-second
TTL — same mechanism v1 already implements). The header title
similarly switches between three states:

- Idle: `◈ ccmanager · all projects · 47 sessions`
- Searching: `◈ ccmanager · all projects · 5 / 47 sessions match`
- Loading: `◈ ccmanager · all projects · loading… 12 so far`

### §2 — Header line content (block top title)

Same `header_line()` helper as v1 produces this. No code changes to
the helper. The integration changes: instead of rendering its output
as a `Paragraph` in a dedicated row, the output line is passed to
`Block::title(Line::from(...))`.

`ratatui` 0.30's `Block::title` accepts a styled `Line`, so the
multi-span styling (accent for `◈ ccmanager`, primary for scope, dim
for metadata) is preserved.

### §3 — List entries — 3 lines each

```
  ▌ ccmanager   Add F5 refresh           47msg · 2h ago
    You: how do I get F5 to reload conversations…
  ──────────────────────────────────────────────────
```

- **Row 1 — header**: 2-col indent, selection glyph column (1 col:
  either `▌` accent for selected, space for unselected), 1-col gap,
  project name (10-char fixed width, padded with spaces, dim), 2-col
  gap, title (primary text, bold when selected), spaces filling to
  the right margin, right-aligned metadata `<N>msg · <age>` (all dim,
  middle-dot separator).
- **Row 2 — preview**: 4-col indent, single line of preview text
  (dim). Source: the **last user question** in the conversation —
  i.e. the most recent user-authored message. A new field
  `Conversation.last_user_question: Option<String>` is added and
  populated by the parser (`user_messages.last().cloned()` —
  `user_messages` is already collected during parsing; we just expose
  its tail). No `You:` prefix; the row's position and dim styling
  make its role clear. Truncated to the row width minus the indent
  and a 1-col trailing margin; truncation appends `…`. When the
  conversation has no user messages (rare edge case — e.g. a
  conversation deleted before any user input), the row renders empty.
- **Row 3 — separator**: 2-col indent, thin `─` rule across the
  remaining row width, rendered in `theme.separator`.

**Search highlighting** applies to the title and project name on row
1 only — preview text stays clean. `highlight_query()` from v1 is
reused unchanged.

**Selection**:
- The `▌` gutter bar lives in the glyph column on row 1.
- All three rows of the selected entry get a background tint
  (`theme.selection_bg`), so the row reads as "present" even if the
  bar scrolls past.
- `List` widget's `highlight_style` provides the tint; the gutter bar
  is foreground content emitted by the row builder when `selected`
  is true.

### §4 — Search input row

Same as v1: inline `  search ▸ <query>` (with placeholder
`(fuzzy across all transcripts)` when empty). Cursor positioned at
the query's display-width end. `search_line()` helper unchanged.

The row lives inside the block, on the first content line. No box
around it.

### §5 — Per-row metadata

Right-aligned on row 1: `<N>msg · <age>`.

- `<N>msg` — total message count (`Conversation.message_count`).
- `<age>` — relative time using the existing `format_timestamp()`
  helper (`"2h ago"`, `"yesterday"`, `"3 days ago"`).
- Middle-dot (`·`) separator, all in `theme.text_muted`.

Not shown: model name, token count, duration. Carry minor information
density at the cost of row width; user can `--render` for the full
JSONL details.

The metadata is truncated/dropped (not the title) when the row is
too narrow — the title's information value is higher than the
metadata's.

### §6 — Selection indicator

Switch from v1's chevron (`◆`) back to the gutter bar (`▌`), accent
color, in the leftmost glyph column.

The background tint stays (gives "present" feedback even when the bar
glyph is off-screen during search-input typing).

The `▌` glyph occupies a single column, U+258C "Left half block". On
unselected rows, that column is a plain space.

### §7 — Inter-item separator

Each entry's row 3 is the separator: 2-col indent, then `─` repeated
across the row to a 2-col right margin. Color `theme.separator` (the
faintest visible grey).

This visually separates entries without committing the full vertical
space of a blank row.

### §8 — Viewer mode

Same outer-frame treatment. The block's top title carries the
viewer-specific header: `◈ ccmanager · session <12-char-id> · <N>
turns`. The bottom title carries viewer key hints (different from list
mode — uses `FooterState::Viewer` or `FooterState::ViewerMessageNav`).

Inside the block (top to bottom):

1. Rendered ledger content (the existing `render_view_content`,
   unchanged) — fills the available height.
2. When search-typing mode is active: the inline find bar
   (`find ▸ <query>     M / N matches`) renders on the **last
   content row inside the frame**, just above the bottom border —
   same as v1.

Status messages override the bottom title for 3 seconds the same way
they do on the list screen.

### §9 — Modals (export menu, rename, confirm, help)

**Unchanged from v1.** The shared `render_modal()` helper and its four
callers stay as they are. The modals sit on top of the outer frame;
the visual stacking continues to work because modals draw their own
rounded `Block` with `Clear` first.

### §10 — Palette

**Unchanged from v1.** Cool blue accent, restrained greys,
`text_tertiary`, palette tests.

### §11 — Compact-mode fallback

For terminals smaller than 60 cols or 20 rows:

- The outer frame **stays** (its border row + key-hint row contribute
  2 rows; the frame is part of the brand and shouldn't disappear).
- List entries collapse from 3 lines to **2 lines** — the preview
  row is dropped, but the header and separator rows stay. Metadata
  remains right-aligned on row 1.
- Bottom key-hint title shortens to `↑↓  /  ⏎  ?` (the four most
  important keys).
- Top brand title is preserved as-is.

This is a different fallback than v1 (which collapsed to 1 line).
Two lines keeps the metadata visible, which is more useful than the
preview when space is tight.

### §12 — Empty / loading / no-history states

- **Loading**: frame's top title reads `◈ ccmanager · all projects ·
  loading… <N> so far` (the existing `HeaderState::Loading` carries
  this; no change). The inside of the frame is the search input + an
  otherwise empty list area.
- **No matches** (query active, zero filtered): centered dim two-line
  message inside the frame:
  ```
        no conversations match your search
              press Esc to clear it
  ```
- **No history at all** (empty `~/.claude/projects/`): same centered
  treatment, message reads `you don't have any Claude Code
  conversations yet`.

### §13 — Status messages

3-second TTL (`STATUS_TTL` const, unchanged). When active, the message
replaces the bottom key-hint title text (rendered in accent color).
Mechanism unchanged from v1.

### §14 — Project name uses the LATEST cwd, not the first

**Bug fix.** Today, `parse_conversation_file()` in
`src/history/parser.rs:122` extracts the cwd from the *first* user
message that carries one and ignores all subsequent cwd values. When
a user renames their project directory (or moves the project)
*during* a live session, every later JSONL entry records the new
path, but the cached "first cwd" wins. The list then displays the
project under its **old** name.

This is exactly what happens in the very session that's rendering
ccmanager itself: the folder was renamed from `claude-history` to
`ccmanager` mid-session, but the list still shows the active session
under `claude-history` because that was the cwd in the JSONL's first
user entry.

The fix is one-shot in the parser: scan every user entry's cwd and
**keep the most recent non-empty value**, instead of latching the
first one. Concrete change at `src/history/parser.rs:122`:

```rust
// Before:
// Extract cwd from the first user message that has it
if extracted_cwd.is_none()
    && let Some(cwd_str) = cwd
{
    extracted_cwd = Some(PathBuf::from(cwd_str));
}

// After:
// Extract cwd from the latest user message that has it.
// Each entry's cwd reflects where `claude` was running at the time;
// the latest one is the most current (handles mid-session rename).
if let Some(cwd_str) = cwd {
    extracted_cwd = Some(PathBuf::from(cwd_str));
}
```

The downstream `format_short_name_from_path()` and existing
fallback-to-decoded-name logic both already handle the "cwd points
at a non-existent directory" case; no further changes needed.

A unit test is added asserting the latest cwd wins when a file has
multiple user entries with different cwd values.

## Implementation notes

The bulk of the work is in `src/tui/ui.rs`. Touch points:

- `render_list_mode` (currently uses standalone band layout) — rewrite
  to wrap the screen in a single rounded `Block` whose top and bottom
  title strips carry the header and footer `Line`s. Drop the
  horizontal-rule bands.
- `render_view_mode` — same shape: one outer block, viewer-specific
  title strips, content fills the inside.
- `list_row_lines()` — extend to produce **three** lines per entry
  (header / preview / separator) in normal mode, and **two** lines
  in compact mode (header + separator, no preview). Title row gains
  the right-aligned metadata; new preview row reads the new
  `Conversation.last_user_question` field (§3); separator row
  renders the thin `─` rule. The current 5-arg signature
  (`theme, conv, selected, query, compact`) gains a 6th arg
  `inner_width: u16` so the metadata can be right-aligned to the
  row width inside the frame. Pass `inner_area.width` from the
  caller.
- `is_compact_layout()` — unchanged; thresholds still 60×20.
- `render_modal()` and modal callers — unchanged.
- `viewer.rs` — speaker label coloring unchanged from v1.
- `theme.rs` — unchanged.
- `src/history/mod.rs` — add `pub last_user_question: Option<String>`
  to the `Conversation` struct.
- `src/history/parser.rs` —
  - Populate `last_user_question` from `user_messages.last().cloned()`
    when constructing the `Conversation` (§3).
  - Change the cwd extraction at line ~122 to keep the latest cwd,
    not the first (§14).

The `header_line()` and `footer_line()` helpers stay pure (return a
`Line`); the callers just pass that `Line` to
`Block::title(...)` / `Block::title_bottom(...)` instead of rendering
it as a `Paragraph` in a dedicated row.

Several v1 functions become trivially smaller after this round
(`render_list_mode` shrinks because it no longer manages 7 separate
chunks; just 3: top-of-content / list / bottom-of-content inside one
block). The horizontal-rule helper used between v1's bands can be
deleted.

## Test plan

- Unit test for the new `list_row_lines()`:
  - Normal mode returns 3 lines (header + preview + separator) — new
    test.
  - Compact mode returns 2 lines (header + separator, no preview) —
    new test.
  - Right-aligned metadata fits the row width — new test (assert the
    rendered span sequence ends with the metadata, with at least one
    space between the title and the metadata).
  - Preview row shows the conversation's last user question —
    construct a fixture with `last_user_question = Some("how do I
    refresh the list?")`, assert the second line contains that
    text.
- Unit test for the parser's cwd extraction (§14):
  - JSONL fixture with two user entries: first has
    `cwd: "/old/path"`, second has `cwd: "/new/path"`. Assert the
    parsed `Conversation.cwd == Some(PathBuf::from("/new/path"))`.
- Unit test for the parser's `last_user_question` population (§3):
  - JSONL fixture with three user messages "hi", "what's up", "last
    question". Assert
    `Conversation.last_user_question == Some("last question")`.
- Existing tests for `header_line` / `footer_line` / `search_line` /
  `highlight_query` continue to apply unchanged.
- Manual smoke test: launch `ccmanager`, browse list, type a query,
  verify highlight pops on title + project name, verify preview row
  shows the most recent user question for each conversation, verify
  the active session shows under `ccmanager` (not `claude-history`),
  verify selected row has gutter bar + background tint across all 3
  rows, verify viewer screen has the same framed shape.

## Open questions

None — all major decisions resolved during brainstorming.

## Out of scope

- Adding a resume-count to `Conversation` (still deferred from v1).
- Underline rule under the modal title (still deferred from v1).
- Cleaning up the ~13 dead palette tokens in `theme.rs` (separate
  pass).
- Cleaning up the `#[allow(dead_code)]` markers on
  `format_model_name` / `format_tokens` / `format_tokens_long`
  (separate pass — they could be resurrected if we decide to surface
  model info in the metadata in a future round).
