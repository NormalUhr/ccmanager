## Unreleased

(Nothing yet — next release goes here.)

## v1.1.3 (2026-05-27)

### Fixed

- **Ctrl+C now cancels every modal dialog.** Reported as a freeze
  on delete: pressing `Ctrl+X` to delete a session opened the
  confirm dialog, and `Ctrl+C` (the universal "get me out of here"
  shortcut) had no visible effect — looked like the TUI was frozen,
  forcing a terminal-tab close. The program was actually running and
  reading every keystroke; the confirm dialog's key handler just
  silently ignored everything that wasn't `y / Y / n / N / Esc`. The
  same trap existed in the export format picker and the help overlay
  (Ctrl+C ignored), and in the rename modal (Ctrl+C arrived as
  `Char('c')` and got appended to the title buffer as a literal `c`).
  Ctrl+C now closes any open modal with no side effect; existing
  `y / n / Esc` / type-to-edit bindings unchanged.

## v1.1.2 (2026-05-27)

### Fixed

- **Resume reliably opens a new tab/window in Terminal.app.** The
  second `Ctrl+R` (or third, or fourth) in a single ccmanager session
  used to fail silently on macOS Terminal.app: the status bar said
  "Resumed in new terminal tab" but no new tab appeared, and the TUI
  re-entered the previously-viewed session's detail page. Root cause
  was a timing race in the spawn AppleScript — the old code
  synthesized Cmd+T via System Events, waited a fixed 0.25 s, then
  ran `do script "..." in front window`. `do script ... in front
  window` runs in the front window's *currently selected* tab; it
  does not create one. On macOS setups where the "New Tab" menu item
  has no Cmd+T shortcut bound (default on recent macOS), the
  synthesized Cmd+T resolves to "open a new window," and the new
  window's promotion to "front window" is asynchronous. When 0.25 s
  lost the race, the resume command got typed into ccmanager's stdin
  instead, which the TUI walked character-by-character — re-entering
  View mode on the synthetic Enter, opening the rename dialog on
  `r`, etc. The macOS spawn now uses Terminal.app's native
  `do script ""` to create the tab and synchronously capture a
  direct reference to it, then runs `do script "..." in newTab`
  against that exact tab. Nothing to race against. Also removes the
  Accessibility / System Events permission dependency. iTerm and
  Linux paths were unaffected.
- **Cross-project fork no longer risks truncating the transcript.**
  When the selected session's `.jsonl` already lived in the cwd's
  project dir (e.g. a leftover from an earlier fork) but the
  session's recorded `cwd` field pointed elsewhere, the resume
  planner would emit `copy: Some((same_dir, same_dir))`, and
  `std::fs::copy(p, p)` truncates `p` to 0 bytes on macOS. The
  resume now short-circuits the copy step when src == dst, with a
  defensive guard in `run_copy_step` itself.
- **Stale duplicates removed from the conversation list.** Sessions
  that existed in more than one `~/.claude/projects/<dir>/` (typical
  after a cross-project fork) used to show up multiple times in the
  list, with the stale copy diverging from the live transcript over
  time. The list now dedupes by session UUID and prefers the
  canonical copy — the one whose parent directory name matches the
  encoded form of its recorded `cwd`. Disk state is unchanged; only
  the in-memory list view is filtered.

## v1.1.1 (2026-05-23)

### Changed

- **New color palette.** Replaced the cool-blue (TUI) and teal (web)
  signature colors with Claude Code's warm orange. Same hue family in
  both UIs, tuned per background: `#E6886A` on dark, `#BF5C3C` on
  light. The lingering legacy teal highlights — TUI timestamps,
  search-match background, context highlight, focus rings — were
  retinted into the warm family so the palette is coherent.
- **Improved gray contrast.** Body, preview and metadata text was hard
  to read in both dark and light mode. Lifted `text_secondary`,
  `text_muted`, `text_tertiary`, `preview`, `msg_count`, and the web
  UI's `--fg-muted` / `--fg-subtle` toward higher contrast in both
  themes, while keeping the visual hierarchy intact.

