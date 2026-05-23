# ccmanager TUI v2 — visual polish design

**Status:** approved (brainstorming round, 2026-05-22)
**Scope:** the terminal interface (`ratatui` + `crossterm`), not the web UI.
**Goal:** the TUI should feel "designed" — restrained, modern, with a
clear visual hierarchy — without overhauling its information
architecture or interaction model. Same keys, same workflows, fewer
chrome characters, more breathing room.

## Goals and non-goals

**Goals**

- Strip the heavy outer border around the main list and viewer; let the
  terminal frame the content directly.
- Replace dense one-line rows with two-line entries that surface
  metadata (turn count, resume count) without needing to open the
  conversation.
- A persistent header band and footer key-hint band that bookend every
  screen.
- A single, restrained accent color used consistently — quietly visible
  on the brand mark, the search arrow, the selected speaker label, the
  modal border.
- A signature brand mark (`◈`) shared with the existing web UI
  templates so the two surfaces feel like one product.

**Non-goals**

- No new features or new keys. The visible behavior of `↑↓`, `/`,
  `Enter`, `Ctrl+R`, `Ctrl+F`, `Ctrl+X`, `F5`, `r`, `e`, `y`, `t`,
  `T`, `i`, `Q`, `Tab`, `?` is unchanged.
- No new layouts (preview pane, dashboard). The information
  architecture stays single-pane.
- No animations.
- No new dependencies.

## High-level approach

Adopt a *modern minimal* aesthetic across every TUI surface (list,
viewer, modals). Chrome subtracts: thin horizontal rules replace
boxed borders; whitespace replaces nested chrome; one accent color
replaces multi-color decoration. Two-line list entries trade vertical
density for scannability and visible metadata.

A *compact fallback* (single-line rows, collapsed footer) activates
automatically when the terminal is too small to comfortably show
two-line entries.

## Detailed design

### §1 — Composition

The list screen is divided into five vertical bands. **No outer
border around the main screen.** Thin horizontal rules are the only
structural lines; no boxes, no padding frames. (Modal dialogs are a
separate component — see §6 — and *do* keep their rounded borders.)

```
  ◈ ccmanager  ·  all projects  ·  47 sessions
  ────────────────────────────────────────────────

  search ▸ deploy

  ◆ ccmanager   Add F5 refresh
     2 hours ago  ·  47 turns

    work        Deploy strategy
    yesterday    ·  23 turns  ·  resumed 2x

    notes       Email template
    yesterday    ·  8 turns

  ────────────────────────────────────────────────
  ↑↓ nav    /  search    ⏎ view    ^R resume    ? help
```

Vertical layout (top to bottom):

1. Header (1 row).
2. Top separator: thin horizontal rule (1 row).
3. Search input (1 row), with one row of blank above and below.
4. List rows (2 rows per entry + 1 blank-row gap), scrollable.
5. Bottom separator: thin horizontal rule (1 row).
6. Footer (1 row).

The viewer screen uses the same bands; only the middle section is
the rendered transcript instead of the list.

### §2 — Header

```
  ◈ ccmanager  ·  <scope>  ·  <count> sessions
```

- `◈` (U+25C8) is the brand mark. Rendered in the accent color.
- `ccmanager` is also in the accent color — together with the glyph,
  they form the logotype.
- `<scope>`: `all projects` by default; `this project: <name>` when
  `Tab`-toggled or `-L` was used at launch. The literal "this
  project:" prefix is dimmed; the project name is primary.
- `<count>` is the total session count when no search query is active.
  When a search query *is* active, the header reads
  `<matched> / <total> sessions match` so it silently surfaces the
  search-hit fraction without adding a line.
- Middle dots (`·`) and the word "sessions" are dimmed.
- Visible at all times — list mode, viewer mode, single-file mode.
  Single-file mode uses `<filename>` instead of `<scope>`.

### §3 — List rows

```
  ◆ ccmanager   Add F5 refresh
     2 hours ago  ·  47 turns  ·  resumed 2x
```

- **Two rows per entry**, separated by a blank row.
- Line 1: 2-column left indent, selection glyph (`◆` for selected, two
  spaces otherwise), project name (dimmed), then a 4+ space gap, then
  the conversation title (primary, bold if the terminal supports it).
- Line 2: aligned indent under the title, then `<age>  ·  <turns>
  turns  ·  resumed <N>x` — all dimmed, middle-dot separated.
  `resumed <N>x` appears only when N > 0.
