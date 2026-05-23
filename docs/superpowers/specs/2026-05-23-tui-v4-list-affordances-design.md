# ccmanager TUI v4 — list affordances (IDs, no-click, free cursor, search-footer)

**Status:** approved (brainstorming round, 2026-05-23)
**Scope:** the terminal interface (list mode + viewer mode). No web UI changes.
**Predecessor spec:** `2026-05-23-tui-framed-hybrid-design.md` (v3 — currently shipped).

## Goal

Four small refinements to the list and viewer interactions, motivated by the
user's day-to-day frustrations with v3:

1. Number each visible list row 1..N so they're easy to refer to in
   conversation ("the 3rd one") and to scan.
2. Make the viewer's vim-style in-conversation search discoverable — the
   footer doesn't currently surface `n`/`N` for cycling matches.
3. Stop mouse clicks from entering a session (cursor-only navigation),
   because the user keeps mis-clicking.
4. Fix the cursor-vs-page interaction in the list: the cursor should move
   freely within the visible viewport and only trigger a page scroll when
   it crosses an edge.

## Goals and non-goals

**Goals**

- Each visible list entry shows a per-row ID column (1, 2, ..., N) in the
  leftmost columns of its header row, dim, right-aligned.
- The viewer's footer shows `n next · N prev · M / N matches · Esc close`
  whenever `search_mode == Active`.
- Left-mouse-button clicks in list mode do nothing. Mouse-wheel scrolling
  is unchanged.
- The list's selected row is persisted across renders so ratatui's
  `List` widget keeps the scroll offset stable — i.e. the cursor moves
  freely within the visible viewport, and only crossing the viewport edge
  shifts the page.

**Non-goals**

- No keyboard shortcut to jump directly to a session by ID (e.g. `5` to
  open session 5). The IDs are purely for visual reference in this round.
- No change to the existing `n`/`N` cycling behavior or to `/` in the
  viewer. Only the footer presentation changes for (2).
- No new ratatui dependency, no new features, no animations.
- No change to mouse-wheel scrolling or to mouse interactions outside the
  list (the viewer's mouse wheel still scrolls the transcript).

## Detailed design

### §1 — Per-row session IDs

Each visible list entry gets a per-row ID prepended to its header line.
IDs reflect the **filtered list order** (top-down: 1 is the top visible
row, regardless of whether a search filter is active). Applying or
changing a search query re-numbers from 1.

Layout — the header row gains a 3+ col ID column before the gutter bar:

```
  ID  GUTTER  PROJECT      TITLE                METADATA
  ↓   ↓       ↓            ↓                    ↓
   1  ▌ ccmanager  Add F5 refresh           47msg · 2h ago
        he came back with a fix for the bug…
      ─────────────────────────────────────────────────────
   2     work       Deploy strategy        23msg · yesterday
        should we deploy on Tuesday or wait…
      ─────────────────────────────────────────────────────
```

- ID width = `digits(filtered.len()).max(3)`. So a list of 5 conversations
  still uses a 3-col ID column ("  1", "  2", …) — keeps the layout
  stable as the list grows.
- IDs render in `theme.text_muted` (dim).
- Rows 2 (preview) and 3 (separator) indent past the ID column to keep
  alignment.
- `list_row_lines` gains an `id: usize` parameter (1-based). The caller
  in `render_list` passes the position in the filtered list +1.

The metadata's right-alignment math (the `padding` calc in v3's
`list_row_lines`) accounts for the new column: `LEFT_WIDTH` becomes
`id_width + 1 (sep) + 1 (glyph) + 1 (gap) + 10 (project col) + 2 (gap)`,
i.e. `id_width + 15`. For `id_width = 3` (default), `LEFT_WIDTH = 18`.

Compact mode behaves the same — the ID column stays, the row is 2 lines
instead of 3, separator row uses the same width math.

### §2 — Viewer search footer

The viewer's vim-style in-conversation search is already implemented
correctly (`/` → typing, `Enter` → Active, `n`/`N` → cycle, `Esc` →
close). The only gap is discoverability: the footer's `Viewer` state
shows generic hints (`↑↓ scroll · / search · e copy · r rename · q back
· ? help`) and doesn't tell the user `n`/`N` works.

