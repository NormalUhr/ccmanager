//! Launching `claude --resume <id>` from a `ccmanager` session.
//!
//! Two entry points:
//! - [`exec_in_place`] — replaces the current process with `claude`. Used by
//!   the `ccmanager --resume` CLI flag, where the caller expects to be
//!   handed off to a Claude Code session in the same terminal.
//! - [`spawn_in_new_terminal`] — opens a new tab in the current terminal
//!   window (Terminal.app or iTerm on macOS; gnome-terminal / konsole on
//!   Linux when available, falling back to a new window for emulators
//!   without a tab CLI) and runs `claude` there. Used by the TUI so the
//!   interactive session it just spawned stays running and the user is
//!   moved to the new tab.
//!
//! Both paths share [`plan_resume`], which decides the cwd to launch in, the
//! exact `claude` args, and whether the session's `.jsonl` (and any per-session
//! tool-result subdir) needs to be copied to the cwd's project dir first
//! (used when the original project dir is gone or when forking cross-project).

use crate::error::{AppError, Result};
use crate::history;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What [`plan_resume`] decided to do — separated from the exec so it can be
/// unit-tested without actually launching claude.
#[derive(Debug, PartialEq, Eq)]
pub struct ResumePlan {
    /// Working directory to launch claude in. Claude Code locates the
    /// session by encoding this path and reading
    /// `<encoded-cwd>/<session-id>.jsonl` under the projects root.
    pub launch_cwd: PathBuf,
    pub args: Vec<String>,
    /// When set, copy the session files (`.jsonl` + `<session-id>/` subdir)
    /// from the source dir to the target dir before launching claude.
    pub copy: Option<(PathBuf, PathBuf)>,
}

/// Append `--dangerously-skip-permissions` to `args` iff the user opted into
/// skip-permissions for this resume AND it isn't already present from the
/// configured `default_args` (avoids passing the flag twice).
fn maybe_add_skip_permissions(args: &mut Vec<String>, default_args: &[String], skip: bool) {
    const FLAG: &str = "--dangerously-skip-permissions";
    if !skip {
        return;
    }
    let already_present = default_args.iter().any(|a| a == FLAG) || args.iter().any(|a| a == FLAG);
    if !already_present {
        args.push(FLAG.to_string());
    }
}

pub fn plan_resume(
    selected_path: &Path,
    project_path: Option<&PathBuf>,
    default_args: &[String],
    fork_session: bool,
    cwd: &Path,
    skip_permissions: bool,
) -> Result<ResumePlan> {
    let conversation_id = selected_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            AppError::ClaudeExecutionError("Conversation filename is not valid Unicode".to_string())
        })?
        .to_owned();

    let conv_projects_dir = selected_path.parent().ok_or_else(|| {
        AppError::ClaudeExecutionError(
            "Cannot determine conversation's project directory".to_string(),
        )
    })?;

    // Only use project_path if launching claude there would actually let it
    // find the .jsonl file. Claude Code locates a session by encoding its cwd
    // and looking up `<encoded-cwd>/<session-id>.jsonl` under
    // `~/.claude/projects/`. If `encoded(project_path)` doesn't match the
    // directory holding the file (which happens with renamed/moved project
    // dirs, lossy worktree decoding, or messages whose recorded cwd no longer
    // matches the on-disk project dir name), claude would either error with
    // "session not found" or silently resume an unrelated session that
    // happens to share the id — appearing to the user as a disorganized,
    // mismatched conversation. Fall back to copy-and-resume in that case.
    let project_dir = project_path.filter(|p| {
        p.exists()
            && p.is_dir()
            && history::get_claude_projects_dir(p)
                .map(|d| d == conv_projects_dir)
                .unwrap_or(false)
    });

    // When the original project directory is gone (e.g. deleted worktree) or when
    // forking cross-project, copy session files to CWD's project directory and
    // resume from there.
    let needs_copy = if project_dir.is_none() {
        true
    } else if fork_session {
        let cwd_projects_dir = history::get_claude_projects_dir(cwd)?;
        cwd_projects_dir != conv_projects_dir
    } else {
        false
    };

    let mut args = vec!["--resume".to_string(), conversation_id];
    // Pass --fork-session in both paths when requested. Cross-project fork
    // (the copy path) needs it too, otherwise the copied file is appended to
    // under the original session id while the original still exists in the
    // source project dir — leaving two diverging files that share an id and
    // confuse later resumes/searches.
    if fork_session {
        args.push("--fork-session".to_string());
    }
    maybe_add_skip_permissions(&mut args, default_args, skip_permissions);
    args.extend(default_args.iter().cloned());

    if needs_copy {
        let cwd_projects_dir = history::get_claude_projects_dir(cwd)?;
        // When the selected jsonl already lives in cwd's project dir
        // (e.g. resuming a session that was previously copied here by
        // an earlier fork), there's nothing to copy — and naively
        // setting `copy = Some((same, same))` would have
        // `std::fs::copy(jsonl, jsonl)` truncate the file to 0 bytes
        // on macOS. Drop the copy step in that case; the file is
        // already where claude will look for it.
        let copy = if cwd_projects_dir == conv_projects_dir {
            None
        } else {
            Some((conv_projects_dir.to_path_buf(), cwd_projects_dir))
        };
        Ok(ResumePlan {
            launch_cwd: cwd.to_path_buf(),
            args,
            copy,
        })
    } else {
        Ok(ResumePlan {
            launch_cwd: project_dir.unwrap().clone(),
            args,
            copy: None,
        })
    }
}