- **Selected row** also gets a subtle background tint so the row
  reads as "present" even when the glyph is off-screen during search
  input. The tint covers both rows and the blank-row gap between
  them. Concrete tint values: `#eeeeee` on light terminals,
  `#1f2128` on dark terminals — close to but distinct from terminal
  default. Falls back gracefully on terminals that can't render
  background colors (the chevron alone still signals selection).
- **Custom titles** (set via `r`) display as the title literally.
  Claude's original summary is dropped (not shown alongside) to keep
  the line clean.
- **Search highlights**: matched substrings in title and project name
  render in the accent color (also bold if available). The existing
  per-token highlight stays; we replace its color choice with the
  accent.

### §4 — Search input

```
  search ▸ deploy_
```

- Plain inline text, no input box.
- `search` label in dim text, `▸` in accent, the typed query in
  primary text.
- Cursor (block) at the end of the query (already provided by
  ratatui).
- Empty state: `  search ▸` followed by a faint placeholder
  `(fuzzy across all transcripts)` in tertiary-dim. The placeholder
  vanishes the moment any character is typed.

### §5 — Footer / status bar

Context-aware key hints. Three states:

| State           | Content                                                            |
| --------------- | ------------------------------------------------------------------ |
| List, idle      | `↑↓ nav    /  search    ⏎ view    ^R resume    F5 refresh    ? help` |
| Viewer          | `↑↓ scroll    /  search    e copy    r rename    q back    ? help`   |
| Status message  | The message itself in accent, e.g. `Refreshed: 47 conversations`. Replaces the key hints for 3 seconds, then they return. |

- Key glyphs (`↑↓`, `/`, `⏎`, `^R`, `F5`, `?`, `e`, `r`, `q`) in
  primary text.
- Their descriptions in dim text.
- Always one row, always visible, always at the bottom.

When in message-nav mode (after `J`/`K`), swap two keys into the
footer: `J K` replaces `↑↓ scroll` as the leading key hint (since
those are the active navigation bindings in nav mode), and
`y copy message` replaces `e copy`.

### §6 — Modal dialogs

Modals (export menu, rename, confirm-delete, help overlay) **keep**
their rounded borders — they're modal, they should feel separate.
But:

- Border weight stays single-line, but the border color is a muted
  variant of the accent (rendered at ~60% brightness) so it doesn't
  fight the content.
- Title (`Copy to clipboard`, `Rename conversation`, …) renders in
  full-strength accent, centered, with a thin underline rule
  (separate row) below it.
- Internal padding reduced from 2-col / 1-row to 1-col / 0-row.
- Option list: selected option gets the same bg tint + chevron
  treatment as list rows.
- Bottom border carries the dismiss hint inline (`Esc cancel  ·  ⏎
  select`), in dim — same convention the existing help overlay uses.

The four current modals follow the same template; differences are
only the title and the option list contents.

### §7 — Viewer (ledger-style transcript)

Same band composition as the list. The middle band is the rendered
conversation.

- Header: `◈ ccmanager  ·  session <12-char-id>  ·  <N> turns`
  (replaces the list-mode scope/count).
- Ledger entries unchanged structurally. The speaker label (`You`,
  `Claude`) renders in **accent color** instead of plain text;
  everything else (the `│` gutter separator, message content,
  message-nav `▌` cursor) stays as today.
- Footer switches to viewer keys (see §5).
- Search-in-viewer bar (when `/` pressed) is an inline
  `find ▸ <query>     3 / 12 matches` line at the top of the middle
  band, no box. `n`/`N` cycle results as today.
- The `t` / `T` / `i` / `Q` toggles produce the same content
  changes; the toggle state is not surfaced anywhere in the chrome
  (matches today's behavior — `?` shows current state in the help
  overlay).

### §8 — Color palette

One accent color, consistently applied. Restrained greys for
hierarchy. Auto-detected light vs. dark via `terminal-light`.

```
                        Light theme  Dark theme
  Primary text:         default fg   default fg
  Secondary (dim):      #707070      #9a9a9a
  Tertiary (more dim):  #a8a8a8      #6a6a6a
  Selected bg tint:     2 shades off 2 shades off
  Accent (default):     #0a6fc7      #6cb8ff   (cool blue)
  Accent border        ~60% of accent (one notch dimmer)
  (in modals):
```