## v1.1.0 (2026-05-23)

### Added

- **Star/favorite a session.** Press `F2` from the list view (acts on the
  hovered row) or from the viewer (acts on the open session) to toggle
  the star. A `★` glyph appears in a new leftmost column on starred
  rows.
- **Starred-only filter.** Press `F3` from the list view to show only
  starred sessions; press again to clear. Composes cleanly with the
  workspace filter and the search query — search within starred,
  resume from starred, delete from starred, all the usual list
  operations work on the filtered view.

Star state is persisted by appending a `{"type":"star","starred":bool}`
marker line to the conversation's JSONL file (latest marker wins, same
durable pattern used by `/rename`). No new state files, no external
DB. Cache schema bumped to v3; first launch after upgrade rebuilds the
cache automatically.

## v1.0.0 (2026-05-23)

First public release of `ccmanager`. Forked from `claude-history`,
substantially rewritten for the new UI and refined over four
iterations (Modern Minimal → Framed Hybrid → List Affordances). If
you're coming from `claude-history`, see the migration note at the
bottom of this entry.

### What's in the box

- **TUI** for browsing, searching, viewing, renaming, deleting,
  exporting, and resuming conversations.
- **Local web UI** (`ccmanager serve`) — same data, in a browser.
- **MCP stdio server** (`ccmanager mcp`) so Claude Code can search
  your history when you ask it to. Read-only.
- **Self-updater** (`ccmanager update`).

### List view

- Rounded outer frame with a brand-marked header
  (`◈ ccmanager · scope · count`) and a context-aware key-hint footer.
- Per-row session IDs (1, 2, … N) in a dim left column, re-numbered
  on filter changes.
- Three-line entries: header (project + title + right-aligned
  `<N>msg · <age>`), preview (the **start of the most recent user
  message** in the conversation), and a thin `─` separator.
- Gutter bar (`▌`) + subtle background tint mark the selected row.
- Search-as-you-type accent highlighting on titles and project names.
- Compact-mode fallback (two-line rows) for small terminals.
- Free-cursor scrolling: the cursor moves through the visible
  viewport; the page only scrolls when the cursor crosses an edge.
- Mouse-wheel scrolling supported; mouse clicks intentionally not
  bound (avoid mis-click-to-enter).

### Resume in a new terminal tab

`Ctrl+R` / `Alt+R` / `Ctrl+F` / `Alt+F` open a new tab in your
current terminal window running `claude --resume <id>`, switch focus
there, and leave `ccmanager` in the original tab. macOS Terminal.app
uses Cmd+T via AppleScript + System Events; iTerm uses its native
`create tab` API; Linux prefers `gnome-terminal --tab` /
`konsole --new-tab`, falls back to a new window otherwise.

`cd` and `claude` are sent as two separate lines so shell hooks
(direnv / nvm / asdf / mise) get a prompt redraw and update `PATH`
before `claude` runs. Project-local `claude` works even when
`ccmanager` was launched from outside the project folder.

By default, `--dangerously-skip-permissions` is passed automatically
(most resumes continue work you already approved). `Alt+R` / `Alt+F`
prompt instead; `[resume].skip_permissions = false` flips the
default globally.

The `ccmanager --resume` CLI flag is the non-TUI variant — it
`execvp`s `claude` in place, so shell scripts and aliases keep
working.

### Live refresh (`F5`)

For workflows where `ccmanager` stays open while a separate Claude
Code session keeps appending turns to disk, hitting `F5` re-scans
`~/.claude/projects/` and re-renders the active viewer in place.
Search query, workspace filter, and view-mode toggles all persist;
the previously selected or open conversation is re-selected by file
path.

### Viewer

- Same framed treatment as the list, with session-id + turn-count
  header.
- Vim-style in-conversation search: `/` opens, typing live-jumps to
  the first match, `Enter` confirms, `n`/`N` cycle, `Esc` closes.
  The footer surfaces `n next  ·  N prev  ·  M / N matches  ·  Esc
  close` while active.
