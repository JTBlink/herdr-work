#![cfg(unix)]
use std::{path::Path, process::Command};
fn files(root: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    fn visit(root: &Path, dir: &Path, out: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.push((
                    path.strip_prefix(root).unwrap().into(),
                    std::fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut out = vec![];
    visit(root, root, &mut out);
    out.sort();
    out
}
#[test]
fn direct_worker_ignores_third_party_profiles_and_never_takes_over_plugins() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let plugins = home.join("third-party");
    for name in [
        "sidebar/state.json",
        "projects/data/task.json",
        "projects/settings/config.json",
        "projects/reports/report.json",
        "review-tool/drafts.json",
        "herdr/plugins.json",
        "bin/herdr-sidebar",
        "bin/herdr-projects",
    ] {
        let path = plugins.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("original {name}")).unwrap();
    }
    let before = files(&plugins);
    let root = temp.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    for explicit in [false, true] {
        let expected = if explicit {
            temp.path().join("explicit-profile")
        } else {
            home.join(".herdr-work")
        };
        let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-work-projects"));
        command
            .current_dir(&root)
            .env("HOME", &home)
            .env_remove("HERDR_WORK_HOME")
            .env("HERDR_PROJECTS_HOME", plugins.join("projects"))
            .env("HERDR_PROJECTS_DATA_DIR", plugins.join("projects/data"))
            .env(
                "HERDR_PROJECTS_CONFIG_DIR",
                plugins.join("projects/settings"),
            )
            .env(
                "HERDR_PROJECTS_REPORTS_DIR",
                plugins.join("projects/reports"),
            )
            .env("WORKDESK_HOME", plugins.join("sidebar"))
            .env("HERDR_PLUGIN_ID", "third.party.sidebar")
            .env(
                "HERDR_PLUGIN_CONTEXT_JSON",
                format!("{{\"focused_pane_cwd\":{:?}}}", plugins),
            )
            .env("HERDR_PROJECTS_INLINE", "1")
            .env("HERDR_PROJECTS_NO_KEEPER", "1");
        if explicit {
            command.env("HERDR_WORK_HOME", &expected);
        }
        let output = command.arg("herdr-work-environment").output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let observed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            observed["home"],
            expected.join("projects").to_string_lossy().as_ref()
        );
        assert_eq!(observed["managed"], true);
        assert_eq!(observed["plugin"], "");
        // Exercise ordinary startup, not only the integration-only early-return command.
        let mut ordinary = Command::new(env!("CARGO_BIN_EXE_herdr-work-projects"));
        ordinary.current_dir(&root);
        for (key, value) in command.get_envs() {
            if let Some(value) = value {
                ordinary.env(key, value);
            } else {
                ordinary.env_remove(key);
            }
        }
        let output = ordinary
            .args(["add", "isolated direct worker", "--json"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(expected.join("projects/data/tasks").is_dir());
        assert_eq!(
            files(&plugins),
            before,
            "A standalone third-party plugin was changed"
        );
    }
}
