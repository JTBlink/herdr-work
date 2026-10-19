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
            herdr_reviewr::proc::herdr_work_run(command, Duration::from_secs(1))
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
            herdr_reviewr::proc::herdr_work_run(command, Duration::from_secs(1))
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
fn bulk_git_confirmation_tracks_untracked_bytes_and_respects_nested_ownership() {
    use herdr_work_engine::WorkGitAction;
    let (repo, _home, desk) = fixture();
    std::fs::write(repo.path().join("new.txt"), "first").unwrap();
    let plan = desk
        .prepare_work_git(repo.path(), WorkGitAction::StageAll)
        .unwrap();
    std::fs::write(repo.path().join("new.txt"), "second").unwrap();
    assert!(desk.execute_work_git(&plan).is_err());
    let nested = repo.path().join("inner");
    std::fs::create_dir(&nested).unwrap();
    git(&nested, &["init", "-q"]);
    std::fs::write(nested.join("own.txt"), "nested").unwrap();
    let plan = desk
        .prepare_work_git(repo.path(), WorkGitAction::StageAll)
        .unwrap();
    desk.execute_work_git(&plan).unwrap();
    let snapshot = desk.code_snapshot(repo.path(), &[], 1).unwrap();
    assert!(
        snapshot
            .changes
            .iter()
            .any(|c| c.staged && c.path == "new.txt")
    );
    assert!(
        !snapshot
            .changes
            .iter()
            .any(|c| c.staged && c.path.starts_with("inner"))
    );
    let plan = desk
        .prepare_explorer_stage(&nested.join("own.txt"))
        .unwrap();
    assert_eq!(plan.root, nested.canonicalize().unwrap());
    desk.execute_work_git(&plan).unwrap();
    git(
        &nested,
        &["diff", "--cached", "--exit-code", "--", "absent"],
    );
    assert!(
        desk.code_snapshot(repo.path(), &[], 2)
            .unwrap()
            .changes
            .iter()
            .any(|c| c.root == nested.canonicalize().unwrap() && c.staged && c.path == "own.txt")
    );
}

#[test]
fn explorer_staging_literal_path_does_not_stage_a_matching_sibling() {
    let (repo, _home, desk) = fixture();
    std::fs::write(repo.path().join("[a].txt"), "literal").unwrap();
    std::fs::write(repo.path().join("a.txt"), "sibling").unwrap();
    let plan = desk
        .prepare_explorer_stage(&repo.path().join("[a].txt"))
        .unwrap();
    desk.execute_work_git(&plan).unwrap();
    let snapshot = desk.code_snapshot(repo.path(), &[], 1).unwrap();
    assert!(
        snapshot
            .changes
            .iter()
            .any(|c| c.path == "[a].txt" && c.staged)
    );
    assert!(
        !snapshot
            .changes
            .iter()
            .any(|c| c.path == "a.txt" && c.staged)
    );
}

#[test]
fn confirmed_folder_bulk_stash_and_discard_keep_other_repositories_separate() {
    use herdr_work_engine::WorkGitAction as A;
    let (repo, _home, desk) = fixture();
    std::fs::create_dir(repo.path().join("dir")).unwrap();
    std::fs::write(repo.path().join("dir/new.txt"), "untracked").unwrap();
    let apply = |action| {
        let plan = desk.prepare_work_git(repo.path(), action).unwrap();
        desk.execute_work_git(&plan).unwrap()
    };
    apply(A::StageUnder("dir".into()));
    let snapshot = desk.code_snapshot(repo.path(), &[], 1).unwrap();
    assert!(
        snapshot
            .changes
            .iter()
            .any(|c| c.path == "dir/new.txt" && c.staged)
    );
    assert!(
        !snapshot
            .changes
            .iter()
            .any(|c| c.path == "file.txt" && c.staged)
    );
    apply(A::UnstageUnder("dir".into()));
    apply(A::Stash);
    assert!(!repo.path().join("dir/new.txt").exists());
    assert_eq!(
        std::fs::read_to_string(repo.path().join("file.txt")).unwrap(),
        "before\n"
    );
    git(repo.path(), &["stash", "pop", "-q"]);
    apply(A::Discard("dir/new.txt".into()));
    assert!(!repo.path().join("dir/new.txt").exists());
    apply(A::StageAll);
    apply(A::UnstageAll);
    apply(A::DiscardAll);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("file.txt")).unwrap(),
        "before\n"
    );
}