Add a new `FooterState::ViewerSearchActive { current: usize, total: usize }`
variant to `footer_line()`. When the viewer's `search_mode == Active`,
`render_view_mode` selects this variant instead of `FooterState::Viewer`.

Rendered output:

```
  n next  ·  N prev  ·  3 / 12 matches  ·  Esc close
```

- `n` and `N` in `text_primary`, descriptions in `text_muted`.
- The `3 / 12 matches` segment uses the `text_muted` style throughout —
  it's informational, not actionable.
- Middle dots separate the segments.

The footer-state priority remains: `StatusMessage` (3s TTL) >
`ViewerSearchActive` > `ViewerMessageNav` > `Viewer`.

### §3 — Mouse click no longer enters a session

In `run_with_loader`'s event match (`src/tui/app.rs:2844-2849`), remove
the entire `MouseEventKind::Down(MouseButton::Left)` arm:

```rust
// REMOVE this block:
MouseEventKind::Down(MouseButton::Left)
    if app.handle_list_click(m.row, frame_area) =>
{
    app.enter_view_mode(content_width);
    break;
}
```

Mouse-wheel scrolling (`ScrollUp` / `ScrollDown`) stays untouched. Left
clicks become no-ops in list mode. The viewer's own mouse handling is
unaffected.

`App::handle_list_click()` becomes unreachable. Delete it (don't leave
it `#[allow(dead_code)]`'d). It's a non-trivial helper (~60 lines of
geometry math) that was only used by the removed mouse arm.

### §4 — Persisted `ListState` for cursor-relative scrolling

#### Root cause

`render_list` (`src/tui/ui.rs:1316-1371`) builds a fresh
`ListState::default()` on every render and calls `state.select(selected_idx)`.
The fresh state has `offset = 0`. Then ratatui's `List` widget's auto-scroll
fires:

- If `state.selected < state.offset` → set `offset = selected` (cursor at
  TOP of viewport).
- If `state.selected >= offset + items_fit` → set
  `offset = selected - items_fit + 1` (cursor at BOTTOM of viewport).
- Else: offset stays.

With offset always reset to 0 at render-time, the second branch always
fires when `selected >= items_fit`, pinning the cursor to the bottom of
the viewport. The user then moves the cursor up; selection decrements;
re-render starts at `offset = 0`; ratatui re-pins to the bottom.

#### Fix

Persist `ListState` as a field on `App`. The state's `offset` survives
re-renders, so ratatui's auto-scroll only kicks in when the cursor
genuinely crosses a viewport edge — which is the standard "free cursor
within viewport" behavior the user wants.

**Implementation choice**: `list_state: RefCell<ListState>` so that the
existing `render_list(frame, app: &App, area, compact)` signature
doesn't have to change to `&mut App`. The render function does a single
`borrow_mut()` to update `ListState`'s selected (and pass it as
`&mut` to `render_stateful_widget`), then drops the borrow. Single-
threaded code; no contention.

Selection-driving methods (`select_next`, `select_prev`, `select_first`,
`select_last`, `select_page_up`, `select_page_down`,
`select_half_page_down`, `select_half_page_up`,
`update_filter`-and-friends, `remove_selected_from_list`) all update
`self.selected: Option<usize>` AND call
`self.list_state.borrow_mut().select(Some(idx))` to keep the persistent
state in sync.

When the filter changes (search query updated) the selected index resets
to 0 (existing behavior), and the persistent state's offset is also
reset to 0 (since the visible set just changed and we want the new top
to be visible).

When the conversations vec is cleared and reloaded (the `F5` refresh
flow), the state is reset to default and re-seeded with the restored
selection.

#### Behavior after the fix

- Open ccmanager, viewport shows rows 1..20.
- Press `↓` 19 times: cursor moves from row 1 to row 20 inside the
  viewport. No scroll.
- Press `↓` once more (selected=21): `state.selected = 20`, `state.offset`
  was 0, ratatui adjusts `offset` to 1. Cursor still appears at the
  bottom row of the viewport.