/// Execute the copy step from `plan.copy` if set. No-op when `plan.copy` is
/// `None`. Safe to call before either `exec_in_place` or
/// `spawn_in_new_terminal`.
pub fn run_copy_step(selected_path: &Path, plan: &ResumePlan) -> Result<()> {
    let Some((source_dir, target_dir)) = &plan.copy else {
        return Ok(());
    };
    // Defensive: `plan_resume` is supposed to set `copy = None` when
    // src == dst, but guard here too. A `std::fs::copy(p, p)` would
    // truncate `p` to 0 bytes on macOS, destroying the transcript.
    if source_dir == target_dir {
        return Ok(());
    }
    let conversation_id = selected_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            AppError::ClaudeExecutionError("Conversation filename is not valid Unicode".to_string())
        })?;
    std::fs::create_dir_all(target_dir).map_err(AppError::Io)?;
    copy_session_files(selected_path, conversation_id, source_dir, target_dir)
}

/// Copy session files from one project directory to another for cross-project forking.
///
/// Copies:
/// 1. The .jsonl transcript file
/// 2. The session subdirectory (tool-results/, subagents/) if it exists
/// 3. The file-history directory for undo support if it exists
fn copy_session_files(
    jsonl_path: &Path,
    session_id: &str,
    source_projects_dir: &Path,
    target_projects_dir: &Path,
) -> Result<()> {
    // 1. Copy the .jsonl file
    let target_jsonl = target_projects_dir.join(jsonl_path.file_name().unwrap());
    std::fs::copy(jsonl_path, &target_jsonl).map_err(AppError::Io)?;

    // 2. Copy the session subdirectory (tool-results/, subagents/)
    let session_dir = source_projects_dir.join(session_id);
    if session_dir.is_dir() {
        let target_session_dir = target_projects_dir.join(session_id);
        copy_dir_recursive(&session_dir, &target_session_dir)?;
    }

    // Note: file-history (~/.claude/file-history/<uuid>/) is global, not per-project.
    // Claude Code finds it by session ID, so no copy needed.

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(AppError::Io)?;
    for entry in std::fs::read_dir(src).map_err(AppError::Io)? {
        let entry = entry.map_err(AppError::Io)?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(AppError::Io)?;
        }
    }
    Ok(())
}

/// Replace the current process with `claude`. Unix path uses `execvp`; on
/// non-Unix platforms we spawn and wait. Used by the `--resume` CLI flag,
/// where the caller wants to be handed off to Claude Code directly.
#[cfg(unix)]
pub fn exec_in_place(plan: &ResumePlan) -> Result<()> {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new("claude");
    command.args(&plan.args);
    command.current_dir(&plan.launch_cwd);
    let err = command.exec();
    Err(AppError::ClaudeExecutionError(err.to_string()))
}

