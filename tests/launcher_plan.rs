//! Env-dependent tests for `launcher::plan_resume`.
//!
//! These live as an integration test (own test binary) because they mutate
//! the process-global `CLAUDE_CONFIG_DIR`. The library test binary also has
//! tests that mutate that env var (in `src/web/tests.rs` and
//! `src/mcp/tests.rs`) without serialization, so running these alongside the
//! library tests races. As an integration test they run in their own
//! process, and the local `ENV_LOCK` serializes the cases within this file.

use ccmanager::history::convert_path_to_project_dir_name;
use ccmanager::launcher::plan_resume;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvScope {
    prev: Option<String>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl EnvScope {
    fn new(claude_config_dir: &Path) -> Self {
        let guard = ENV_LOCK.lock().unwrap();
        let prev = env::var("CLAUDE_CONFIG_DIR").ok();
        unsafe {
            env::set_var("CLAUDE_CONFIG_DIR", claude_config_dir);
        }
        Self {
            prev,
            _guard: guard,
        }
    }
}

impl Drop for EnvScope {
    fn drop(&mut self) {
        unsafe {
            if let Some(prev) = self.prev.take() {
                env::set_var("CLAUDE_CONFIG_DIR", prev);
            } else {
                env::remove_var("CLAUDE_CONFIG_DIR");
            }
        }
    }
}

fn make_session(project_dir_path: &Path) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let projects_root = temp.path().join("projects");
    let project_dir_name = convert_path_to_project_dir_name(project_dir_path);
    let project_dir = projects_root.join(&project_dir_name);
    std::fs::create_dir_all(&project_dir).unwrap();

    let session_id = "11111111-2222-3333-4444-555555555555";
    let jsonl_path = project_dir.join(format!("{}.jsonl", session_id));
    std::fs::write(&jsonl_path, "{}\n").unwrap();
    (temp, jsonl_path, project_dir)
}

#[test]
fn plan_uses_project_dir_when_encoding_matches() {
    let project_path = std::env::temp_dir().join("ch-test-match");
    std::fs::create_dir_all(&project_path).unwrap();
    let (temp, jsonl, _) = make_session(&project_path);
    let _scope = EnvScope::new(temp.path());

    let cwd = std::env::temp_dir().join("ch-test-match-cwd");
    std::fs::create_dir_all(&cwd).unwrap();

    let plan = plan_resume(&jsonl, Some(&project_path), &[], false, &cwd, false).unwrap();
    assert_eq!(plan.launch_cwd, project_path);
    assert!(plan.copy.is_none(), "should not copy in the happy path");
    assert_eq!(
        plan.args,
        vec![
            "--resume".to_string(),
            "11111111-2222-3333-4444-555555555555".to_string()
        ]
    );
    std::fs::remove_dir_all(&project_path).ok();
    std::fs::remove_dir_all(&cwd).ok();
}

#[test]
fn plan_falls_back_to_copy_when_project_path_does_not_exist() {
    let original = std::env::temp_dir().join("ch-test-missing-DOES-NOT-EXIST");
    let (temp, jsonl, _) = make_session(&original);
    let _scope = EnvScope::new(temp.path());

    let cwd = std::env::temp_dir().join("ch-test-missing-cwd");
    std::fs::create_dir_all(&cwd).unwrap();

    let plan = plan_resume(&jsonl, Some(&original), &[], false, &cwd, false).unwrap();
    assert_eq!(plan.launch_cwd, cwd, "missing project_dir → launch in cwd");
    assert!(
        plan.copy.is_some(),
        "missing project_dir → copy session to cwd's project dir"
    );
    std::fs::remove_dir_all(&cwd).ok();
}

#[test]
fn plan_falls_back_to_copy_when_project_path_encoding_mismatches() {
    let session_origin = std::env::temp_dir().join("ch-test-mismatch-origin");
    let (temp, jsonl, _) = make_session(&session_origin);
    let _scope = EnvScope::new(temp.path());

    let unrelated = std::env::temp_dir().join("ch-test-mismatch-unrelated");
    std::fs::create_dir_all(&unrelated).unwrap();

    let cwd = std::env::temp_dir().join("ch-test-mismatch-cwd");
    std::fs::create_dir_all(&cwd).unwrap();

    let plan = plan_resume(&jsonl, Some(&unrelated), &[], false, &cwd, false).unwrap();
    assert_eq!(
        plan.launch_cwd, cwd,
        "encoding mismatch must NOT silently launch in unrelated dir"
    );
    assert!(plan.copy.is_some(), "encoding mismatch must trigger copy");
    std::fs::remove_dir_all(&unrelated).ok();
    std::fs::remove_dir_all(&cwd).ok();
}

#[test]
fn plan_passes_fork_session_in_direct_path() {
    let project_path = std::env::temp_dir().join("ch-test-fork-direct");
    std::fs::create_dir_all(&project_path).unwrap();
    let (temp, jsonl, _) = make_session(&project_path);
    let _scope = EnvScope::new(temp.path());

    let plan = plan_resume(&jsonl, Some(&project_path), &[], true, &project_path, false).unwrap();
    assert!(plan.copy.is_none());
    assert!(
        plan.args.iter().any(|a| a == "--fork-session"),
        "fork_session must be forwarded in the direct path: {:?}",
        plan.args
    );
    std::fs::remove_dir_all(&project_path).ok();
}