- Toggle tool calls (`t`), thinking blocks (`T`), timestamps (`i`),
  questions-only view (`Q`).
- Rename a conversation in place (`r`) — writes the same JSONL
  `custom-title` format as Claude Code's `/rename`, so the two stay
  in sync.

### Export to clipboard

The `e` key in the viewer opens a format menu (Ledger / Plain /
Markdown / JSONL) and copies the chosen render to the system
clipboard — no temp files. Tool calls / thinking / intermediate
narration filter out for the readable formats; JSONL is raw.

### Configuration

Optional `~/.config/ccmanager/config.toml` for display defaults,
default resume args, and rebindable keys. `CLAUDE_CONFIG_DIR` is
respected.

### Migration from `claude-history`

```sh
mv ~/.config/claude-history/config.toml ~/.config/ccmanager/config.toml 2>/dev/null
mv ~/.cache/claude-history ~/.cache/ccmanager 2>/dev/null
```

---

## Predecessor history (claude-history v0.x)

Versions below are inherited from the `claude-history` ancestor for
reference. They predate the `ccmanager` rename and the v1.0.0
redesign.

## v0.2.0 (2026-04-23)

Major release adding a local web UI and several viewer ergonomics.

- Web UI: search state now persists across back-navigation. As you type,
  the URL updates to `/conversations?q=…` in place (no extra history
  entries). Clicking a row then pressing `Cmd+[` (macOS) / `Ctrl+[` or the
  browser back button restores both the query and the filtered results.
- Web UI: the `← list` button and the `Cmd+[` shortcut prefer
  `history.back()` when you came from the list, and fall back to an
  explicit navigation for deep-linked conversation URLs. `← list` now has
  a tooltip advertising the shortcut.
- **Local web UI.** `ccmanager serve` starts a localhost web server
  (default `http://127.0.0.1:7878`) with the same list + viewer + search
  workflow as the TUI, plus syntax-highlighted code blocks, collapsible
  tool blocks, live fuzzy search, rename, and Markdown / plain / ledger /
  JSONL export downloads. Binds to `127.0.0.1` by default; non-localhost
  binds require `--token <T>` for auth. `--read-only` disables all
  mutation endpoints. Available as a default Cargo feature; build with
  `--no-default-features` for a smaller TUI-only binary. Keyboard
  shortcuts match the TUI where possible (`j/k`, `J/K`, `g/G`, `/`).
- Viewer now defaults to a clean dialogue-only view — tool calls and tool
  results are hidden on open so you see only your prompts and Claude's
  replies. Press `t` to cycle through modes (hidden → truncated → full), or
  pass `--show-tools` / set `no_tools = false` in config to start with tools
  visible. Previously the default was truncated mode.
- Questions-only view in the viewer: press `Q` to collapse the conversation
  down to just your prompts, hiding Claude's answers, tool output, and
  subagent internals. Use `J` / `K` (or `[` / `]`) to jump between questions
  in the collapsed view, then press `Q` again to return to the full view —
  it scrolls automatically to whichever question you had focused, so the
  collapsed view doubles as a question-based jump list.