#[cfg(not(unix))]
pub fn exec_in_place(plan: &ResumePlan) -> Result<()> {
    let mut command = Command::new("claude");
    command.args(&plan.args);
    command.current_dir(&plan.launch_cwd);
    let status = command
        .status()
        .map_err(|e| AppError::ClaudeExecutionError(e.to_string()))?;
    if !status.success() {
        return Err(AppError::ClaudeExecutionError(format!(
            "claude CLI exited with status {}",
            status
        )));
    }
    Ok(())
}

/// Open a new tab in the current terminal window and run
/// `claude --resume <id> …` in it. The caller (the TUI) keeps running in
/// the original tab. On success returns `Ok(())` once the spawn helper has
/// been launched — we don't (and can't) wait for `claude` itself to exit.
///
/// macOS: AppleScript via `osascript`. Terminal.app gets a synthesized
/// Cmd+T via System Events (requires Accessibility permission for
/// `osascript` on first use); iTerm uses its native `create tab` API when
/// `$TERM_PROGRAM == iTerm.app`. Other macOS terminals (WezTerm, Ghostty,
/// kitty, …) fall through to the Terminal.app path — they get a tab in a
/// new Terminal.app window rather than one of their own.
///
/// Linux: prefers `gnome-terminal --tab` and `konsole --new-tab`; for
/// emulators without a "new tab in current window" CLI it falls back to
/// opening a new window.
///
/// Other platforms: not supported; returns an error so the TUI can surface
/// a status message.
#[allow(clippy::needless_return)] // cfg-gated arms; only one is compiled in
pub fn spawn_in_new_terminal(plan: &ResumePlan) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        return spawn_macos(plan);
    }
    #[cfg(target_os = "linux")]
    {
        return spawn_linux(plan);
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = plan;
        Err(AppError::ClaudeExecutionError(
            "Opening a new terminal window isn't supported on this platform yet — \
             run `ccmanager --resume <id>` from a shell instead"
                .to_string(),
        ))
    }
}

/// Build the combined shell command line:
/// `cd <quoted-cwd> && exec claude <quoted-args...>`.
///
/// Used for the Linux path (a single `sh -c <cmd>` invocation). The macOS
/// path uses [`build_cd_command`] + [`build_claude_command`] separately so
/// it can send them as two lines with a prompt redraw in between.
///
/// `exec` makes the shell hand its PID to claude, so when claude exits the
/// terminal window's shell exits cleanly.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn build_shell_command(cwd: &Path, args: &[String]) -> String {
    format!(
        "{} && {}",
        build_cd_command(cwd),
        build_claude_command(args)
    )
}

/// `cd '<quoted-cwd>'` — the cd portion of the launch command, used on its
/// own for the macOS two-step approach.
fn build_cd_command(cwd: &Path) -> String {
    let cwd_str = cwd.to_string_lossy();
    format!("cd {}", shell_escape(&cwd_str))
}

/// `exec claude <quoted-args...>` — the claude portion, used on its own for
/// the macOS two-step approach. Sent as a separate line so the shell's
/// prompt redraws between `cd` and `claude`, giving direnv / nvm / asdf /
/// mise hooks a chance to fire and update PATH for the project dir before
/// we try to launch `claude`. Bundling both into one pipeline (`cd && exec`)
/// skips that redraw and breaks any setup where `claude` is project-local.
fn build_claude_command(args: &[String]) -> String {
    let mut s = String::from("exec claude");
    for a in args {
        s.push(' ');
        s.push_str(&shell_escape(a));
    }
    s
}

