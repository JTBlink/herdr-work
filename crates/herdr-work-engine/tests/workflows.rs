use herdr_work_core::{AgentTarget, Delivery, Execution, ExecutionId, Side, TaskId};
use herdr_work_engine::HerdrWork;
use std::{path::Path, process::Command};

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn fixture() -> (tempfile::TempDir, tempfile::TempDir, HerdrWork) {
    let repo = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q"]);
    git(repo.path(), &["config", "user.name", "Fixture"]);
    git(
        repo.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    std::fs::write(repo.path().join("file.txt"), "before\n").unwrap();
    git(repo.path(), &["add", "file.txt"]);
    git(repo.path(), &["commit", "-qm", "initial"]);
    std::fs::write(repo.path().join("file.txt"), "after\n").unwrap();
    let desk = HerdrWork::open(home.path(), None).unwrap();
    (repo, home, desk)
}

#[test]
fn file_menu_operations_reuse_source_actions_without_overwriting_changed_targets() {
    use herdr_work_code::actions::MenuAction;
    let (repo, _home, desk) = fixture();
    let create = desk
        .prepare_file_action(repo.path(), None, MenuAction::NewFile)
        .unwrap();
    desk.apply_file_action(&create, "created.txt").unwrap();
    assert!(repo.path().join("created.txt").is_file());
    assert!(desk.apply_file_action(&create, "created.txt").is_err());
    let rename = desk
        .prepare_file_action(
            repo.path(),
            Some(&repo.path().join("created.txt")),
            MenuAction::Rename,
        )
        .unwrap();
    desk.apply_file_action(&rename, "renamed.txt").unwrap();
    let delete = desk
        .prepare_file_action(
            repo.path(),
            Some(&repo.path().join("renamed.txt")),
            MenuAction::Delete,
        )
        .unwrap();
    std::fs::write(
        repo.path().join("renamed.txt"),
        "changed since confirmation",
    )
    .unwrap();
    assert!(desk.apply_file_action(&delete, "DELETE").is_err());
    assert!(repo.path().join("renamed.txt").exists());
    assert!(
        desk.prepare_file_action(repo.path(), Some(repo.path()), MenuAction::Delete)
            .is_err()
    );
    assert!(desk.edit_file(repo.path(), ".git/config").is_err());
}

#[test]
fn shared_git_reference_actions_respect_confirmation_snapshot() {
    use herdr_work_code::scm::{Drawer, MenuAction};
    let (repo, _home, desk) = fixture();
    git(repo.path(), &["tag", "delete-me"]);
    let view = desk.scm_drawer(repo.path(), Drawer::Tags, None).unwrap();
    let index = view.lines.iter().position(|r| r == "delete-me").unwrap();
    let plan = desk
        .prepare_scm_action(&view, index, MenuAction::DeleteTag)
        .unwrap();
    std::fs::write(repo.path().join("file.txt"), "new external edit").unwrap();
    assert!(desk.execute_git_plan(&plan).is_err());
    git(repo.path(), &["rev-parse", "refs/tags/delete-me"]);
    let plan = desk
        .prepare_scm_action(&view, index, MenuAction::DeleteTag)
        .unwrap();
    desk.execute_git_plan(&plan).unwrap();
    assert!(
        !Command::new("git")
            .args(["rev-parse", "--verify", "refs/tags/delete-me"])
            .current_dir(repo.path())
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[cfg(unix)]
#[test]
fn internal_editor_and_file_menu_cannot_follow_an_outside_symlink() {
    let (repo, _home, desk) = fixture();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("private.txt"), "third party data").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("private.txt"),
        repo.path().join("escape.txt"),
    )
    .unwrap();
    assert!(desk.edit_file(repo.path(), "escape.txt").is_err());
    assert!(
        desk.prepare_file_action(
            repo.path(),
            Some(&repo.path().join("escape.txt")),
            herdr_work_code::actions::MenuAction::Delete
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("private.txt")).unwrap(),
        "third party data"
    );
}
fn execution(root: &Path) -> Execution {
    Execution {
        id: ExecutionId("fixture-run".into()),
        task: TaskId("fixture/1".into()),
        attempt: None,
        worktree: root.into(),
        worktree_identity: HerdrWork::worktree_identity(root).unwrap(),
        origin_terminal: None,
        origin_socket: None,
        target: Some(AgentTarget {
            socket: "fixture.sock".into(),
            session: "fixture-session".into(),
            pane: "pane-1".into(),
            terminal: "terminal-1".into(),
        }),
    }
}
#[test]
fn git_changes_diff_stage_unstage_and_commit_use_upstream_logic() {
    let (repo, _home, desk) = fixture();
    let view = desk.diff(repo.path(), "file.txt", "working", None).unwrap();
    assert!(
        view.rows
            .iter()
            .any(|r| r.marker == '+' && r.text == "after")
    );
    let snap = desk.code_snapshot(repo.path(), &[], 1).unwrap();
    assert_eq!(snap.changes.len(), 1);
    desk.stage(&snap.changes[0], true).unwrap();
    let snap = desk.code_snapshot(repo.path(), &[], 2).unwrap();
    assert!(snap.changes[0].staged);
    desk.stage(&snap.changes[0], false).unwrap();
    let snap = desk.code_snapshot(repo.path(), &[], 3).unwrap();
    desk.stage(&snap.changes[0], true).unwrap();
    let plan = desk.prepare_commit(repo.path()).unwrap();
    desk.commit(&plan, "from HerdrWork").unwrap();
    assert!(
        desk.code_snapshot(repo.path(), &[], 4)
            .unwrap()
            .changes
            .is_empty()
    );
}
#[test]
fn commit_refuses_changes_to_previously_reviewed_index() {
    let (repo, _home, desk) = fixture();
    let snap = desk.code_snapshot(repo.path(), &[], 0).unwrap();
    desk.stage(&snap.changes[0], true).unwrap();
    let plan = desk.prepare_commit(repo.path()).unwrap();
    std::fs::write(repo.path().join("file.txt"), "changed again\n").unwrap();
    git(repo.path(), &["add", "file.txt"]);
    assert!(
        desk.commit(&plan, "must not commit")
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
}
#[test]
fn delivered_batch_preserves_comment_edited_during_send() {
    let (repo, _home, desk) = fixture();
    let review = desk
        .open_review(&execution(repo.path()), "working", None)
        .unwrap();
    let review = desk
        .add_comment(&review.id, "file.txt", Side::New, 1, 1, "check this")
        .unwrap();
    let batch = desk.prepare_send(&review.id).unwrap();
    assert!(batch.text.contains("file.txt:1"));
    desk.record_delivery(&review.id, &batch.id, Delivery::Sending)
        .unwrap();
    desk.edit_comment(&review.id, 1, "new comment text")
        .unwrap();
    let fresh = desk
        .record_delivery(&review.id, &batch.id, Delivery::Delivered)
        .unwrap();
    assert_eq!(fresh.comments.len(), 1);
    assert_eq!(fresh.comments[0].text, "new comment text");
}
#[test]
fn interrupted_send_recovers_unknown_and_cannot_automatically_repeat() {
    let (repo, home, desk) = fixture();
    let review = desk
        .open_review(&execution(repo.path()), "working", None)
        .unwrap();
    desk.add_comment(&review.id, "file.txt", Side::New, 1, 1, "review")
        .unwrap();
    let batch = desk.prepare_send(&review.id).unwrap();
    desk.record_delivery(&review.id, &batch.id, Delivery::Sending)
        .unwrap();
    drop(desk);
    let desk = HerdrWork::open(home.path(), None).unwrap();
    let review = desk.review(&review.id).unwrap();
    assert_eq!(review.batches[0].state, Delivery::Unknown);
    assert!(desk.prepare_send(&review.id).is_err());
    desk.resolve_batch(&review.id, &batch.id, false).unwrap();
    assert!(desk.prepare_send(&review.id).is_ok());
}
#[test]
fn changed_content_invalidates_review_even_when_head_has_not_changed() {
    let (repo, _home, desk) = fixture();
    let review = desk
        .open_review(&execution(repo.path()), "working", None)
        .unwrap();
    desk.add_comment(&review.id, "file.txt", Side::New, 1, 1, "review")
        .unwrap();
    std::fs::write(repo.path().join("file.txt"), "newer content\n").unwrap();
    assert!(
        desk.prepare_send(&review.id)
            .unwrap_err()
            .to_string()
            .contains("stale")
    );
}
#[test]
fn profile_is_exclusive_and_can_be_reopened_after_release() {
    let (_repo, home, desk) = fixture();
    assert!(HerdrWork::open(home.path(), None).is_err());
    drop(desk);
    assert!(HerdrWork::open(home.path(), None).is_ok());
}

#[test]
fn same_path_new_worktree_cannot_inherit_review_or_git_write_authority() {
    let (repo, _home, desk) = fixture();
    let old_execution = execution(repo.path());
    let review = desk.open_review(&old_execution, "working", None).unwrap();
    let change = desk
        .code_snapshot(repo.path(), &[], 0)
        .unwrap()
        .changes
        .remove(0);
    let moved = repo.path().with_extension("moved-fixture");
    std::fs::rename(repo.path(), &moved).unwrap();
    std::fs::create_dir(repo.path()).unwrap();
    git(repo.path(), &["init", "-q"]);
    assert!(HerdrWork::validate_execution(&old_execution).is_err());
    assert!(
        desk.add_comment(&review.id, "file.txt", Side::New, 1, 1, "old")
            .is_err()
    );
    assert!(desk.stage(&change, true).is_err());
    std::fs::remove_dir_all(moved).unwrap();
}

#[test]
fn unresolved_operation_requires_explicit_evidence_and_preserves_original_record() {
    let (_repo, home, desk) = fixture();
    std::fs::create_dir(home.path().join("operations")).unwrap();
    std::fs::write(
        home.path().join("operations/unknown.json"),
        r#"{"state":"started","task":"p/1"}"#,
    )
    .unwrap();
    assert!(
        desk.resolve_operation("../unknown.json", "checked")
            .is_err()
    );
    assert!(desk.resolve_operation("unknown.json", "").is_err());
    let resolved = desk
        .resolve_operation("unknown.json", "Confirmed stopped in original task data")
        .unwrap();
    assert_eq!(resolved["state"], "finished");
    assert_eq!(resolved["resolution"]["previous"], "started");
    assert_eq!(resolved["task"], "p/1");
}

#[test]
fn comment_composition_cannot_silently_move_to_new_content() {
    let (repo, _home, desk) = fixture();
    let review = desk
        .open_review(&execution(repo.path()), "working", None)
        .unwrap();
    let displayed = desk.diff(repo.path(), "file.txt", "working", None).unwrap();
    let anchor = herdr_work_core::CommentAnchor {
        file: "file.txt".into(),
        side: Side::New,
        start: 1,
        end: 1,
        fingerprint: displayed.fingerprint,
    };
    std::fs::write(repo.path().join("file.txt"), "changed while typing\n").unwrap();
    assert!(
        desk.add_comment_at(&review.id, &anchor, "comment on old text")
            .unwrap_err()
            .to_string()
            .contains("re-anchor")
    );
    assert!(desk.review(&review.id).unwrap().comments.is_empty());
}

#[test]
fn staging_refuses_external_file_changes_since_selection() {
    let (repo, _home, desk) = fixture();
    let change = desk
        .code_snapshot(repo.path(), &[], 0)
        .unwrap()
        .changes
        .remove(0);
    std::fs::write(repo.path().join("file.txt"), "external edit\n").unwrap();
    assert!(
        desk.stage(&change, true)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    let snap = desk.code_snapshot(repo.path(), &[], 1).unwrap();
    assert!(snap.changes.iter().all(|c| !c.staged));
}

#[cfg(unix)]
#[test]
fn git_hook_descendants_and_blocking_hooks_are_bounded_after_commit() {
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let (repo, _home, _desk) = fixture();
    git(repo.path(), &["add", "file.txt"]);
    let hook = repo.path().join(".git/hooks/post-commit");
    std::fs::write(&hook, "#!/bin/sh\nsleep 30 &\nexit 0\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let source = herdr_work_code::git::Git::discover(repo.path()).unwrap();
    let began = Instant::now();
    let result = herdr_work_code::git::herdr_work_bounded(
        |command, _| {
                .map_err(|e| e.to_string())
        },
        || source.commit("bounded hook"),
    );
    if let Err(error) = result {
        assert!(error.contains("timed out"));
    }
    assert!(began.elapsed() < Duration::from_secs(5));
    std::fs::write(repo.path().join("file.txt"), "another edit\n").unwrap();
    git(repo.path(), &["add", "file.txt"]);
    std::fs::write(&hook, "#!/bin/sh\nsleep 30\n").unwrap();
    let began = Instant::now();
    let result = herdr_work_code::git::herdr_work_bounded(
        |command, _| {
                .map_err(|e| e.to_string())
        },
        || source.commit("bounded blocking hook"),
    );
    assert!(result.unwrap_err().contains("timed out"));
    assert!(began.elapsed() < Duration::from_secs(5));
    // The commit can have landed before the pipe timeout, so it must be checked rather than blindly repeated.
    let head = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&head.stdout).trim(),
        "bounded blocking hook"
    );
}

#[test]
fn editor_observation_never_replaces_edits_made_while_the_disk_check_is_pending() {
    let (repo, _home, desk) = fixture();
    let mut session = desk.edit_file(repo.path(), "file.txt").unwrap();
    let request = session.external_request();
    std::fs::write(repo.path().join("file.txt"), "external\n").unwrap();
    let observed = request.observe().unwrap();
    session.editor.insert_text("local ");
    let cursor = session.editor.cursor();
    session.apply_external(observed);
    assert_eq!(session.editor.text(), "local after\n");
    assert_eq!(session.editor.cursor(), cursor);
    assert!(session.editor.external_changed);
    assert!(session.save().is_err());
    assert_eq!(
        std::fs::read_to_string(repo.path().join("file.txt")).unwrap(),
        "external\n"
    );
}

#[test]
fn editor_reload_and_overwrite_preserve_utf8_bom_and_crlf() {
    use herdr_work_code::editor::SaveOutcome;
    let (repo, _home, desk) = fixture();
    std::fs::write(
        repo.path().join("file.txt"),
        b"\xef\xbb\xbfhello\r\nworld\r\n",
    )
    .unwrap();
    let mut session = desk.edit_file(repo.path(), "file.txt").unwrap();
    std::fs::write(
        repo.path().join("file.txt"),
        b"\xef\xbb\xbfhello\r\nnew disk\r\n",
    )
    .unwrap();
    session.apply_external(session.external_request().observe().unwrap());
    session.editor.insert_text("界");
    assert_eq!(session.editor.text(), "界hello\nnew disk\n");
    std::fs::write(repo.path().join("file.txt"), "changed again\n").unwrap();
    assert_eq!(
        session.save_with_overwrite(false).unwrap(),
        SaveOutcome::Conflict
    );
    assert_eq!(
        session.save_with_overwrite(true).unwrap(),
        SaveOutcome::Saved
    );
    assert_eq!(
        std::fs::read(repo.path().join("file.txt")).unwrap(),
        "\u{feff}界hello\r\nnew disk\r\n".as_bytes()
    );
}

#[cfg(unix)]
#[test]
fn editor_reload_observe_and_explicit_overwrite_reject_replaced_path_or_root() {
    use std::os::unix::fs::symlink;
    let (repo, _home, desk) = fixture();
    let outside = tempfile::tempdir().unwrap();
    let protected = outside.path().join("private.txt");
    std::fs::write(&protected, "private").unwrap();
    let mut session = desk.edit_file(repo.path(), "file.txt").unwrap();
    std::fs::remove_file(repo.path().join("file.txt")).unwrap();
    symlink(&protected, repo.path().join("file.txt")).unwrap();
    assert!(session.external_request().observe().is_err());
    assert!(session.reload().is_err());
    assert!(session.save_with_overwrite(true).is_err());
    assert_eq!(std::fs::read_to_string(protected).unwrap(), "private");

    let root = outside.path().join("worktree");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("file.txt"), "old root").unwrap();
    let mut session = desk.edit_file(&root, "file.txt").unwrap();
    std::fs::rename(&root, outside.path().join("old-worktree")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("file.txt"), "replacement root").unwrap();
    assert!(session.external_request().observe().is_err());
    assert!(session.reload().is_err());
    assert!(session.save_with_overwrite(true).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).unwrap(),
        "replacement root"
    );
}