#[test]
fn plan_passes_fork_session_in_copy_path() {
    let project_path = std::env::temp_dir().join("ch-test-fork-cross-src");
    std::fs::create_dir_all(&project_path).unwrap();
    let (temp, jsonl, _) = make_session(&project_path);
    let _scope = EnvScope::new(temp.path());

    let cwd = std::env::temp_dir().join("ch-test-fork-cross-dst");
    std::fs::create_dir_all(&cwd).unwrap();

    let plan = plan_resume(&jsonl, Some(&project_path), &[], true, &cwd, false).unwrap();
    assert!(plan.copy.is_some(), "cross-project fork must copy");
    assert!(
        plan.args.iter().any(|a| a == "--fork-session"),
        "fork_session must be forwarded in the copy path: {:?}",
        plan.args
    );
    std::fs::remove_dir_all(&project_path).ok();
    std::fs::remove_dir_all(&cwd).ok();
}

#[test]
fn skip_permissions_appends_flag_when_requested() {
    let project_path = std::env::temp_dir().join("ch-test-skip-on");
    std::fs::create_dir_all(&project_path).unwrap();
    let (temp, jsonl, _) = make_session(&project_path);
    let _scope = EnvScope::new(temp.path());

    let plan = plan_resume(&jsonl, Some(&project_path), &[], false, &project_path, true).unwrap();
    assert!(
        plan.args
            .iter()
            .any(|a| a == "--dangerously-skip-permissions"),
        "skip_permissions=true must add the flag: {:?}",
        plan.args
    );
    std::fs::remove_dir_all(&project_path).ok();
}

#[test]
fn skip_permissions_omits_flag_when_disabled() {
    let project_path = std::env::temp_dir().join("ch-test-skip-off");
    std::fs::create_dir_all(&project_path).unwrap();
    let (temp, jsonl, _) = make_session(&project_path);
    let _scope = EnvScope::new(temp.path());

    let plan = plan_resume(
        &jsonl,
        Some(&project_path),
        &[],
        false,
        &project_path,
        false,
    )
    .unwrap();
    assert!(
        !plan
            .args
            .iter()
            .any(|a| a == "--dangerously-skip-permissions"),
        "skip_permissions=false must NOT add the flag: {:?}",
        plan.args
    );
    std::fs::remove_dir_all(&project_path).ok();
}

#[test]
fn skip_permissions_does_not_duplicate_when_user_already_set_it() {
    let project_path = std::env::temp_dir().join("ch-test-skip-already");
    std::fs::create_dir_all(&project_path).unwrap();
    let (temp, jsonl, _) = make_session(&project_path);
    let _scope = EnvScope::new(temp.path());

    let user_default_args = vec!["--dangerously-skip-permissions".to_string()];
    let plan = plan_resume(
        &jsonl,
        Some(&project_path),
        &user_default_args,
        false,
        &project_path,
        true,
    )
    .unwrap();
    let count = plan
        .args
        .iter()
        .filter(|a| *a == "--dangerously-skip-permissions")
        .count();
    assert_eq!(
        count, 1,
        "must not duplicate the flag when user already set it: {:?}",
        plan.args
    );
    std::fs::remove_dir_all(&project_path).ok();
}

#[test]
fn skip_permissions_works_with_fork_session() {
    let project_path = std::env::temp_dir().join("ch-test-skip-fork");
    std::fs::create_dir_all(&project_path).unwrap();
    let (temp, jsonl, _) = make_session(&project_path);
    let _scope = EnvScope::new(temp.path());

    let plan = plan_resume(&jsonl, Some(&project_path), &[], true, &project_path, true).unwrap();
    assert!(plan.args.iter().any(|a| a == "--fork-session"));
    assert!(
        plan.args
            .iter()
            .any(|a| a == "--dangerously-skip-permissions"),
        "fork + skip_permissions must include both flags: {:?}",
        plan.args
    );
    std::fs::remove_dir_all(&project_path).ok();
}

#[test]
fn plan_does_not_self_copy_when_session_already_in_cwd_projects_dir() {
    // Scenario: the selected jsonl already lives in cwd's project dir
    // (e.g. an earlier fork copied it here). Its recorded `cwd` field
    // points elsewhere, so `project_dir` filtering rejects it and
    // `needs_copy` becomes true. Naive logic would set
    // `copy = Some((cwd_dir, cwd_dir))` — and `std::fs::copy(p, p)`
    // truncates `p` to 0 bytes on macOS, destroying the transcript.
    // The plan must recognize src == dst and emit `copy: None`.
    let cwd = std::env::temp_dir().join("ch-test-no-self-copy-cwd");
    std::fs::create_dir_all(&cwd).unwrap();
    // Place the jsonl in CWD's project dir directly (simulates the
    // "already a copy here" state).
    let (temp, jsonl, _) = make_session(&cwd);
    let _scope = EnvScope::new(temp.path());

    // `project_path` is unrelated — its encoded form will NOT match
    // the jsonl's parent dir, so `project_dir` filter returns None
    // and the copy-fallback branch runs.
    let unrelated = std::env::temp_dir().join("ch-test-no-self-copy-elsewhere");
    std::fs::create_dir_all(&unrelated).unwrap();

    let plan = plan_resume(&jsonl, Some(&unrelated), &[], false, &cwd, false).unwrap();
    assert_eq!(plan.launch_cwd, cwd);
    assert!(
        plan.copy.is_none(),
        "must not self-copy when jsonl is already in cwd's project dir: {:?}",
        plan.copy
    );
    std::fs::remove_dir_all(&cwd).ok();
    std::fs::remove_dir_all(&unrelated).ok();
}
