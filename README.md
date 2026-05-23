# ccmanager

[![CI](https://github.com/NormalUhr/ccmanager/actions/workflows/ci.yml/badge.svg)](https://github.com/NormalUhr/ccmanager/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/NormalUhr/ccmanager?include_prereleases&sort=semver)](https://github.com/NormalUhr/ccmanager/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A terminal manager for your [Claude Code](https://claude.com/claude-code)
conversation history.

`ccmanager` reads the `.jsonl` transcripts Claude Code writes under
`~/.claude/projects/` and gives you a fast TUI to **search**, **view**,
**rename**, **delete**, **export**, and **resume** them. It also ships a
local **web UI** with the same data, and an **MCP stdio server** so
Claude Code itself can search its own history when you ask it to.

Everything is local. No network calls, no account, no telemetry.

## Install

**One-line install** (recommended — downloads the prebuilt binary for
your platform from the latest GitHub release):

```sh
curl -fsSL https://raw.githubusercontent.com/NormalUhr/ccmanager/main/scripts/install.sh | bash
```

Installs to `/usr/local/bin/ccmanager` (or `~/.local/bin/ccmanager` if
`/usr/local/bin` isn't writable). Supports macOS (Intel + Apple
Silicon) and Linux (x86_64).

### From source

If you'd rather build from source, or you're on a platform the
prebuilt binaries don't cover:

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

## Quick start

```sh
ccmanager                          # browse all conversations
ccmanager -L                       # only conversations from the current project
ccmanager --resume                 # pick one and resume it in Claude Code
ccmanager <path/to/file.jsonl>     # view one transcript directly
ccmanager serve --open             # local web UI on http://127.0.0.1:7878
ccmanager mcp                      # MCP stdio server (for Claude Code itself)
```

In the TUI, type to fuzzy-search. `Enter` opens the viewer; `Ctrl+R`
resumes the selected conversation in a new terminal tab; `?` shows all
keybindings; `q`/`Esc` goes back or quits. Mouse-wheel scrolls the
list; mouse clicks are intentionally not bound (use `Enter`).

## The list

```
╭─ ◈ ccmanager · all projects · 47 sessions ──────────────╮
│  search ▸ deploy                                         │
│ ────────────────────────────────────────────────────── │
│   1 ▌ ccmanager  Add F5 refresh         47msg · 2h ago  │
│       how do I get F5 to reload conversations live, w… │
│     ────────────────────────────────────────────────── │
│   2   work       Deploy strategy      23msg · yesterday │
│       should we deploy on Tuesday or wait for the rele… │
│     ────────────────────────────────────────────────── │
│ ...                                                      │
╰─ ↑↓ nav  · / search  · ⏎ view  · ^R resume  · ? help ──╯
```

Each row carries a session **ID** (`1`, `2`, …), the project name,
the conversation title, and a metadata column (`<N>msg · <age>`)
right-aligned to the frame. The dim line beneath each title is the
**start of the most recent user message** in that conversation — at a
glance, "what was I asking?". A thin rule separates entries.

Type a query and the title + project highlight in the accent color
live as you type. The session count in the top title strip switches
to `5 / 47 sessions match` so you always know how filtered the view
is.

The selected row stays visible inside the viewport as you move — the
cursor walks freely through the visible area, and the page only
scrolls when the cursor crosses the top or bottom edge.

## Keys at a glance

The full table is in the `?` overlay. The ones you'll use every day:

| Key | What it does |
|---|---|
| Type | Fuzzy-search across all conversations |
| `↑↓` / `jk` | Move selection (list) / scroll (viewer) |
| `Enter` | Open viewer |
| `Ctrl+R` / `Alt+R` | Resume conversation in a new tab (fast / with permission prompts) |
| `Ctrl+F` / `Alt+F` | Same, but fork — branches off a new session ID |
| `F5` | Reload list and current viewer from disk |
| `e` | Copy whole conversation to clipboard (format menu) |
| `y` | Copy focused message (in nav mode) or open the format menu |
| `Y` / `I` / `p` | Copy file path / copy session ID / show path |
| `r` | Rename — sets a custom title shown in the list |
| `Ctrl+X` | Delete conversation (with confirm) |
| `t` / `T` / `i` | Tool display (off/trunc/full) / thinking / timestamps |
| `Q` | Questions-only view (hide Claude's answers) |
| `Tab` | Toggle all-projects ↔ current-project filter |
| `/` then type | Start in-viewer search; auto-jumps to first match |
| `Enter` (after `/`) | Confirm search; switches to nav mode |
| `n` / `N` | Next / previous match (vim-style) |
| `J` `K` `[` `]` | Jump between messages |
| `?` | Help overlay |
| `q` / `Esc` | Back / quit |

## Resuming conversations

`Ctrl+R` opens a new tab in your **current** terminal window running
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
continuing work you'd already approved). Use `Alt+R` for the standard
permission flow, or set `[resume].skip_permissions = false` in the
config to flip the default.

`Ctrl+F` forks instead — creates a new session ID that branches from
the original transcript. When the conversation's original project
directory no longer exists or you fork cross-project, the session
files are first **copied** into your CWD's project directory so
Claude Code can find them there.

The CLI flag `ccmanager --resume` is different from the TUI key: it
**replaces the current process** with `claude --resume <id>` via
`execvp`, which is what shell scripts and aliases want.

## Live refresh (`F5`)

Keep a `ccmanager` window open while a separate Claude Code session
keeps appending turns to disk. Hit `F5` and `ccmanager` re-scans
`~/.claude/projects/` and re-renders the active viewer in place. No
quit-and-relaunch.

Preserved across refresh: the search query, the workspace filter
(`Tab`), the view-mode toggles (`t`/`T`/`i`/`Q`), and the
selected-or-open conversation (re-selected by file path, with a
sensible fallback if the file is gone).

## Export and clipboard

All export operations land on the **system clipboard** — no files are
written to your cwd. Paste with `Cmd+V` / `Ctrl+V`.

| Key | What it copies |
|---|---|
| `e` | Whole conversation. Opens a format menu: **Ledger** (formatted, speaker-prefixed), **Plain** (`You:` / `Claude:` lines), **Markdown** (`## You` / `## Claude`), or **JSONL** (raw). For the three readable formats, tool calls / thinking / intermediate narration are filtered out so what you paste is just the dialogue. |
| `y` | In message-nav mode (after `J`/`K`): the focused message as raw markdown. Otherwise: opens the same format menu as `e`. |
| `Y` | The conversation file's full path on disk. |
| `I` | The session UUID. |
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
# skip_permissions = true    # Ctrl+R uses --dangerously-skip-permissions

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

## Updating

```sh
ccmanager update          # downloads the latest GitHub release for your platform
brew upgrade ccmanager    # if installed via Homebrew
```

The self-updater refuses to overwrite a Homebrew-managed install.

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
