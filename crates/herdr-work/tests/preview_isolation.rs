//! The actual mouse handler and preview protocol run in a private PTY/host.
#![cfg(unix)]

#[test]
fn file_clicks_honor_preview_placement_and_leave_independent_sidebar_untouched() {
    let output = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/tests/preview_fixture.py"
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