Accent choice is **cool blue**. Quiet, doesn't fight terminal
themes, and reads as "tooling/code" without being the
warmer/branding territory of orange. The existing palette in
`src/tui/theme.rs` is replaced with these tokens.

User can override via `[display].theme = "light" | "dark"` in
config (already supported infrastructure-wise — only the palette
content changes).

### §9 — Empty / loading / fallback states

- **Loading** (streaming conversations in): header reads
  `◈ ccmanager  ·  all projects  ·  loading… <n> so far`. Search
  input is rendered but won't fire searches until `finish_loading`
  has run. List rows appear as their batches arrive.
- **No matches** (search query with zero hits): centered in the list
  area, two dim lines:
  ```
        no conversations match your search
              press Esc to clear it
  ```
- **No history at all** (empty `~/.claude/projects/`): same shape,
  message reads `you don't have any Claude Code conversations yet`.
- **Compact fallback**: when terminal height < 20 rows OR width < 60
  cols, the list collapses to single-line rows (no blank row between)
  and the footer shortens to the four most-important keys
  (`↑↓ /  ⏎  ? help`). Threshold is a single const, not a config
  knob.

## Implementation notes

Files touched, in rough order of work:

- `src/tui/theme.rs` — replace palette tokens; add `accent_dim`
  (~60% accent), `bg_tint` (one shade off bg), `text_tertiary`.
- `src/tui/ui.rs` — bulk of the work:
  - `render_list_screen`: rewrite to band layout. Use
    `ratatui::layout::Layout::vertical` with `[Length(1),
    Length(1), Length(1), Min(0), Length(1), Length(1)]` for
    header / rule / search / list / rule / footer. Drop the
    `Block::default().borders(ALL)` wrapping.
  - `render_view_screen`: same band layout; middle section
    delegates to the existing transcript renderer (unchanged).
  - `render_list_row`: rewrite to emit two lines per entry plus a
    blank gap. Pass the selection state explicitly so we can apply
    bg tint to both lines.
  - `render_search_bar`: drop the box; emit
    `  search ▸ <query>` as a single styled line.
  - `render_header`: new function emitting the brand mark + scope
    + count line.
  - `render_footer`: new function with three branches (list /
    viewer / status-message).
  - `render_export_menu`, `render_rename_dialog`,
    `render_confirm_dialog`, `render_help_overlay`: refactor to
    share a common `render_modal(title, body, hint)` helper that
    applies the §6 rules.
- `src/tui/app.rs` — viewport-height math for the new band
  composition; small adjustment to scroll-offset clamping because
  entries now span two rows + a gap. Add a `compact_mode_active()`
  helper that returns true under the §9 thresholds.
- `src/history/loader.rs` or wherever `Conversation` is built —
  ensure `turn_count` and `resume_count` are populated (verify;
  may already exist).
- `src/web/server.rs` etc. — unchanged. Web UI is out of scope.
- `src/tui/viewer.rs` — small change to color the speaker label in
  accent. Everything else stays.

The accent-color choice (cool blue) lives in `theme.rs` and can be
flipped to amber by changing two hex codes if we change our mind
later.

## Test plan

- Unit tests for `render_list_row` (input: Conversation + selected
  bool; output: two styled lines + a gap line; verify the bg-tint
  spans both data lines and the gap when selected).
- Unit tests for `render_header` (three permutations: idle, search
  active, loading).
- Unit tests for `render_footer` (three permutations: list, viewer,
  status-message).
- Snapshot tests of the full list-screen render at two terminal
  sizes (one normal, one below the compact threshold) — verify the
  fallback engages at the boundary.
- Manual TUI smoke test: launch, browse, search, open viewer,
  rename, delete (confirm), export, resume, refresh. Check both
  light and dark terminals.

## Open questions

None — all major decisions resolved during brainstorming:

- Surface: TUI only.
- Direction: visual polish (not density or interactions).
- Profile: modern minimal (the middle of three options).
- Brand mark: yes, `◈`, matching the web UI.
- Accent: cool blue (`#0a6fc7` / `#6cb8ff`).
- Modals: keep borders, refine internals.

## Out of scope

- Web UI (`ccmanager serve`) is untouched.
- No new keybindings.
- No new dependencies.
- No animations or transitions.
- Color override: a custom user-defined accent via config is not
  added in this pass — only "light" vs "dark" continues to be
  user-controllable. Custom accent can come later if requested.
