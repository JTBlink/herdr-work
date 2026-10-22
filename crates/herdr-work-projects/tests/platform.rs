//! Unsupported platforms fail before creating a task profile or starting a backend.
#![cfg(not(unix))]

#[test]
fn unsupported_worker_reports_unix_requirement_without_creating_a_profile() {
    let temp = tempfile::tempdir().unwrap();
    let profile = temp.path().join("profile");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_herdr-work-projects"))
        .args(["add", "unsupported task", "--json"])
        .env("HERDR_WORK_HOME", &profile)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires Unix"));
    assert!(!profile.exists());
}
