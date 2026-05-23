# ccmanager

[![CI](https://github.com/NormalUhr/ccmanager/actions/workflows/ci.yml/badge.svg)](https://github.com/NormalUhr/ccmanager/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/NormalUhr/ccmanager?include_prereleases&sort=semver)](https://github.com/NormalUhr/ccmanager/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A terminal manager for your [Claude Code](https://claude.com/claude-code)
conversation history.

`ccmanager` reads the `.jsonl` transcripts Claude Code writes under
`~/.claude/projects/` and gives you a fast TUI to **search**, **view**,
**star**, **rename**, **delete**, **export**, and **resume** them. It
also ships a local **web UI** with the same data, and an **MCP stdio
server** so Claude Code itself can search its own history when you ask
it to.

Everything is local. No network calls, no account, no telemetry.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/NormalUhr/ccmanager/main/scripts/install.sh | bash && exec $SHELL
```

If `ccmanager` isn't found afterward, just restart your shell.

Update with:

```sh
ccmanager update
```

Prefer to build from source, or install on an air-gapped machine? See
[Other install methods](#other-install-methods) below.

## Quick start

```sh
ccmanager                          # browse all conversations
ccmanager -L                       # only conversations from the current project
ccmanager serve --open             # local web UI on http://127.0.0.1:7878
```

In the TUI, type to fuzzy-search. `enter` opens the viewer; `ctrl+r`
resumes a session in a new terminal tab; `?` shows all keybindings.

## Common scenarios

**Find a past session.** Just type. Search matches project name,
session title, custom alias, and conversation text — all at once.

**Show only this project's sessions.** `ccmanager -L` from the project
directory, or hit `tab` inside the TUI to toggle.

**Star a session so you can find it later.** Hover the row in the
list (or open it in the viewer) and press `f2`. Press `f2` again to
unstar. A `★` shows up in the leftmost column.

**Show only starred sessions.** `f3`. Press again to clear. Stacks
with the project filter and the search query.

**Give a session a memorable alias.** Focus it, press `r`, type a
name. The alias shows in the list header and is fuzzy-searchable.

**Open and read a session.** `enter`. Scroll with `↑` `↓` or `j` `k`.

**Skim just my questions** (skip Claude's answers). In the viewer,
press `shift+q`.

**Search inside an open session.** `/` then type. `n` / `shift+n`
jump to the next / previous match.

**Resume a session** in a new terminal tab. `ctrl+r`. Use `alt+r` if
you want the standard tool-permission prompts instead of the
skip-permissions default.

**Fork off a session** instead of resuming. `ctrl+f` — creates a new
session ID branched from the same starting point.

**Copy one message.** In the viewer, jump to it with `shift+j` /
`shift+k`, then press `y`.

**Export the whole dialogue.** `e`, then pick a format: Ledger /
Plain / Markdown / JSONL. Tool calls and thinking blocks are filtered
out of the readable formats.

**Refresh to pick up a message Claude Code just wrote** in another
tab. `f5`. Your search query, filter, and selection are preserved.

**Delete a session.** Focus it, `ctrl+x`, confirm.

## Keys at a glance

The full table is in the `?` overlay. Uppercase letters always mean
"hold shift" — lowercase never does.

| Key | What it does |
|---|---|
| (just type) | Fuzzy-search across all conversations |
| `↑` `↓` / `j` `k` | Move selection (list) / scroll (viewer) |
| `enter` | Open viewer |
| `tab` | Toggle all-projects ↔ current-project filter |
| `f2` | Star / unstar the focused (or open) session |
| `f3` | Toggle starred-only filter |
| `f5` | Reload list + viewer from disk |
| `ctrl+r` / `alt+r` | Resume in a new tab (fast / with permission prompts) |
| `ctrl+f` / `alt+f` | Fork — branch a new session ID |
| `r` | Rename — set a custom alias |
| `ctrl+x` | Delete (with confirm) |
| `e` | Copy whole conversation (format menu) |
| `y` | Copy focused message (after `shift+j` / `shift+k`), or open format menu |
| `shift+y` / `shift+i` / `p` | Copy file path / copy session ID / show path |
| `t` / `shift+t` / `i` | Tool display (off / truncated / full) / thinking / timestamps |
| `shift+q` | Questions-only view (hide Claude's answers) |
| `/` then type | In-viewer search; auto-jumps to first match |
| `enter` (after `/`) | Confirm search; switch to nav mode |
| `n` / `shift+n` | Next / previous match |
| `shift+j` / `shift+k` / `[` / `]` | Jump between messages |
| `?` | Help overlay |
| `q` / `esc` | Back / quit |

## Resuming conversations

`ctrl+r` opens a new tab in your **current** terminal window running
`claude --resume <id>`, switches focus to it, and leaves the
`ccmanager` TUI running in the original tab (returning to the list).

- macOS Terminal.app: Cmd+T is synthesized via AppleScript + System
  Events. macOS will prompt for **Accessibility** permission on
  `osascript` the first time — grant it once.
- macOS iTerm: native `create tab` API, no Accessibility prompt.
- Linux: `gnome-terminal --tab` or `konsole --new-tab` if present;
  other emulators (xterm, alacritty, kitty, wezterm, foot) fall back
  to opening a new window.

`cd` and `claude` are typed to the new tab as **two separate lines**
with a small delay between them, so shell hooks like direnv / nvm /
asdf / mise get a prompt redraw to fire and update `PATH` before
`claude` runs. This is what makes a project-local `claude` reachable
even when `ccmanager` was launched from outside the project folder.

By default the resume passes `--dangerously-skip-permissions` (Claude
won't re-ask about every tool, which matches the common case of
continuing work you'd already approved). Use `alt+r` for the standard
permission flow, or set `[resume].skip_permissions = false` in the
config to flip the default.

`ctrl+f` forks instead — creates a new session ID that branches from
the original transcript. When the conversation's original project
directory no longer exists or you fork cross-project, the session
files are first **copied** into your CWD's project directory so
Claude Code can find them there.

The CLI flag `ccmanager --resume` is different from the TUI key: it
**replaces the current process** with `claude --resume <id>` via
`execvp`, which is what shell scripts and aliases want.

## Live refresh (`f5`)

Keep a `ccmanager` window open while a separate Claude Code session
keeps appending turns to disk. Hit `f5` and `ccmanager` re-scans
`~/.claude/projects/` and re-renders the active viewer in place. No
quit-and-relaunch.

Preserved across refresh: the search query, the workspace filter
(`tab`), the view-mode toggles (`t` / `shift+t` / `i` / `shift+q`),
and the selected-or-open conversation (re-selected by file path, with
a sensible fallback if the file is gone).

## Export and clipboard

All export operations land on the **system clipboard** — no files are
written to your cwd. Paste with `Cmd+V` / `Ctrl+V`.

| Key | What it copies |
|---|---|
| `e` | Whole conversation. Opens a format menu: **Ledger** (formatted, speaker-prefixed), **Plain** (`You:` / `Claude:` lines), **Markdown** (`## You` / `## Claude`), or **JSONL** (raw). For the three readable formats, tool calls / thinking / intermediate narration are filtered out so what you paste is just the dialogue. |
| `y` | In message-nav mode (after `shift+j` / `shift+k`): the focused message as raw markdown. Otherwise: opens the same format menu as `e`. |
| `shift+y` | The conversation file's full path on disk. |
| `shift+i` | The session UUID. |
| `p` | Prints the file path into the status bar (no copy). |

For a file instead of the clipboard, pipe `ccmanager --plain` to one:

```sh
ccmanager --plain > conversation.txt          # pick from the TUI
ccmanager --render <file.jsonl> > out.txt     # render a specific file
```

## Web UI

```sh
ccmanager serve --open
```

Same data, in a browser. Defaults to `127.0.0.1:7878`. Non-local
binds (`--host 0.0.0.0`) **require** `--token <T>` for auth, so you
can't accidentally expose your history.

Supports live fuzzy search, viewer with syntax highlighting,
collapsible tool blocks, rename, delete (or `--read-only`), export
downloads (Markdown / plain / ledger / JSONL), and keyboard
navigation. Search state persists across back-navigation so you can
click a row, hit back, and the filtered list is restored from the URL.

## MCP server

`ccmanager mcp` runs as a [Model Context Protocol][mcp] stdio server
so Claude Code can search and read your past conversations when you
ask it to. Register it once in `~/.claude.json`:

```jsonc
{
  "mcpServers": {
    "ccmanager": {
      "command": "ccmanager",
      "args": ["mcp"]
    }
  }
}
```

Restart Claude Code. It exposes three read-only tools:

- **`search_history(query, limit?, project?)`** — fuzzy-search past
  conversations. Returns JSON with `session_id`, title, project, age,
  message count, and a short snippet per match.
- **`get_session(session_id, max_chars?)`** — fetch one conversation
  as a markdown transcript. Dialogue-only by default, truncated to
  `max_chars` (default 50k).
- **`list_recent_sessions(limit?, project?)`** — recent-first list,
  same JSON shape as search.

Claude won't call these on its own — you drive it. Try "check history
for our caching discussion" or "open the rename conversation from last
week."

[mcp]: https://modelcontextprotocol.io/

## Configuration

Optional. Create `~/.config/ccmanager/config.toml` with any subset of:

```toml
[display]
# show_thinking = true       # show <thinking> blocks in the viewer
# no_tools = false           # show tool calls in full (default: hidden)
# plain = false              # plain-text output (no ledger formatting)
# pager = true               # pipe output through `less -R` when stdout is a TTY
# last = true                # preview the *last* messages in the list (vs first)

[resume]
# default_args = ["--dangerously-skip-permissions"]
# skip_permissions = true    # ctrl+r uses --dangerously-skip-permissions

[keys]
# resume = "ctrl+r"          # primary resume binding
# fork   = "ctrl+f"          # primary fork-resume binding
# delete = "ctrl+x"          # delete binding
# resume_alt = "alt+r"       # alt binding — inverts skip_permissions
# fork_alt   = "alt+f"
```

CLI flags override the config. `CLAUDE_CONFIG_DIR` is respected if you
keep Claude's data in a non-default location.

For the full flag list: `ccmanager --help`.

## Other install methods

### From source

```sh
git clone https://github.com/NormalUhr/ccmanager.git
cd ccmanager
cargo install --path . --locked
```

This drops a `ccmanager` binary at `~/.cargo/bin/`. On Linux you also
need the X11 clipboard headers `arboard` links against —
`libxcb-shape0-dev libxcb-render0-dev libxcb-xfixes0-dev` on
Debian/Ubuntu, `libxcb-devel` on Fedora/RHEL.

### Air-gapped install (no internet on the target machine)

Vendor every Cargo dependency once on a machine that has internet, then
build offline on the server.

On the internet-connected machine:

```sh
cd ccmanager
cargo vendor --locked > .cargo/config.toml
cd .. && tar --exclude='ccmanager/target' --exclude='ccmanager/.git' \
              -czf ccmanager-offline.tar.gz ccmanager
```

Transfer `ccmanager-offline.tar.gz` to the server (scp / USB / internal
mirror). Then on the server (with rustc ≥ 1.85 and the clipboard libs
above):

```sh
tar -xzf ccmanager-offline.tar.gz
cd ccmanager
cargo build --release --offline --locked
install -m 755 target/release/ccmanager ~/.local/bin/ccmanager
```

If the server has no Rust toolchain either, also ship the matching
`rust-<version>-<triple>.tar.gz` from <https://static.rust-lang.org/dist/>
and run its `./install.sh --prefix="$HOME/.local"` first.

## Changelog

| Version | Released | Highlights |
|---|---|---|
| [1.1.1](https://github.com/NormalUhr/ccmanager/releases/tag/v1.1.1) | 2026-05-23 13:04 PDT | New warm Claude-orange palette across TUI + web UI; lifted gray-text contrast in both light and dark modes. |
| [1.1.0](https://github.com/NormalUhr/ccmanager/releases/tag/v1.1.0) | 2026-05-23 11:43 PDT | Star / favorite sessions (`f2`) and starred-only filter (`f3`); persisted as JSONL markers. |
| [1.0.0](https://github.com/NormalUhr/ccmanager/releases/tag/v1.0.0) | 2026-05-22 18:03 PDT | First public release. TUI + web UI + MCP server; rename, delete, export, resume, fork. |

Full notes per release: [CHANGELOG.md](CHANGELOG.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Short version:

```sh
cargo build
cargo test
just check        # fmt + clippy + build
just install-dev  # symlink target/debug/ccmanager into ~/.cargo/bin/
```

Open an issue with the bug or feature template, or start a thread in
Discussions for open-ended ideas.

## License

MIT — see [LICENSE](LICENSE).
