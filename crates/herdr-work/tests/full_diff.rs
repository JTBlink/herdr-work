#[cfg(unix)]
#[test]
fn actual_viewer_shows_full_diff_and_sidebar_folds_from_the_top() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python3")
        .arg(root.join("scripts/tests/full_diff_fixture.py"))
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