- Press `↓` more: each step advances offset by 1; cursor stays at the
  visual bottom row.
- Press `↑` once: `state.selected = N - 1`, `state.offset` stays at the
  same value because `selected >= offset && selected < offset + items_fit`.
  Cursor moves up by one row within the viewport. **This is the user's
  requested behavior.**
- Continue pressing `↑`: cursor walks up through the viewport.
- When cursor reaches the visual top (selected == offset), pressing `↑`
  makes selected < offset, ratatui adjusts offset down by 1 to keep
  selection at the top. The cursor stays at the visual top while the
  page scrolls upward.

## Implementation notes

Files touched:

- `src/tui/ui.rs` —
  - `list_row_lines` gains an `id: usize` parameter. Render the ID as a
    right-aligned dim numeral at the start of the header line, width =
    `id_width` (computed from the caller's max ID).
  - `render_list` accepts a `&mut ListState` parameter and uses it
    instead of constructing a fresh `ListState::default()`. Caller is
    `render_list_mode`; pass `&mut app.list_state.borrow_mut()`.
  - The 2- and 3-line preview/separator rows also indent past the ID
    column (currently 4-col indent for preview, 2-col for separator —
    they become `id_width + 4` and `id_width + 2`).
  - `footer_line` gains a new `FooterState::ViewerSearchActive` arm.

- `src/tui/app.rs` —
  - New field: `list_state: RefCell<ListState>` on `App`. Default-
    initialized in all three constructors (`new`, `new_loading`,
    `new_single_file`).
  - Add `pub fn list_state(&self) -> std::cell::RefMut<'_, ListState>`
    so the renderer can mutate it through `&App`.
  - All `select_*` methods that mutate `self.selected` also call
    `self.list_state.borrow_mut().select(Some(idx))`. There's no
    equivalent for `select_first` setting to 0 — the state's offset
    naturally adjusts.
  - `finish_loading` and `begin_refresh` reset `list_state` to
    `ListState::default()` (the conversations vec changed underneath
    it; old offsets are meaningless).
  - When `self.selected` becomes `None` (no filtered items),
    `list_state.borrow_mut().select(None)`.
  - The `run_with_loader` event match's `MouseEventKind::Down(MouseButton::Left)`
    arm is deleted. `handle_list_click` is also deleted.
  - In `render_view_mode` (which uses `footer_line`), select the new
    `ViewerSearchActive` variant when `view_state.search_mode == Active`.

## Test plan

- Unit test for `list_row_lines` with the new `id` parameter:
  - `list_row_id_renders_right_aligned_in_3_cols`: id=1, fixed
    `id_width=3`, assert the header line starts with `"  1"` (two
    leading spaces).
  - `list_row_id_two_digit`: id=42, id_width=3, assert header line
    starts with `" 42"`.
  - `list_row_id_three_digit`: id=999, id_width=3, assert header line
    starts with `"999"`.
- Unit test for `footer_line`:
  - `viewer_search_active_shows_n_N_and_count`: assert the rendered
    text contains `n next`, `N prev`, `3 / 12 matches`, `Esc close`.
- No new test for the mouse click removal — verifying "no behavior on
  click" is hard without a TestBackend; rely on the manual smoke test
  and the diff review.
- No new test for the ListState persistence — verifying viewport
  behavior requires a TestBackend with a fixed-size buffer. Rely on the
  manual smoke test. Add to the manual checklist: "scroll past the
  visible area; reverse direction; cursor moves freely within the
  visible page" — the user's exact reproducer.

## Open questions

None — all design choices resolved during brainstorming.

## Out of scope

- Keyboard shortcut to jump by ID (e.g. type a number to jump). Could
  be added later if useful.
- Showing the current ID in the viewer header (e.g. "session #3 ·
  abc-1234 · 47 turns"). The ID is per-list-position, not per-session,
  so it's misleading in the viewer where ordering doesn't apply.
- Cursor-pinning policies other than the default (e.g. nvim's
  `scrolloff` to keep N rows of context above/below the cursor). Could
  be added as a config knob later if the default feels uncomfortable.