#[test]
fn file_history_show_contains_colored_patch_only_for_selected_file() {
    use herdr_work_code::scm::Drawer;
    let (repo, _home, desk) = fixture();
    std::fs::write(repo.path().join("other.txt"), "unrelated-patch\n").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "change both"]);
    let history = desk
        .scm_drawer(repo.path(), Drawer::FileHistory, Some("file.txt"))
        .unwrap();
    let patch = desk.scm_show(&history, 0).unwrap();
    let plain = herdr_work_code::ansi::to_lines(&patch)
        .into_iter()
        .map(|l| {
            l.spans
                .into_iter()
                .map(|s| s.content.into_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plain.contains("+after"), "missing actual patch: {patch}");
    assert!(
        patch.contains("\u{1b}["),
        "patch should retain source colors"
    );
    assert!(!patch.contains("unrelated-patch"));
    assert!(!patch.contains("other.txt"));
}

#[test]
fn readonly_preview_uses_source_highlight_and_bounds_large_text() {
    let (repo, _home, desk) = fixture();
    std::fs::write(
        repo.path().join("code.rs"),
        "fn main() { println!(\"界\"); }\n",
    )
    .unwrap();
    let doc = desk.preview_file(repo.path(), "code.rs", None, 60).unwrap();
    assert!(doc.numbered());
    assert!(
        doc.lines()
            .iter()
            .flat_map(|l| &l.spans)
            .any(|s| s.style.fg.is_some())
    );
    std::fs::write(repo.path().join("large.txt"), "line\n".repeat(6000)).unwrap();
    let doc = desk
        .preview_file(repo.path(), "large.txt", Some(80), 60)
        .unwrap();
    assert!(doc.plain_text().ends_with("… (truncated)"));
    assert_eq!(doc.lines().len(), 5001);
}
