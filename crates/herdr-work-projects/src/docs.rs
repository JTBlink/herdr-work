//! `herdr-projects docs`: the pages made from the code, and the repo's own rules checked. It writes what is out of
//! date (`--check` only says which), prints the map (`--where [folder]`), or lists what breaks a rule (`--guard`).
//!   docs/keys.md              the table of keys, from crates/flow/src/board/keys.rs
//!   docs/settings.md          the table of settings, from crates/core/src/store/settings_list.rs
//!   docs/agents.md            the table of agent programs, from crates/core/src/agents/kinds/
//!   docs/where-things-live.md the map: which file holds what, from the comment at the top of each file. Not in git
//!                             (it changed in most pull requests and collided): written in your own folder only.
//! A page made in part keeps its own words outside the two marker lines; only what is between them is replaced.
pub mod agents_table;
pub mod guard;
pub mod map;
pub mod tables;

use std::path::{Path, PathBuf};

/// Source-test workspace only; this module is excluded from the product build.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
/// Resolve the source suite's original relative names to their one authoritative product copy.
pub fn source_path(file: &str) -> PathBuf {
    for (old, new) in [
        ("crates/core/", "../herdr-work-task-data/"),
        ("crates/flow/", "../herdr-work-task-flow/"),
        ("crates/board/", "../herdr-work-task-board/"),
        ("crates/web/", "../herdr-work-task-web/"),
        ("tests/", "tests/source/"),
        ("scripts/", "tests/fixtures/projects/scripts/"),
        ("bin/", "tests/fixtures/projects/bin/"),
        (".github/", "../../docs/provenance/projects/github/"),
        ("docs/", "../../docs/provenance/projects/docs/"),
    ] {
        if file == old.trim_end_matches('/') {
            return root().join(new);
        }
        if let Some(rest) = file.strip_prefix(old) {
            return root().join(new).join(rest);
        }
    }
    root().join(file)
}
pub fn read(file: &str) -> String {
    std::fs::read_to_string(source_path(file)).unwrap_or_default()
}
pub fn write_file(file: &str, text: &str) -> bool {
    std::fs::write(source_path(file), text).is_ok()
}
pub fn exists(file: &str) -> bool {
    Path::new(&source_path(file)).exists()
}

pub fn docs_main(a: &[String]) -> i32 {
    let has = |f: &str| a.iter().any(|x| x == f);
    if let Some(at) = a
        .iter()
        .position(|x| x == "--where" || x.starts_with("--where="))
    {
        let under = a[at]
            .strip_prefix("--where=")
            .map(String::from)
            .or_else(|| a.get(at + 1).cloned())
            .unwrap_or_default();
        match map::where_page(&under) {
            Ok(p) => {
                print!("{p}");
                return 0;
            }
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if has("--guard") {
        let bad = guard::problems();
        for b in &bad {
            eprintln!("✖ {b}");
        }
        if !bad.is_empty() {
            return 1;
        }
        println!(
            "Guard: every source file is within its size, says what it holds, and every made table is up to date."
        );
        return 0;
    }
    if has("--check") {
        let old = tables::stale();
        if !old.is_empty() {
            eprintln!("Out of date: {}. Run: herdr-projects docs", old.join(", "));
            return 1;
        }
        println!("The made pages are up to date.");
        return 0;
    }
    match tables::write() {
        Ok(old) => {
            println!(
                "{}",
                if old.is_empty() {
                    "Nothing to write: the made pages are up to date.".to_string()
                } else {
                    format!("Wrote {}", old.join(", "))
                }
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
