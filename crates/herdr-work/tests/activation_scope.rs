//! Activation follows the invoking tab and its live project, even from a plugin cwd.
#![cfg(unix)]
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[test]
fn auto_open_ignores_preview_and_parking_tab_events_before_contacting_host() {
    let temp = tempfile::tempdir().unwrap();
    let profile = temp.path().join("profile");
    std::fs::create_dir_all(profile.join("sidebar")).unwrap();
    std::fs::write(profile.join("sidebar/state.json"), r#"{"auto_open":true}"#).unwrap();
    for data in [
        json!({"tab":{"tab_id":"w1:t2","label":"example.md · preview"}}),
        json!({"tab_id":"w1:t2","label":"example.md · preview"}),
        json!({"tab":{"tab_id":"w1:t2","label":"Working panes (temporary preview)"}}),
        json!({"tab_id":"w1:t2","label":"Working panes (temporary preview)"}),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_herdr-work"))
            .args(["--home", profile.to_str().unwrap(), "sidebar-ensure"])
            .env("HERDR_SOCKET_PATH", temp.path().join("nonexistent.sock"))
            .env(
                "HERDR_PLUGIN_EVENT_JSON",
                json!({"event":"tab.created","data":data}).to_string(),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["action"],
            "preview-tab"
        );
    }
}

fn open_from_plugin(context: Value, caller: &str, explicit_root: bool, quiet: bool) {
    let temp = tempfile::tempdir_in("/tmp").unwrap();
    let project = temp.path().join("current-project");
    let plugin = temp.path().join("plugin-directory");
    std::fs::create_dir(&project).unwrap();
    std::fs::create_dir(&plugin).unwrap();
    let project = project.canonicalize().unwrap();
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    let profile = profile.canonicalize().unwrap();
    if quiet {
        std::fs::create_dir(profile.join("sidebar")).unwrap();
        std::fs::write(profile.join("sidebar/state.json"), r#"{"auto_open":true}"#).unwrap();
    }
    let token = herdr_work_engine::fingerprint(profile.to_string_lossy().as_bytes());
    let socket = temp.path().join("host.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let recorded = requests.clone();
    let cwd = project.clone();
    let host = std::thread::spawn(move || {
        while !stopping.load(Ordering::SeqCst) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            recorded.lock().unwrap().push(request.clone());
            let result = match request["method"].as_str().unwrap() {
                "pane.list" => json!({"panes":[
                    {"pane_id":"old-terminal","tab_id":"old-tab","focused":quiet,"cwd":"/","tokens":{}},
                    {"pane_id":"other-tab-sidebar","tab_id":"old-tab","tokens":{"herdr-work-profile":token}},
                    {"pane_id":"current-terminal","tab_id":"current-tab","focused":!quiet,"cwd":"/","foreground_cwd":cwd,"tokens":{}}
                ]}),
                "pane.layout" => {
                    json!({"layout":{"tab_id":"current-tab","area":{"x":0,"y":0,"width":160,"height":40},"panes":[{"pane_id":request["params"]["pane_id"],"rect":{"x":0,"y":0,"width":160,"height":40}}],"splits":[]}})
                }
                "plugin.pane.open" => json!({"pane_id":"created"}),
                _ => json!({}),
            };
            writeln!(stream, "{}", json!({"result":result})).unwrap();
        }
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-work"));
    command.args(["--home", profile.to_str().unwrap()]);
    if quiet {
        command.arg("sidebar-ensure");
    } else {
        command.args(["activate", "show-explorer"]);
    }
    if explicit_root {
        command.args(["--root", plugin.to_str().unwrap()]);
    }
    let output = command
        .current_dir(&plugin)
        .env("HERDR_ENV", "1")
        .env("HERDR_SOCKET_PATH", socket)
        .env("HERDR_PANE_ID", caller)
        .env("HERDR_TAB_ID", "old-tab")
        .env("HERDR_PLUGIN_CONTEXT_JSON", context.to_string())
        .env_remove("HERDR_PLUGIN_EVENT_JSON")
        .output()
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    host.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let requests = requests.lock().unwrap();
    let open = requests
        .iter()
        .find(|r| r["method"] == "plugin.pane.open")
        .unwrap();
    assert_eq!(open["params"]["target_pane_id"], "current-terminal");
    if quiet {
        assert_eq!(open["params"]["focus"], false);
        let focus = requests
            .iter()
            .rev()
            .find(|r| r["method"] == "pane.focus")
            .unwrap();
        assert_eq!(focus["params"]["pane_id"], "old-terminal");
    }

    let expected = if explicit_root {
        plugin.canonicalize().unwrap()
    } else {
        project
    };
    assert_eq!(open["params"]["cwd"], json!(expected));
    assert_eq!(open["params"]["env"]["HERDR_WORK_ROOT"], json!(expected));
}

#[test]
fn action_uses_current_panes_project_instead_of_plugin_process_directory() {
    open_from_plugin(
        json!({"focused_pane_id":"current-terminal","tab_id":"current-tab"}),
        "current-terminal",
        false,
        false,
    );
}

#[test]
fn action_tab_context_overrides_an_inherited_pane_from_another_tab() {
    open_from_plugin(
        json!({"tab_id":"current-tab"}),
        "old-terminal",
        false,
        false,
    );
}

#[test]
fn explicit_project_still_overrides_current_pane_directory() {
    open_from_plugin(
        json!({"focused_pane_id":"current-terminal","tab_id":"current-tab"}),
        "current-terminal",
        true,
        false,
    );
}

#[test]
fn background_auto_open_restores_focus_in_the_original_tab_after_split_and_swap() {
    open_from_plugin(
        json!({"tab_id":"current-tab", "focused_pane_id":"current-terminal"}),
        "current-terminal",
        false,
        true,
    );
}
