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
        Command::new(env!("CARGO_BIN_EXE_herdr-work"))
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

#[test]
fn activate_reuses_only_verified_own_pane_even_while_profile_is_locked() {
    use std::sync::Mutex;
    let temp = tempfile::tempdir_in("/tmp").unwrap();
    let root = temp.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let profile = temp.path().join("profile");
    let desk = herdr_work_engine::HerdrWork::open(&profile, None).unwrap();
    let profile = profile.canonicalize().unwrap();
    let token = herdr_work_engine::fingerprint(profile.to_string_lossy().as_bytes());
    let socket = temp.path().join("activate.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorded = requests.clone();
    let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_herdr-work"))
        .canonicalize()
        .unwrap();
    let host = std::thread::spawn(move || {
        while !stopping.load(Ordering::SeqCst) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            recorded.lock().unwrap().push(request.clone());
            let result = match request["method"].as_str().unwrap() {
                "pane.list" => json!({"panes":[
                    {"pane_id":"independent-sidebar","label":"HerdrWork","tokens":{"herdr-work-code-explorer":123}},
                    {"pane_id":"own-pane","tokens":{"jtblink.herdr-work":token,"herdr-work-instance":"fixture-instance","herdr-work-ready":true}}
                ]}),
                "pane.process_info" => {
                    json!({"process_info":{"pane_id":"own-pane","foreground_processes":[{"argv0":binary,"argv":[binary,"ui","."]}]}})
                }
                "pane.focus" => json!({}),
                other => panic!("unexpected host operation: {other}"),
            };
            writeln!(stream, "{}", json!({"result":result})).unwrap();
        }
    });
    let result = Command::new(env!("CARGO_BIN_EXE_herdr-work"))
        .args([
            "--home",
            profile.to_str().unwrap(),
            "activate",
            "show-search",
            "--root",
            root.to_str().unwrap(),
        ])
        .env("HERDR_SOCKET_PATH", &socket)
        .output()
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    host.join().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let requests = requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|r| r["method"] == "pane.focus" && r["params"]["pane_id"] == "own-pane")
    );
    assert!(
        requests
            .iter()
            .filter(|r| r["method"] == "pane.process_info")
            .all(|r| r["params"]["pane_id"] == "own-pane")
    );
    assert!(!requests.iter().any(|r| matches!(
        r["method"].as_str(),
        Some("pane.send_input" | "pane.close" | "pane.run" | "plugin.pane.open")
    )));
    let registration = herdr_work_engine::WorkbenchRegistration {
        socket,
        pane: "own-pane".into(),
        profile: herdr_work_engine::fingerprint(profile.to_string_lossy().as_bytes()),
        instance: "fixture-instance".into(),
    };
    let request = desk
        .pending_activation(&registration, None)
        .unwrap()
        .unwrap();
    assert_eq!(request.view, herdr_work_engine::InitialView::Search);
    assert_eq!(request.root, root.canonicalize().unwrap());
    assert!(
        desk.pending_activation(&registration, Some(&request.nonce))
            .unwrap()
            .is_none()
    );
    let restarted = herdr_work_engine::WorkbenchRegistration {
        instance: "new-ui-instance".into(),
        ..registration
    };
    assert!(desk.pending_activation(&restarted, None).unwrap().is_none());
}

fn activation_case(
    forged_own_identity: bool,
    foreground: Option<Value>,
) -> (std::process::Output, Vec<Value>) {
    use std::sync::Mutex;
    let temp = tempfile::tempdir_in("/tmp").unwrap();
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    let profile = profile.canonicalize().unwrap();
    let token = herdr_work_engine::fingerprint(profile.to_string_lossy().as_bytes());
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let socket = temp.path().join("host.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let requests = Arc::new(Mutex::new(vec![]));
    let recorded = requests.clone();
    let host = std::thread::spawn(move || {
        while !stopping.load(Ordering::SeqCst) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            recorded.lock().unwrap().push(request.clone());
            let result = match request["method"].as_str().unwrap() {
                "pane.list" if forged_own_identity => {
                    json!({"panes":[{"pane_id":"unverified","tokens":{"jtblink.herdr-work":token,"herdr-work-instance":"stale","herdr-work-ready":true}}]})
                }
                "pane.list" => {
                    json!({"panes":[{"pane_id":"independent-projects","label":"HerdrWork","tokens":{"herdr-projects":"own-other-plugin"}}]})
                }
                "pane.process_info" if forged_own_identity => {
                    json!({"process_info":{"pane_id":"unverified","foreground_processes":[foreground.clone().unwrap_or_else(|| json!({"argv0":"/bin/sh","argv":["/bin/sh","ui"]}))]}})
                }
                "plugin.pane.open" if !forged_own_identity => {
                    json!({"plugin_pane":{"plugin_id":"jtblink.herdr-work","entrypoint":"pane","pane":{"pane_id":"created-own"}}})
                }
                "pane.report_metadata" if !forged_own_identity => json!({}),
                other => panic!("unexpected host mutation {other}"),
            };
            writeln!(stream, "{}", json!({"result":result})).unwrap();
        }
    });
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-work"))
        .args([
            "--home",
            profile.to_str().unwrap(),
            "activate",
            "quick-open",
        ])
        .env("HERDR_SOCKET_PATH", &socket)
        .env(
            "HERDR_PLUGIN_CONTEXT_JSON",
            json!({"workspace_id":"workspace-fixture","focused_pane_cwd":root}).to_string(),
        )
        .output()
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    host.join().unwrap();
    let requests = requests.lock().unwrap().clone();
    (output, requests)
}
#[test]
fn activate_allocates_a_tty_only_through_its_own_host_entrypoint() {
    let (output, requests) = activation_case(false, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let open = requests
        .iter()
        .find(|r| r["method"] == "plugin.pane.open")
        .unwrap();
    assert_eq!(open["params"]["plugin_id"], "jtblink.herdr-work");
    assert_eq!(open["params"]["entrypoint"], "pane");
    assert_eq!(open["params"]["placement"], "tab");
    assert_eq!(open["params"]["workspace_id"], "workspace-fixture");
    assert_eq!(
        open["params"]["env"]["HERDR_WORK_INITIAL_VIEW"],
        "quick-open"
    );
    assert!(
        open["params"]["env"]
            .as_object()
            .unwrap()
            .keys()
            .all(|k| k.starts_with("HERDR_WORK_"))
    );
    assert_eq!(
        open["params"]["cwd"],
        open["params"]["env"]["HERDR_WORK_ROOT"]
    );
    assert!(requests.iter().all(|r| matches!(
        r["method"].as_str(),
        Some("pane.list" | "plugin.pane.open" | "pane.report_metadata")
    )));
    assert!(
        requests
            .iter()
            .filter(|r| r["method"] == "pane.report_metadata")
            .all(|r| r["params"]["pane_id"] == "created-own"
                && r["params"]["source"] == "jtblink.herdr-work")
    );
}
#[test]
fn matching_label_or_metadata_cannot_authorize_a_different_foreground_program() {
    let (output, requests) = activation_case(true, None);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("身份无法核实"));
    assert!(requests.iter().all(|r| matches!(
        r["method"].as_str(),
        Some("pane.list" | "pane.process_info")
    )));
}

