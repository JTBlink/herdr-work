use std::{path::Path, process::Command};
use workdesk_core::{AgentTarget, Delivery, Execution, ExecutionId, Side, TaskId};
use workdesk_engine::Workdesk;

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
fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Workdesk) {
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
    let desk = Workdesk::open(home.path(), None).unwrap();
    (repo, home, desk)
}
fn execution(root: &Path) -> Execution {
    Execution {
        id: ExecutionId("fixture-run".into()),
        task: TaskId("fixture/1".into()),
        attempt: None,
        worktree: root.into(),
        worktree_identity: Workdesk::worktree_identity(root).unwrap(),
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
    desk.commit(&plan, "from workdesk").unwrap();
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
    let desk = Workdesk::open(home.path(), None).unwrap();
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
fn profile_is_exclusive_and_preview_import_is_read_only_and_idempotent() {
    let (_repo, home, desk) = fixture();
    assert!(Workdesk::open(home.path(), None).is_err());
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(source.path().join("data/tasks/p/1")).unwrap();
    std::fs::write(
        source.path().join("data/tasks/p/1/task.json"),
        "{\"id\":\"p/1\"}",
    )
    .unwrap();
    let preview = desk.import_preview(source.path()).unwrap();
    assert!(!desk.projects_home().exists());
    desk.import_projects(&preview).unwrap();
    desk.import_projects(&preview).unwrap();
    assert!(
        desk.projects_home()
            .join("data/tasks/p/1/task.json")
            .exists()
    );
    std::fs::write(desk.projects_home().join("new-file"), "new work").unwrap();
    assert!(desk.rollback_import().is_err());
}

#[test]
fn interrupted_import_after_backup_rename_finishes_from_verified_staging() {
    let (_repo, home, desk) = fixture();
    let source = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("tasks.json"), "imported").unwrap();
    let preview = desk.import_preview(source.path()).unwrap();
    std::fs::create_dir(desk.projects_home()).unwrap();
    std::fs::write(desk.projects_home().join("old"), "original").unwrap();
    let stage = home.path().join("staged-import");
    std::fs::create_dir(&stage).unwrap();
    std::fs::copy(source.path().join("tasks.json"), stage.join("tasks.json")).unwrap();
    std::fs::write(
        home.path().join("import.pending.json"),
        serde_json::to_vec(&serde_json::json!({
            "phase":"import", "preview":preview, "staged":"staged-import", "had_target":true
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::rename(
        desk.projects_home(),
        home.path().join("projects.before-import"),
    )
    .unwrap();
    drop(desk);
    let desk = Workdesk::open(home.path(), None).unwrap();
    assert_eq!(
        std::fs::read_to_string(desk.projects_home().join("tasks.json")).unwrap(),
        "imported"
    );
    desk.rollback_import().unwrap();
    assert_eq!(
        std::fs::read_to_string(desk.projects_home().join("old")).unwrap(),
        "original"
    );
    assert!(!home.path().join("import.pending.json").exists());
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
    assert!(Workdesk::validate_execution(&old_execution).is_err());
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
    let anchor = workdesk_core::CommentAnchor {
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
fn git_hook_descendant_holding_pipes_cannot_block_the_worker_forever() {
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let (repo, _home, _desk) = fixture();
    git(repo.path(), &["add", "file.txt"]);
    let hook = repo.path().join(".git/hooks/post-commit");
    std::fs::write(&hook, "#!/bin/sh\nsleep 30 &\nexit 0\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let source = herdr_sidebar::git::Git::discover(repo.path()).unwrap();
    let began = Instant::now();
    let result = herdr_sidebar::git::workdesk_bounded(
        |command, _| {
            herdr_reviewr::proc::workdesk_run(command, Duration::from_millis(250))
                .map_err(|e| e.to_string())
        },
        || source.commit("bounded hook"),
    );
    assert!(result.unwrap_err().contains("timed out"));
    assert!(began.elapsed() < Duration::from_secs(3));
    // The commit can have landed before the pipe timeout, so it must be checked rather than blindly repeated.
    let head = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "bounded hook");
}