#[test]
fn branch_delete_requires_a_second_force_plan_and_explicit_reconciliation_does_not_retry() {
    use herdr_work_engine::{WorkGitAction as A, WorkGitOutcome};
    let (repo, _home, desk) = fixture();
    git(repo.path(), &["checkout", "-qb", "topic"]);
    std::fs::write(repo.path().join("topic.txt"), "topic").unwrap();
    git(repo.path(), &["add", "topic.txt"]);
    git(repo.path(), &["commit", "-qm", "topic"]);
    git(repo.path(), &["checkout", "-q", "-"]);
    assert!(
        desk.prepare_work_git(repo.path(), A::ForceDeleteBranch("topic".into()))
            .is_err()
    );
    let plan = desk
        .prepare_work_git(repo.path(), A::DeleteBranch("topic".into()))
        .unwrap();
    let WorkGitOutcome::DeleteNotMerged(force) = desk.execute_work_git(&plan).unwrap() else {
        panic!("unmerged branch was deleted without a second confirmation")
    };
    git(repo.path(), &["rev-parse", "--verify", "refs/heads/topic"]);
    desk.execute_work_git(&force).unwrap();
    let plan = desk
        .prepare_work_git(repo.path(), A::CreateBranch("bad..name".into()))
        .unwrap();
    assert!(desk.execute_work_git(&plan).is_err());
    assert!(
        desk.execute_work_git(&plan)
            .err()
            .unwrap()
            .to_string()
            .contains("未知")
    );
    let records = desk.git_reconciliation().unwrap();
    let operation = records["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["record"]["state"] == "unknown")
        .unwrap();
    assert!(operation["record"]["observed"]["head"].is_string());
    desk.resolve_operation(
        operation["operation"].as_str().unwrap(),
        "Checked HEAD and refs; invalid branch was not created",
    )
    .unwrap();
    assert_eq!(
        desk.git_reconciliation().unwrap()["operations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["record"]["state"] == "unknown")
            .count(),
        0
    );
}

#[test]
fn remote_branch_checkout_establishes_tracking_and_sync_pushes_the_active_repository() {
    use herdr_work_engine::WorkGitAction as A;
    let (repo, _home, desk) = fixture();
    let remote = tempfile::tempdir().unwrap();
    git(remote.path(), &["init", "--bare", "-q"]);
    git(
        repo.path(),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git(repo.path(), &["push", "-qu", "origin", "HEAD"]);
    git(repo.path(), &["branch", "remote-topic"]);
    git(repo.path(), &["push", "-q", "origin", "remote-topic"]);
    git(repo.path(), &["branch", "-D", "remote-topic"]);
    let plan = desk
        .prepare_branch_checkout(repo.path(), "origin/remote-topic")
        .unwrap();
    desk.execute_work_git(&plan).unwrap();
    let snapshot = desk.code_snapshot(repo.path(), &[], 1).unwrap();
    assert_eq!(snapshot.repositories[0].branch, "remote-topic");
    assert!(snapshot.repositories[0].has_upstream);
    git(repo.path(), &["commit", "--allow-empty", "-qm", "ahead"]);
    assert_eq!(
        desk.code_snapshot(repo.path(), &[], 2)
            .unwrap()
            .repositories[0]
            .ahead,
        1
    );
    let plan = desk.prepare_work_git(repo.path(), A::Sync).unwrap();
    desk.execute_work_git(&plan).unwrap();
    assert_eq!(
        desk.code_snapshot(repo.path(), &[], 3)
            .unwrap()
            .repositories[0]
            .ahead,
        0
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("file.txt")).unwrap(),
        "after\n"
    );
    let plan = desk
        .prepare_work_git(repo.path(), A::CreateBranch("new-local".into()))
        .unwrap();
    desk.execute_work_git(&plan).unwrap();
    assert!(desk.prepare_work_git(repo.path(), A::Sync).is_err());
}

#[test]
fn commit_confirmation_binds_the_branch_even_when_the_head_oid_is_identical() {
    let (repo, _home, desk) = fixture();
    git(repo.path(), &["add", "file.txt"]);
    let plan = desk.prepare_commit(repo.path()).unwrap();
    git(repo.path(), &["checkout", "-qb", "other-same-head"]);
    assert!(
        desk.commit(&plan, "must not commit on a different branch")
            .is_err()
    );
    assert!(desk.prepare_commit(repo.path()).is_ok());
}

#[test]
fn folder_stage_and_unstage_keep_both_sides_of_a_detected_rename_together() {
    use herdr_work_engine::WorkGitAction as A;
    let (repo, _home, desk) = fixture();
    std::fs::write(repo.path().join("file.txt"), "before\n").unwrap();
    std::fs::create_dir(repo.path().join("target")).unwrap();
    std::fs::rename(
        repo.path().join("file.txt"),
        repo.path().join("target/file.txt"),
    )
    .unwrap();
    git(repo.path(), &["add", "-N", "target/file.txt"]);
    let plan = desk
        .prepare_work_git(repo.path(), A::StageUnder("target".into()))
        .unwrap();
    desk.execute_work_git(&plan).unwrap();
    let snapshot = desk.code_snapshot(repo.path(), &[], 1).unwrap();
    assert_eq!(snapshot.changes.len(), 1);
    assert!(snapshot.changes[0].staged);
    assert_eq!(snapshot.changes[0].original.as_deref(), Some("file.txt"));
    let plan = desk
        .prepare_work_git(repo.path(), A::UnstageUnder("target".into()))
        .unwrap();
    desk.execute_work_git(&plan).unwrap();
    assert!(
        desk.code_snapshot(repo.path(), &[], 2)
            .unwrap()
            .changes
            .iter()
            .all(|c| !c.staged)
    );
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

#[test]
fn media_and_lossy_text_previews_refresh_only_after_metadata_changes() {
    let (repo, _home, desk) = fixture();
    std::fs::write(
        repo.path().join("image.png"),
        include_bytes!("fixtures/preview.png"),
    )
    .unwrap();
    let image = desk
        .preview_file(repo.path(), "image.png", None, 60)
        .unwrap();
    assert!(image.is_media());
    assert_eq!(image.media_hint(), Some("图片预览"));
    assert!(
        desk.preview_file_if_changed(repo.path(), "image.png", image.stamp(), None, 60)
            .unwrap()
            .is_none()
    );
    std::fs::write(repo.path().join("image.png"), "invalid changed image").unwrap();
    let changed = desk
        .preview_file_if_changed(repo.path(), "image.png", image.stamp(), None, 60)
        .unwrap()
        .unwrap();
    assert!(!changed.is_media());
    assert!(!changed.plain_text().is_empty());
    std::fs::write(repo.path().join("lossy.txt"), [b'a', 0xff, b'b']).unwrap();
    assert_eq!(
        desk.preview_file(repo.path(), "lossy.txt", None, 60)
            .unwrap()
            .plain_text(),
        "a�b"
    );
    std::fs::write(repo.path().join("binary.bin"), [b'a', 0, b'b']).unwrap();
    assert!(
        desk.preview_file(repo.path(), "binary.bin", None, 60)
            .unwrap()
            .plain_text()
            .contains("binary file")
    );
}

#[test]
fn large_untracked_sparse_file_keeps_read_only_lists_usable_and_refuses_incomplete_write_plans() {
    use herdr_work_engine::WorkGitAction;
    let (repo, _home, desk) = fixture();
    let path = repo.path().join("large-untracked.bin");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(256 * 1024 * 1024 + 1)
        .unwrap();
    let snapshot = desk
        .code_snapshot(repo.path(), &[], 1)
        .expect("large changes must not hide Explorer or SCM");
    assert!(
        snapshot
            .files
            .iter()
            .any(|row| row.path.ends_with("large-untracked.bin"))
    );
    assert!(
        snapshot
            .files
            .iter()
            .any(|row| row.path.ends_with("file.txt"))
    );
    assert!(
        snapshot.repositories[0]
            .write_error
            .as_deref()
            .is_some_and(|error| error.contains("256 MiB"))
    );
    assert_eq!(desk.file(repo.path(), "file.txt").unwrap(), "after\n");
    let large = snapshot
        .changes
        .iter()
        .find(|c| c.path == "large-untracked.bin")
        .unwrap();
    assert!(!large.staged);
    assert!(large.write_error.is_some());
    assert!(
        desk.stage(large, true).is_err(),
        "a read-only snapshot must never authorize a write"
    );
    assert!(
        desk.prepare_work_git(repo.path(), WorkGitAction::StageAll)
            .is_err()
    );
    assert!(
        desk.prepare_work_git(repo.path(), WorkGitAction::Stash)
            .is_err()
    );
    assert!(desk.prepare_explorer_stage(&path).is_err());
    assert_eq!(path.metadata().unwrap().len(), 256 * 1024 * 1024 + 1);
    git(repo.path(), &["diff", "--cached", "--exit-code"]);
}

#[test]
#[ignore = "requires the optional glow and ffmpeg executables"]
fn installed_preview_tools_render_markdown_and_a_real_video_poster() {
    let (repo, _home, desk) = fixture();
    std::fs::write(
        repo.path().join("note.md"),
        "# Preview fixture\n\n**formatted body**\n",
    )
    .unwrap();
    let markdown = desk.preview_file(repo.path(), "note.md", None, 60).unwrap();
    assert!(
        !markdown.numbered(),
        "glow output must replace numbered source"
    );
    let rendered = markdown
        .lines()
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("formatted body"));
    assert!(!rendered.contains("**formatted body**"));

    let video = repo.path().join("poster.mp4");
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=48x32:d=0.2",
            "-c:v",
            "mpeg4",
            "-y",
        ])
        .arg(&video)
        .output()
        .expect("ffmpeg must be installed to run this opt-in test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let poster = desk
        .preview_file(repo.path(), "poster.mp4", None, 60)
        .unwrap();
    assert!(poster.is_media(), "{}", poster.plain_text());
    assert_eq!(poster.media_hint(), Some("视频封面"));
}

#[test]
fn product_workspace_state_is_root_scoped_and_rejects_escaped_paths() {
    use std::path::PathBuf;
    let (repo, _home, desk) = fixture();
    std::fs::create_dir(repo.path().join("folder")).unwrap();
    std::fs::write(repo.path().join("folder/child.txt"), "child").unwrap();
    let mut saved = desk.workspace_ui_state(repo.path()).unwrap();
    saved.tree.expanded = vec![PathBuf::from("folder")];
    saved.tree.selected = Some(PathBuf::from("folder/child.txt"));
    desk.save_workspace_ui_state(repo.path(), &saved).unwrap();
    let restored = desk.workspace_ui_state(repo.path()).unwrap();
    assert_eq!(
        restored.tree.selected,
        Some(PathBuf::from("folder/child.txt"))
    );
    let other = tempfile::tempdir().unwrap();
    assert!(
        desk.workspace_ui_state(other.path())
            .unwrap()
            .tree
            .expanded
            .is_empty()
    );
    saved.tree.selected = Some(PathBuf::from("../outside.txt"));
    assert!(desk.save_workspace_ui_state(repo.path(), &saved).is_err());
    assert_eq!(
        desk.workspace_ui_state(repo.path()).unwrap().tree.selected,
        Some(PathBuf::from("folder/child.txt"))
    );
}

#[test]
fn workspace_final_save_cannot_be_overwritten_by_an_older_background_snapshot() {
    let (repo, _home, desk) = fixture();
    let mut old = desk.workspace_ui_state(repo.path()).unwrap();
    old.revision = 1;
    let mut final_state = old.clone();
    final_state.revision = 2;
    final_state.tree.selected = Some("file.txt".into());
    desk.save_workspace_ui_state(repo.path(), &final_state)
        .unwrap();
    desk.save_workspace_ui_state(repo.path(), &old).unwrap();
    assert_eq!(
        desk.workspace_ui_state(repo.path()).unwrap().tree.selected,
        Some("file.txt".into())
    );
}

#[test]
fn stale_setting_changes_preserve_other_fields_written_since_they_were_observed() {
    let (_repo, _home, desk) = fixture();
    let observed = desk.ui_preferences().unwrap();
    let mut another = observed.clone();
    another.dock_right = true;
    desk.update_ui_preferences(&observed, &another).unwrap();
    let mut local = observed.clone();
    local.show_hotkeys = false;
    let saved = desk.update_ui_preferences(&observed, &local).unwrap();
    assert!(saved.dock_right);
    assert!(!saved.show_hotkeys);
    assert!(desk.ui_preferences().unwrap().dock_right);
}

#[test]
fn disabling_explorer_decorations_keeps_scm_changes_and_repository_identity() {
    let (repo, _home, desk) = fixture();
    std::fs::write(repo.path().join("file.txt"), "changed\n").unwrap();
    let enabled = desk.code_snapshot(repo.path(), &[], 1).unwrap();
    assert!(enabled.files.iter().any(|row| row.status.is_some()));
    let disabled = desk
        .code_snapshot_with_options(repo.path(), &[], 2, true, false)
        .unwrap();
    assert!(disabled.files.iter().all(|row| row.status.is_none()));
    assert_eq!(disabled.repositories.len(), enabled.repositories.len());
    assert_eq!(disabled.changes.len(), enabled.changes.len());
    assert_eq!(disabled.changes[0].instance, enabled.changes[0].instance);
    assert_eq!(
        disabled.changes[0].expected_index,
        enabled.changes[0].expected_index
    );
}

#[test]
fn workspace_state_does_not_follow_a_replaced_directory_instance() {
    let (_repo, home, desk) = fixture();
    let root = home.path().join("browse-root");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("same.txt"), "one").unwrap();
    let mut state = desk.workspace_ui_state(&root).unwrap();
    state.activity = "git".into();
    state.tree.selected = Some("same.txt".into());
    desk.save_workspace_ui_state(&root, &state).unwrap();
    std::fs::rename(&root, home.path().join("old-root")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("same.txt"), "two").unwrap();
    let fresh = desk.workspace_ui_state(&root).unwrap();
    assert_ne!(fresh.identity, state.identity);
    assert!(fresh.activity.is_empty());
    assert_eq!(fresh.tree.selected, None);
    assert!(desk.save_workspace_ui_state(&root, &state).is_err());
}

#[test]
fn saved_drawer_identity_rejects_a_replaced_nested_repository() {
    use herdr_work_code::scm::Drawer;
    use herdr_work_engine::DrawerState;
    let (repo, _home, desk) = fixture();
    let nested = repo.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    git(&nested, &["init", "-q"]);
    let view = desk.scm_drawer(&nested, Drawer::Branches, None).unwrap();
    let saved = DrawerState {
        kind: Drawer::Branches.title().into(),
        root: herdr_work_engine::DrawerRoot::Workspace("nested".into()),
        instance: view.instance,
        file: None,
    };
    let mut state = desk.workspace_ui_state(repo.path()).unwrap();
    state.drawers.push(saved.clone());
    desk.save_workspace_ui_state(repo.path(), &state).unwrap();
    std::fs::rename(&nested, repo.path().join("previous-nested")).unwrap();
    std::fs::create_dir(&nested).unwrap();
    git(&nested, &["init", "-q"]);
    assert!(desk.restore_ui_drawer(repo.path(), &saved).is_err());
    assert!(
        desk.workspace_ui_state(repo.path())
            .unwrap()
            .drawers
            .is_empty()
    );
}
