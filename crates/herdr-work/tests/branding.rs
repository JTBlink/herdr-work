use serde_json::Value;
use std::{path::Path, process::Command};

fn doctor(home: &Path, args: &[&str], variables: &[(&str, &Path)]) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-work"));
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("HERDR_WORK_HOME")
        .env_remove("WORKDESK_HOME");
    for (key, path) in variables {
        command.env(key, path);
    }
    let output = command.args(args).arg("doctor").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn doctor_reports_new_product_profile_and_backend_names() {
    let temp = tempfile::tempdir().unwrap();
    let data = doctor(temp.path(), &[], &[]);
    assert_eq!(data["product"], "HerdrWork");
    assert_eq!(
        data["profile"],
        temp.path()
            .canonicalize()
            .unwrap()
            .join(".herdr-work")
            .to_str()
            .unwrap()
    );
    assert!(
        data["projects_backend"]
            .as_str()
            .unwrap()
            .contains("herdr-work-projects")
    );
}
#[test]
fn existing_legacy_profile_is_preserved_and_copied_to_the_new_default() {
    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join(".workdesk");
    std::fs::create_dir(&old).unwrap();
    std::fs::write(old.join("config.json"), b"{\"keep\":42}").unwrap();
    doctor(temp.path(), &[], &[]);
    for root in [&old, &temp.path().join(".herdr-work")] {
        assert_eq!(
            std::fs::read(root.join("config.json")).unwrap(),
            b"{\"keep\":42}"
        );
    }
}
#[test]
fn explicit_profile_then_new_environment_then_legacy_environment_take_precedence() {
    let temp = tempfile::tempdir().unwrap();
    let new = temp.path().join("new-env");
    let old = temp.path().join("old-env");
    let explicit = temp.path().join("explicit");
    let values = [
        ("HERDR_WORK_HOME", new.as_path()),
        ("WORKDESK_HOME", old.as_path()),
    ];
    let data = doctor(temp.path(), &[], &values);
    assert_eq!(
        Path::new(data["profile"].as_str().unwrap()),
        new.canonicalize().unwrap()
    );
    let data = doctor(
        temp.path(),
        &["--home", explicit.to_str().unwrap()],
        &values,
    );
    assert_eq!(
        Path::new(data["profile"].as_str().unwrap()),
        explicit.canonicalize().unwrap()
    );
    let data = doctor(temp.path(), &[], &[("WORKDESK_HOME", old.as_path())]);
    assert_eq!(
        Path::new(data["profile"].as_str().unwrap()),
        old.canonicalize().unwrap()
    );
}

#[test]
fn hooks_can_resolve_the_profile_while_the_ui_holds_its_lock() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("profile");
    let _active = herdr_work_engine::HerdrWork::open(&root, None).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-work"))
        .args(["--home", root.to_str().unwrap(), "profile-path"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let data: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(Path::new(data["profile"].as_str().unwrap()), root);
}