- Rename a conversation from the viewer with `r` — the custom title is written
  to the JSONL (same format as Claude Code's `/rename`) and shown in warm gold
  on the list next to the project name. Enter to save, Esc to cancel, empty
  input clears the title.
- Export now dumps only the user's prompts and Claude's final answers per
  round by default — intermediate narration, tool calls, tool results,
  thinking blocks, and subagent entries are filtered out of the Ledger,
  Plain, and Markdown formats. JSONL export remains raw.

## v0.1.53 (2026-04-17)

- Conversation viewer no longer jumps to unrelated content when toggling tool
  output (`t`), thinking (`T`), or timing (`i`), or when resizing the terminal —
  the viewport now stays anchored to the message you were reading

## v0.1.52 (2026-04-17)

- Mouse wheel scrolling in both the search result list and conversation viewer,
  and click-to-open on rows in the search result list (note: enabling mouse
  capture may interfere with click-drag text selection in some terminals — hold
  Shift, or Option on macOS, to bypass)
- Search results now show the selected position (e.g. current/total) so it's
  easier to tell where you are in the list
- Improved search snippet previews — the context line now prefers locations
  where query terms appear adjacent, instead of locking onto boilerplate matches
  that happen earlier in the conversation
- Fixed search ranking missing adjacent-phrase matches when the phrase was
  wrapped in markdown punctuation like `**media pipeline**`
- Added a Nix flake for installation on Nix systems

## v0.1.51 (2026-03-29)

- Improved search ranking — results now score matches by where they appear
  (title, project, summary, or message body), so exact project and title matches
  rank above incidental mentions in conversation text
- Search freshness scoring uses smooth decay instead of sharp cutoffs, giving
  more natural ranking between recent and older conversations

## v0.1.50 (2026-03-27)

- Pressing Esc now clears the search input first — a second Esc quits the app

## v0.1.49 (2026-03-24)

- Faster startup with per-project binary caching of parsed conversations — only
  changed files are re-parsed on subsequent launches
- Reduced memory usage by streaming JSONL lines instead of loading entire files
  into memory

## v0.1.48 (2026-03-24)

- Fixed search missing content in long conversations due to a 256K character
  truncation limit — all conversation text is now fully searchable
- Added Windows support — compilation and home directory resolution now work
  correctly on Windows ([#26](https://github.com/raine/ccmanager/pull/26))

## v0.1.47 (2026-03-22)

- Fixed conversations that only contain skill invocations (e.g. `/consult`,
  `/commit`) being incorrectly filtered out as empty sessions

## v0.1.46 (2026-03-21)

- Fixed the screen freezing when holding down arrow keys or j/k to scroll — the
  view now redraws smoothly during key repeat instead of jumping when the key is
  released ([#25](https://github.com/raine/ccmanager/issues/25))

## v0.1.45 (2026-03-20)

- Skill invocation prompts (e.g. from `/consult`, `/commit`) are now hidden from
  search results and shown as a concise description in the conversation viewer
  instead of displaying the full expanded prompt text

## v0.1.44 (2026-03-17)

- Added support for `CLAUDE_CONFIG_DIR` environment variable — users with custom
  Claude config directories can now use ccmanager without workaround
  ([#24](https://github.com/raine/ccmanager/issues/24))

## v0.1.43 (2026-03-14)

- Added `ccmanager update` command for self-updating the binary directly
  from GitHub releases

## v0.1.42 (2026-03-13)

- Subagent messages are now included in J/K message navigation and single
  message copy
- Plain text mode (`--plain`) now supports pager output
- Fixed `--no-color` flag being ignored in normal (non-render) display mode
- Fixed text wrapping for CJK characters and emoji that occupy two terminal
  columns but were counted as one, causing text to overflow
- Deleting a session in the TUI (`Ctrl+X`) now removes the full session
  directory, not just the transcript file
- Fixed a potential crash when deleting a conversation while a search was
  in-flight
- Fixed conversations opened by UUID not showing project name or matching
  workspace filter
- `--fork-session` now requires `--resume` and shows an error if used alone
  instead of being silently ignored

## v0.1.41 (2026-03-13)

- Workspace filter now includes conversations from git worktrees of the same
  project, so sessions started in workmux worktrees appear alongside the main
  project's sessions
- Search result counter now shows the count relative to the current scope
  (project or global) instead of always showing the total

## v0.1.40 (2026-03-13)

- Search typing is now smoother — search runs in a background thread so
  keystrokes no longer block the UI, especially with large history
- Global view is now the default — all conversations are shown on launch instead
  of only the current workspace's sessions
  ([#21](https://github.com/raine/ccmanager/pull/21))
- Added `Tab` key to toggle between global and workspace-only view in the TUI
  ([#21](https://github.com/raine/ccmanager/pull/21))
- Added `-L`/`--local` flag to start with workspace filter active
- Deprecated `--global`/`-g` flag and `global` config option — global is now the
  default behavior

## v0.1.39 (2026-03-13)

- Added `--delete` flag to remove a session by its ID directly from the command
  line, e.g. `ccmanager --delete <session-id>`
  ([#23](https://github.com/raine/ccmanager/issues/23)
- Added `--version` flag to display the current version
  ([#22](https://github.com/raine/ccmanager/issues/22))
- Invalid session IDs now show a clear error message instead of failing silently

## v0.1.38 (2026-03-13)

- Improved search for CJK (Chinese, Japanese, Korean) text — queries with CJK
  characters now match correctly even when words aren't separated by spaces
  ([#19](https://github.com/raine/ccmanager/pull/19))
- Added emacs-style keybindings to the search input: `Ctrl+A`/`Ctrl+E` to jump
  to start/end, `Ctrl+B`/`Ctrl+F` to move by character, `Alt+B`/`Alt+F` and
  `Ctrl+Left`/`Ctrl+Right` to move by word, `Ctrl+K` to kill to end of line,
  `Ctrl+U` to kill to start of line
  ([#19](https://github.com/raine/ccmanager/pull/19))
- Fixed cursor alignment issues with wide characters (e.g. CJK, emoji) in the
  search input and conversation viewer
  ([#19](https://github.com/raine/ccmanager/pull/19))

## v0.1.37 (2026-03-13)

- Linux prebuilt binaries are now statically linked, fixing compatibility issues
  on older distros with outdated glibc versions
  ([#20](https://github.com/raine/ccmanager/issues/20))

## v0.1.36 (2026-03-12)

- Added message-level navigation — press `J`/`K` or `[`/`]` to jump between
  messages in the conversation viewer, with a teal marker showing the focused
  message ([#15](https://github.com/raine/ccmanager/pull/15))
- Added single message copy — press `y` to copy the currently selected message
  to the clipboard instead
  ([#15](https://github.com/raine/ccmanager/pull/15))
- Fixed empty thinking blocks rendering as blank "Thinking" labels with no
  content

## v0.1.35 (2026-03-12)

- Timestamps in the conversation list now automatically switch between relative
  ("just now", "5 min ago", "2 hours ago", "yesterday") for recent sessions and
  absolute ("Mar 05, 14:30") for older ones
- Recent conversations are color-graded by recency — newest sessions appear in
  bright teal, fading to gray as they get older, making it easy to spot recent
  activity at a glance
- Removed `--relative-time`/`--absolute-time` flags and `display.relative_time`
  config option — the new hybrid format replaces both

## v0.1.34 (2026-03-12)

- Search now covers tool output (command results, file contents, grep output),
  so you can find conversations by content that previously only appeared in tool
  calls
- Search highlighting now merges adjacent matches across separators — searching
  "red team" highlights the full word `red_team` instead of just the individual
  parts
- Improved search performance in conversations with large tool outputs

## v0.1.33 (2026-03-12)

- Added automatic light/dark theme detection — the TUI now adapts its color
  scheme to match your terminal background
- Fixed arrow key navigation lag when holding keys to scroll quickly through the
  list or conversation viewer
- Fixed slow redraw when pasting text into the search field

## v0.1.32 (2026-03-12)

- Fixed clipboard copy/yank not working on Linux — now uses `wl-copy` on Wayland
  and `xclip`/`xsel` on X11, with automatic display server detection
  ([#17](https://github.com/raine/ccmanager/pull/17))
- Fixed resuming sessions from deleted or ephemeral git worktrees failing with
  an error instead of gracefully recovering

## v0.1.31 (2026-03-09)

- Search now matches project names, so you can find sessions by the project they
  belong to

## v0.1.30 (2026-03-09)

- Preview panel now shows the last messages by default instead of the first, so
  you see the most recent context at a glance (use `--first` to revert)

## v0.1.29 (2026-03-09)

- Added `--fork-session` flag to resume a conversation as a fork, creating a new
  branch from an existing session
- Cross-project forking: when forking a session from a different project, the
  session files are automatically copied to the current project so Claude
  resumes in the right context
- Added configurable keybindings via the `[keys]` section in the config file,
  allowing rebinding of resume (`Ctrl+R`), fork (`Ctrl+F`), and delete
  (`Ctrl+X`) actions
- Session list search now matches session UUIDs, making it easier to find a
  specific conversation by ID
- Fixed markdown rendering issues: soft breaks no longer collapse words
  together, inline code no longer clips at block edges, and list item spacing is
  correct

## v0.1.28 (2026-03-04)

- Subagent (Task tool) messages are now nested under their parent task, keeping
  the conversation view clean and organized with `↳` prefixed entries
- Subagent internals are hidden by default and revealed with `T` or
  `--show-thinking`, same as thinking blocks
- XML-tagged content (system reminders, analysis blocks) now displays correctly
  instead of being silently stripped
- Conversations from CI or headless Claude runs that lack timestamps now parse
  and display correctly

## v0.1.27 (2026-02-26)

- Session titles (set via `/rename` in Claude Code) now appear in the
  conversation list and viewer, making it easier to find named sessions
- Search preview shows matches better now

## v0.1.26 (2026-02-18)

- Added `global = true` config option to default to global search without
  passing `-g` every time, with `--local` flag to override when needed
- Ledger export and clipboard copy now render markdown properly (headings,
  lists, code blocks, tables) and wrap long lines instead of overflowing
- Fixed high idle CPU usage (~9% down to near zero) when the TUI was sitting
  idle after loading
- Fixed search preview highlighting partial word matches instead of the actual
  search phrase
- Fixed long lines in code blocks overflowing the terminal width
- Fixed blank lines and indentation issues in ledger export

## v0.1.25 (2026-02-11)

- Added `--show-id` (`-i`) flag to print the selected conversation's session ID,
  useful for resuming with custom shell aliases (e.g.,
  `claude --resume $(ccmanager --show-id)`)
- Added `I` keybinding in the viewer to copy the session ID to clipboard

## v0.1.24 (2026-02-11)

- Tool calls now default to **truncated** mode, showing the header and first few
  lines with a "(N more lines...)" indicator — a middle ground between hidden
  and full output. Press `t` to cycle through modes: off, truncated, full
- Added `--no-tools` flag to start with tools hidden (complements `--show-tools`
  for full mode)
- Tables in conversation output are now rendered with proper box-drawing borders
  instead of being collapsed into plain text

## v0.1.23 (2026-02-08)

- Fixed blank or empty message blocks occasionally appearing in conversation
  output

## v0.1.22 (2026-02-07)

- Added multi-word search support in the viewer — search for phrases like "add
  feature" to find matches containing both words
- Timestamps now display on tool calls and results in ledger view (when timing
  is enabled with `i`)
- Fixed a crash that could occur when highlighting search matches containing
  certain Unicode characters

## v0.1.21 (2026-02-05)

- Fixed timestamp alignment for subagent messages and empty messages in ledger
  view
- Fixed double blank lines appearing after tool calls with empty output
- `/clear` commands are no longer shown in conversation rendering

## v0.1.20 (2026-02-05)

- Added toggleable timing display in conversation viewer — press `i` to show
  timestamps next to each message
- Show conversation duration and model/token count in the viewer header
- Show conversation duration in the conversation list
- Added keyboard shortcuts help overlay — press `?` in any view
- Added keyboard shortcuts bar at the bottom of the conversation list
- Added `Ctrl+R` (resume) and `Ctrl+X` (delete) shortcuts to the viewer status
  bar
- Added `Ctrl+C` to quit from viewer mode
- Exports now include thinking blocks and tool calls when their display is
  toggled on
- Long bash commands in tool calls are now wrapped for readability
- Improved search highlight color for better visibility

## v0.1.19 (2026-02-04)

- Added syntax highlighting for code blocks in conversation output
- Improved tool call display with human-readable formatting instead of raw JSON
- Added Vim-style half-page navigation (Ctrl-D/Ctrl-U) in the viewer
- Added Ctrl-W to delete word before cursor in the search field
- Show conversation summary in the viewer header and search results
- Display subagent conversations in ledger view
- Added direct JSONL file input support (pass a file path as argument)
- Added `--render` flag for debugging ledger output
- Improved header layout: combined into single line when terminal width allows
- Tool/thinking toggle settings now persist within session

## v0.1.18 (2026-02-02)

- Added in-TUI conversation viewer. Press Enter to view conversations without
  leaving the TUI, with Vim-style navigation (j/k, d/u, g/G) and search (/)
- Added export and yank menus to the viewer. Press `e` to export to file or `y`
  to copy to clipboard in multiple formats (ledger, plain text, markdown, JSONL)
- Added `Y` hotkey to copy the conversation file path to clipboard
- Added `resume.default_args` config option to pass custom arguments when
  resuming conversations with `Ctrl+R`
- Improved markdown rendering: fixed spacing after numbered lists, styled
  headings with subtle color
- Fixed thinking blocks to render with italic and dimmed style
- Fixed user messages showing in wrong color in the viewer
- Improved search performance

## v0.1.17 (2026-02-01)

- Added `Ctrl+R` keybinding to resume the selected conversation directly from
  the TUI

## v0.1.16 (2026-02-01)

- Fixed a crash when using global search (`-g`) that could occur when deleting
  conversations

## v0.1.15 (2026-02-01)

- Added ability to delete conversations from the TUI (press `Ctrl+D`, confirm
  with `y`)
- Added cursor navigation in the search field with arrow keys

## v0.1.14 (2026-02-01)

- Added markdown rendering for conversation output with support for headings,
  lists, code blocks, tables, and inline formatting
- Added pager support—long conversations now open in `less` (or `$PAGER`)
- Added `--plain` flag for unformatted output
- Improved search to better match word variations (e.g., "config" now matches
  "configuration")
- Added `curl | bash` install script
- Hide caveat metadata from conversation previews

## v0.1.13 (2026-02-01)

- Replaced fzf with a built-in terminal UI

## v0.1.12 (2026-01-11)

- Fixed project path detection failing for usernames containing dots (e.g.,
  `my.user`) (Thanks @duke8585!)

## v0.1.11 (2025-12-20)

- Cleaned up fzf picker display by removing index numbers

## v0.1.10 (2025-12-15)

- Added a specific error message when fzf version is too old (requires 0.67.0+)

## v0.1.9 (2025-12-14)

- Added color highlighting to the fzf picker

## v0.1.8 (2025-12-14)

- Improved fzf UX: the timestamp stays visible when searching

## v0.1.7 (2025-12-14)

- Added `--global` (`-g`) flag to search conversations across all projects at
  once

## v0.1.6 (2025-11-29)

- Added `--all-projects` (`-a`) flag to browse conversations from any project
- Added `--show-path` (`-p`) flag to print the selected conversation's file path
- Improved fuzzy search to match against full conversation content
- Added Homebrew installation support

## v0.1.5 (2025-11-17)

- Added display of tool call inputs and results when viewing conversations
- Fixed project detection for paths containing dots or special characters

## v0.1.4 (2025-10-30)

- Added faster startup with parallel conversation loading

## v0.1.3 (2025-10-30)

- Added `--debug` flag to show diagnostic information about conversation loading
- Fixed conversations containing only `/clear` commands incorrectly appearing in
  the list
- Cleaned up `/clear` command metadata from conversation previews
- Used file modification time for more accurate conversation dates

## v0.1.2 (2025-10-29)

- Fixed display of tool results that contain structured content instead of plain
  text

## v0.1.1 (2025-10-29)

- Added configuration file support (`~/.config/ccmanager/config.toml`) for
  persistent display preferences
- Added `--show-thinking` and `--hide-thinking` flags to control visibility of
  Claude's thinking blocks
- Hidden tool calls by default (use `--show-tools` or `-t` to show them)
- Added `--first` flag to show first messages in preview (inverse of `--last`)
- Added `--absolute-time` flag to explicitly use timestamps (inverse of
  `--relative-time`)
- Fixed message preview order when using `--last` flag

## v0.1.0 (2025-10-29)

- Initial release
