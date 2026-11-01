//! Actual sidebar deletion input runs only in an isolated fixture directory.
#![cfg(unix)]

#[test]
fn delete_confirmation_supports_mouse_and_keyboard_without_background_input() {
    let output = std::process::Command::new("python3")
        .args([
            "-c",
            "import subprocess,sys; subprocess.run(sys.argv[1:], check=True, timeout=90)",
            "python3",
            "-u",
        ])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/tests/delete_confirmation_fixture.py"
        ))
        .arg(env!("CARGO_BIN_EXE_herdr-work"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
