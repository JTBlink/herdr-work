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
fn obsolete_environment_and_directory_are_ignored() {
    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join(".workdesk");
    std::fs::create_dir(&old).unwrap();
    std::fs::write(old.join("config.json"), b"do not import").unwrap();
    let obsolete_env = temp.path().join("obsolete-env");
    let data = doctor(
        temp.path(),
        &[],
        &[("WORKDESK_HOME", obsolete_env.as_path())],
    );
    assert_eq!(
        Path::new(data["profile"].as_str().unwrap()),
        temp.path().canonicalize().unwrap().join(".herdr-work")
    );
    assert!(!obsolete_env.exists());
    assert!(!temp.path().join(".herdr-work/config.json").exists());
    assert!(
        !temp
            .path()
            .join(".herdr-work/profile-migration.json")
            .exists()
    );
    assert_eq!(
        std::fs::read(old.join("config.json")).unwrap(),
        b"do not import"
    );
}
#[test]
fn explicit_profile_overrides_environment_and_environment_overrides_default() {
    let temp = tempfile::tempdir().unwrap();
    let configured = temp.path().join("configured");
    let explicit = temp.path().join("explicit");
    let values = [("HERDR_WORK_HOME", configured.as_path())];
    let data = doctor(temp.path(), &[], &values);
    assert_eq!(
        Path::new(data["profile"].as_str().unwrap()),
        configured.canonicalize().unwrap()
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