/// POSIX-safe single-quote wrap. Embedded single quotes become `'\''`.
fn shell_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Escape a string for use inside an AppleScript double-quoted literal.
/// Backslash and double-quote are the only specials that matter for our
/// use; we don't try to handle non-ASCII (AppleScript accepts UTF-8).
#[cfg(target_os = "macos")]
fn applescript_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn spawn_macos(plan: &ResumePlan) -> Result<()> {
    // We send `cd` and `claude` as TWO separate lines (not `cd && exec
    // claude` joined). Sending them joined runs them as one pipeline with
    // no prompt redraw in between — and shell hooks like direnv / nvm /
    // asdf / mise wire themselves into `precmd` / `PROMPT_COMMAND`, which
    // only fires when the shell is about to print a new prompt. Without a
    // redraw they never get to update PATH for the new cwd, so a
    // project-local `claude` stays unfindable. Splitting the two commands
    // gives the shell a real prompt redraw after `cd`, the hook runs, PATH
    // gets the project's `claude`, and then we send the second line.
    let cd_cmd = build_cd_command(&plan.launch_cwd);
    let claude_cmd = build_claude_command(&plan.args);
    let cd_esc = applescript_escape(&cd_cmd);
    let claude_esc = applescript_escape(&claude_cmd);

    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

    let lines: Vec<String> = if term_program == "iTerm.app" {
        // iTerm exposes tab creation natively, so no System Events / no
        // Accessibility prompt. After `create tab`, the new tab is the
        // current tab, and `current session of current window` is its
        // shell.
        vec![
            "tell application \"iTerm\" to activate".to_string(),
            "tell application \"iTerm\" to tell current window to create tab with default profile"
                .to_string(),
            format!(
                "tell application \"iTerm\" to tell current session of current window to write text \"{}\"",
                cd_esc
            ),
            "delay 0.8".to_string(),
            format!(
                "tell application \"iTerm\" to tell current session of current window to write text \"{}\"",
                claude_esc
            ),
        ]
    } else {
        // Terminal.app: use AppleScript's native `do script ""` to
        // create a new tab/window and capture a direct reference to its
        // tab. All subsequent commands target that tab by reference, so
        // there's no race over "which is the front window/tab now."
        //
        // Why not the old System Events Cmd+T + `in front window`
        // pattern? Two reasons:
        //
        // 1. **Race.** `do script "..." in front window` runs in the
        //    front window's *currently selected* tab — it does not
        //    create a tab. The old code synthesized Cmd+T and then
        //    delayed 0.25 s, hoping the Cmd+T-created tab/window would
        //    be the selected/front one by the time `do script` ran.
        //    On macOS setups where Terminal.app's "New Tab" menu item
        //    has no Cmd+T shortcut bound (default on recent macOS),
        //    Cmd+T from System Events resolves to "open a new window"
        //    rather than "open a new tab." The new window's promotion
        //    to "front window" is then asynchronous; when the 0.25 s
        //    lost the race, `do script` typed the resume command into
        //    ccmanager's tab via stdin. Symptom: status bar said
        //    "Resumed in new terminal tab" but no new tab appeared and
        //    the TUI re-entered View mode mid-flood.
        //
        // 2. **Permissions.** System Events requires Accessibility
        //    permission for the calling process; silent permission
        //    drift caused confusing failures.
        //
        // `do script ""` (no `in` clause, empty command) creates a new
        // window with a fresh shell and synchronously returns its tab
        // reference. We capture it as `newTab` and run subsequent
        // `do script "<cmd>" in newTab` against that exact tab.
        // Nothing to race against — the reference is bound before any
        // further work.
        //
        // Tabs vs. windows: on systems with macOS's "Prefer tabs when
        // opening documents" = "Always", the OS auto-merges the new
        // window into the existing one as a tab; on other settings it
        // stays as a separate window. Either way the user gets a
        // working interactive claude session and the ccmanager tab is
        // preserved.
        //
        // The 0.8 s between `cd` and `claude` is retained: it gives
        // the shell time to process `cd`, redraw its prompt, and run
        // any direnv / nvm / asdf / mise hook BEFORE we type `claude`.
        // A project-local `claude` is only on PATH after the hook
        // fires for the new cwd. A pipeline like `cd && exec claude`
        // would skip the prompt redraw and the hook would never run.
        vec![
            "tell application \"Terminal\" to activate".to_string(),
            "tell application \"Terminal\"".to_string(),
            "  set newTab to do script \"\"".to_string(),
            format!("  do script \"{}\" in newTab", cd_esc),
            "  delay 0.8".to_string(),
            format!("  do script \"{}\" in newTab", claude_esc),
            "end tell".to_string(),
        ]
    };
    run_osascript(&lines)
}