#[test]
fn stale_own_metadata_cannot_focus_the_same_binary_using_another_profile() {
    let other = tempfile::tempdir().unwrap();
    let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_herdr-work"))
        .canonicalize()
        .unwrap();
    let (output, requests) = activation_case(
        true,
        Some(json!({
            "argv0":binary,"argv":[binary,"--home",other.path(),"ui"]
        })),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("身份无法核实"));
    assert!(requests.iter().all(|r| matches!(
        r["method"].as_str(),
        Some("pane.list" | "pane.process_info")
    )));
}

#[test]
fn explicit_initial_activity_and_root_win_over_an_unrelated_saved_binding_in_private_pty() {
    use herdr_work_core::{Context, Execution, ExecutionId, TaskId};
    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join("old-workspace");
    let root = temp.path().join("new-workspace");
    std::fs::create_dir(&old).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("explicit-file.txt"), "fixture").unwrap();
    let profile = temp.path().join("profile");
    let desk = herdr_work_engine::HerdrWork::open(&profile, None).unwrap();
    desk.save_context(&Context {
        epoch: 7,
        execution: Some(Execution {
            id: ExecutionId("saved".into()),
            task: TaskId("browse".into()),
            attempt: None,
            worktree: old.clone(),
            worktree_identity: herdr_work_engine::HerdrWork::worktree_identity(&old).unwrap(),
            origin_terminal: None,
            origin_socket: None,
            target: None,
        }),
    })
    .unwrap();
    drop(desk);
    let script = r#"
import os,pty,fcntl,termios,struct,subprocess,sys,time,select,re,signal
binary,profile,root=sys.argv[1:]
env={k:v for k,v in os.environ.items() if not k.startswith('HERDR_')}
env.update(TERM='xterm-256color',HERDR_PROJECTS_NO_KEEPER='1',HERDR_PROJECTS_INLINE='1')
for view,title in [('explorer','EXPLORER'),('search','SEARCH'),('source-control','SOURCECONTROL'),('quick-open','QUICKOPEN')]:
    master,slave=pty.openpty(); fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',32,120,0,0))
    def seat():
        os.setsid(); fcntl.ioctl(0,termios.TIOCSCTTY,0)
    wrapper='import subprocess,sys,time; r=subprocess.run(sys.argv[1:]); time.sleep(1); sys.exit(r.returncode)'
    env.update(HERDR_WORK_INITIAL_VIEW=view,HERDR_WORK_ROOT=root)
    initial=[] if view in ('search','quick-open') else ['--initial-view',view,root]
    process=subprocess.Popen([sys.executable,'-c',wrapper,binary,'--home',profile,'ui',*initial],stdin=slave,stdout=slave,stderr=slave,env=env,preexec_fn=seat)
    captured=b''
    try:
        deadline=time.monotonic()+8
        while time.monotonic()<deadline:
            if select.select([master],[],[],.05)[0]:
                captured+=os.read(master,65536)
            text=''.join(re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]',b'',captured).decode(errors='replace').split())
            if title in text and 'new-workspace' in text: break
        assert title in text and 'new-workspace' in text,(view,text[-1600:])
        assert 'old-workspace' not in text,(view,text[-1600:])
        # Allow the first worker results to settle before dismissing the search prompt.
        deadline=time.monotonic()+.4
        while time.monotonic()<deadline:
            if select.select([master],[],[],.05)[0]: os.read(master,65536)
        os.write(master,b'\x1b'); time.sleep(.15); os.write(master,b'q')
        deadline=time.monotonic()+4
        while process.poll() is None and time.monotonic()<deadline:
            if select.select([master],[],[],.05)[0]: os.read(master,65536)
        assert process.wait(timeout=5)==0
    finally:
        if process.poll() is None: os.killpg(process.pid,signal.SIGTERM); process.wait(timeout=5)
        os.close(master); os.close(slave)
"#;
    let output = Command::new("python3")
        .args([
            "-c",
            script,
            env!("CARGO_BIN_EXE_herdr-work"),
            profile.to_str().unwrap(),
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
