//! Exercise real CLI, original readiness check and original socket transport with a private host.
#![cfg(unix)]
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

fn git(root: &Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap()
            .status
            .success()
    );
}
fn script(path: &Path, text: String) {
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}
#[test]
fn ambiguous_post_send_receipt_is_unknown_and_cannot_resend() {
    let temp = tempfile::tempdir_in("/tmp").unwrap();
    let root = temp.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "fixture@example.invalid"]);
    std::fs::write(root.join("file.txt"), "before\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "fixture"]);
    std::fs::write(root.join("file.txt"), "after\n").unwrap();
    let root = root.canonicalize().unwrap();
    let socket = temp.path().join("host.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let sends = Arc::new(AtomicUsize::new(0));
    let count = sends.clone();
    let agents = json!([{"pane_id":"p1","agent":"codex","agent_status":"idle","cwd":root,"workspace_id":"w1","tab_id":"t1"}]);
    let snapshot = json!({"session_id":"fixture","agents":agents,"panes":[{"pane_id":"p1","terminal_id":"terminal-1"}]});
    let host = std::thread::spawn(move || {
        while !stopping.load(Ordering::SeqCst) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            let reply = if request["method"] == "session.snapshot" {
                json!({"result":snapshot})
            } else if request["method"] == "pane.send_text" {
                count.fetch_add(1, Ordering::SeqCst);
                json!({"ambiguous":"host applied paste but response malformed"})
            } else {
                json!({"result":{}})
            };
            writeln!(stream, "{reply}").unwrap();
        }
    });
    let herdr = temp.path().join("herdr");
    script(
        &herdr,
        format!(
            "#!/bin/sh\ncat <<'JSON'\n{}\nJSON\n",
            json!({"result":{"agents":agents}})
        ),
    );
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_workdesk"))
            .args(["--home", temp.path().join("profile").to_str().unwrap()])
            .args(args)
            .env("HERDR_SOCKET_PATH", &socket)
            .env("HERDR_BIN_PATH", &herdr)
            .output()
            .unwrap()
    };
    let opened = run(&[
        "review",
        "open",
        "--worktree",
        root.to_str().unwrap(),
        "--pane",
        "p1",
    ]);
    assert!(
        opened.status.success(),
        "{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    let review: Value = serde_json::from_slice(&opened.stdout).unwrap();
    let id = review["id"].as_str().unwrap();
    assert!(
        run(&[
            "review", "add", id, "--file", "file.txt", "--line", "1", "check"
        ])
        .status
        .success()
    );
    let sent = run(&["review", "send", id]);
    assert!(
        sent.status.success(),
        "{}",
        String::from_utf8_lossy(&sent.stderr)
    );
    let receipt: Value = serde_json::from_slice(&sent.stdout).unwrap();
    assert_eq!(receipt["state"], "Unknown");
    assert!(!run(&["review", "send", id]).status.success());
    assert_eq!(sends.load(Ordering::SeqCst), 1);
    // A task's older terminal cannot be silently rebound to the current same-cwd agent.
    let backend = temp.path().join("backend");
    script(
        &backend,
        format!(
            "#!/bin/sh\ncat <<'JSON'\n{}\nJSON\n",
            json!({"tasks":[{"id":"task/1","title":"old execution","worktree":root,"link":{"terminal":"terminal-old","socket":socket}}]})
        ),
    );
    let refused = run(&[
        "--projects-backend",
        backend.to_str().unwrap(),
        "review",
        "open",
        "--task",
        "task/1",
        "--pane",
        "p1",
    ]);
    assert!(!refused.status.success());
    stop.store(true, Ordering::SeqCst);
    host.join().unwrap();
}