#[cfg(target_os = "macos")]
fn run_osascript(lines: &[String]) -> Result<()> {
    use std::process::Stdio;
    let mut cmd = Command::new("osascript");
    for line in lines {
        cmd.arg("-e").arg(line);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .map_err(|e| AppError::ClaudeExecutionError(format!("osascript: {}", e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = stderr.trim();
        let detail = if msg.is_empty() {
            format!("osascript exited with {}", output.status)
        } else {
            format!("osascript failed: {}", msg)
        };
        return Err(AppError::ClaudeExecutionError(detail));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn spawn_linux(plan: &ResumePlan) -> Result<()> {
    use std::process::Stdio;
    let cmd_string = build_shell_command(&plan.launch_cwd, &plan.args);

    // (binary, prefix args before the shell command). We always end with
    // `sh -c <command>` so the `cd && exec` line is run by a POSIX shell.
    //
    // Tab-capable emulators come first and use their "new tab" flag so the
    // resumed session lands as a tab in the existing window. xterm /
    // alacritty / kitty / wezterm / foot don't have a "new tab in current
    // window" CLI, so they fall through and open a new window instead.
    let candidates: &[(&str, &[&str])] = &[
        ("gnome-terminal", &["--tab", "--", "sh", "-c"]),
        ("konsole", &["--new-tab", "-e", "sh", "-c"]),
        ("x-terminal-emulator", &["-e", "sh", "-c"]),
        ("xterm", &["-e", "sh", "-c"]),
        ("alacritty", &["-e", "sh", "-c"]),
        ("kitty", &["--", "sh", "-c"]),
        ("wezterm", &["start", "--", "sh", "-c"]),
        ("foot", &["-e", "sh", "-c"]),
    ];

    for (bin, prefix) in candidates {
        let mut cmd = Command::new(bin);
        for a in *prefix {
            cmd.arg(a);
        }
        cmd.arg(&cmd_string);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match cmd.spawn() {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(AppError::ClaudeExecutionError(format!("{}: {}", bin, e)));
            }
        }
    }
    Err(AppError::ClaudeExecutionError(
        "No terminal emulator found. Install one of: gnome-terminal, konsole, \
         xterm, alacritty, kitty, wezterm, foot — or set x-terminal-emulator."
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    //! Pure unit tests for shell / AppleScript escaping helpers. The
    //! env-dependent `plan_resume` tests live in `tests/launcher_plan.rs` so
    //! they get their own test binary and don't race with the library's other
    //! `CLAUDE_CONFIG_DIR`-mutating tests (web, mcp).

    use super::*;

    #[test]
    fn shell_escape_simple() {
        assert_eq!(shell_escape("hello"), "'hello'");
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn shell_escape_with_spaces_and_specials() {
        assert_eq!(
            shell_escape("path with spaces & $vars"),
            "'path with spaces & $vars'"
        );
    }

    #[test]
    fn shell_escape_with_embedded_single_quote() {
        // Embedded single quote → close, escaped, reopen.
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn build_shell_command_joins_cd_and_claude() {
        let cmd = build_shell_command(
            Path::new("/tmp/foo bar"),
            &["--resume".to_string(), "abc-123".to_string()],
        );
        assert_eq!(cmd, "cd '/tmp/foo bar' && exec claude '--resume' 'abc-123'");
    }

    #[test]
    fn build_cd_command_quotes_paths() {
        assert_eq!(build_cd_command(Path::new("/tmp/foo")), "cd '/tmp/foo'");
        assert_eq!(
            build_cd_command(Path::new("/tmp/with spaces")),
            "cd '/tmp/with spaces'"
        );
    }

    #[test]
    fn build_claude_command_quotes_each_arg() {
        let cmd = build_claude_command(&[
            "--resume".to_string(),
            "abc-123".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ]);
        assert_eq!(
            cmd,
            "exec claude '--resume' 'abc-123' '--dangerously-skip-permissions'"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn applescript_escape_quotes_and_backslashes() {
        assert_eq!(applescript_escape("plain"), "plain");
        assert_eq!(applescript_escape("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(applescript_escape("c:\\path"), "c:\\\\path");
        // A shell command with quoted args round-trips both escapes.
        let cmd = "cd '/tmp/x' && exec claude '--resume' 'id'";
        let esc = applescript_escape(cmd);
        // No double-quote characters in the source so the output equals input.
        assert_eq!(esc, cmd);
    }
}
