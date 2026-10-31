use super::*;
use ratatui::{Terminal, backend::TestBackend};

struct Fixture {
    temp: tempfile::TempDir,
}
impl Fixture {
    fn new(text: &str) -> Self {
        let this = Self {
            temp: tempfile::tempdir().unwrap(),
        };
        this.git(&["init", "-q"]);
        this.git(&["config", "user.name", "Diff Fixture"]);
        this.git(&["config", "user.email", "fixture@example.invalid"]);
        this.git(&["config", "core.autocrlf", "false"]);
        this.git(&["config", "color.ui", "always"]);
        this.git(&["config", "diff.context", "1"]);
        this.write(text);
        this.git(&["add", "."]);
        this.git(&["commit", "-qm", "fixture"]);
        this
    }
    fn root(&self) -> &Path {
        self.temp.path()
    }
    fn write(&self, text: &str) {
        std::fs::write(self.root().join("demo [1].rs"), text).unwrap();
    }
    fn git(&self, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(self.root())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fn load(&self, kind: &str) -> Doc {
        load_diff(self.root(), "demo [1].rs", kind)
    }
}
fn source() -> String {
    (1..=240)
        .map(|no| format!("let unchanged_{no} = {no};\n"))
        .collect()
}
fn text(doc: &Doc) -> String {
    doc.lines
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn real_git_keeps_every_unchanged_line_and_staged_worktree_boundaries() {
    let original = source();
    let f = Fixture::new(&original);
    let staged = original
        .replace("unchanged_30 = 30", "staged_30 = 300")
        .replace("unchanged_190 = 190", "staged_190 = 1900");
    f.write(&staged);
    let mut worktree = f.load("worktree");
    assert_eq!(worktree.change_count(), Some((0, 2)));
    for no in (1..=240).filter(|no| ![30, 190].contains(no)) {
        assert!(
            text(&worktree).contains(&format!("let unchanged_{no} = {no};")),
            "lost unchanged line {no}"
        );
    }
    assert!(!text(&worktree).contains('⋯'));
    assert!(worktree.split.is_some());
    f.git(&["add", "."]);
    f.write(&staged.replace("unchanged_100 = 100", "worktree_only = 1000"));
    let index = f.load("staged");
    assert_eq!(index.change_count(), Some((0, 2)));
    assert!(!text(&index).contains("worktree_only"));
    assert!(text(&index).contains("unchanged_100 = 100"));
    let newer = f.load("worktree");
    assert_eq!(newer.change_count(), Some((0, 1)));
    assert!(text(&newer).contains("staged_30 = 300"));
    // A full document starts at line 1; arrows still jump straight to changes.
    worktree.relayout(120, 16);
    assert!(
        worktree.rows[0]
            .line
            .to_string()
            .contains("unchanged_1 = 1")
    );
    worktree.step_change(false);
    worktree.relayout(120, 16);
    assert!(
        worktree.rows[worktree.scroll]
            .line
            .to_string()
            .contains("staged_30")
    );
    worktree.step_change(false);
    worktree.relayout(120, 16);
    assert!(
        worktree.rows[worktree.scroll]
            .line
            .to_string()
            .contains("staged_190")
    );
}

#[test]
fn no_text_change_still_shows_index_content_and_binary_and_empty_files_are_explicit() {
    let f = Fixture::new("same\nbody\n");
    f.write("unstaged\nbody\n");
    let index = f.load("staged");
    assert_eq!(index.change_count(), Some((0, 0)));
    assert!(text(&index).contains("same"));
    assert!(!text(&index).contains("unstaged"));
    f.git(&["add", "."]);
    let clean = f.load("worktree");
    assert!(text(&clean).contains("unstaged"));
    assert_eq!(clean.change_count(), Some((0, 0)));
    std::fs::write(f.root().join("new.rs"), "whole new file\n").unwrap();
    assert_eq!(
        load_diff(f.root(), "new.rs", "untracked").change_count(),
        Some((0, 1))
    );
    std::fs::write(f.root().join("new.bin"), b"a\0b").unwrap();
    assert_eq!(
        load_diff(f.root(), "new.bin", "untracked").change_count(),
        Some((0, 0))
    );
    std::fs::write(f.root().join("empty.rs"), "").unwrap();
    assert!(text(&load_diff(f.root(), "empty.rs", "untracked")).contains("empty file"));
    f.git(&["rm", "-f", "--", "demo [1].rs"]);
    let deleted = f.load("staged");
    assert!(text(&deleted).contains("same"));
    assert_eq!(deleted.change_count(), Some((0, 1)));
}

#[test]
fn large_full_diff_fails_explicitly_instead_of_silently_hiding_context() {
    let f = Fixture::new(&"line\n".repeat(MAX_DIFF_LINES + 10));
    f.write(&format!("new\n{}", "line\n".repeat(MAX_DIFF_LINES + 9)));
    let doc = f.load("worktree");
    assert!(text(&doc).contains("无法完整显示 Diff"));
    assert!(doc.split.is_none());
    assert_eq!(doc.change_count(), Some((0, 0)));
}

#[test]
fn split_alignment_wrap_resize_and_overview_click_follow_the_same_changes() {
    let f = Fixture::new(&source());
    f.write(
        &source()
            .replace(
                "let unchanged_30 = 30;",
                &format!(
                    "let changed_30 = \"{}\";\nadded_line();",
                    "中文 long value ".repeat(12)
                ),
            )
            .replace("let unchanged_190 = 190;\n", ""),
    );
    let mut doc = f.load("worktree");
    doc.relayout(120, 20);
    let change = doc
        .rows
        .iter()
        .position(|row| row.line.to_string().contains("changed_30"))
        .unwrap();
    let row = &doc.rows[change].line;
    assert!(row.to_string().contains("unchanged_30"));
    assert!(
        row.spans
            .iter()
            .any(|span| span.style.bg == Some(palette().diff_del_bg))
    );
    assert!(
        row.spans
            .iter()
            .any(|span| span.style.bg == Some(palette().diff_add_bg))
    );
    assert!(
        doc.rows
            .iter()
            .any(|row| row.line.to_string().contains('╱'))
    );
    assert!(doc.rows.iter().all(|row| row.line.width() <= 118));
    let body = Rect::new(5, 4, 120, 20);
    let marks = doc.overview(body.height);
    assert!(marks.contains(&Some(ChangeKind::Modified)));
    assert!(marks.contains(&Some(ChangeKind::Removed)));
    let bucket = marks.iter().position(Option::is_some).unwrap();
    assert!(doc.overview_mouse(
        &MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: body.right() - 1,
            row: body.y + bucket as u16,
            modifiers: KeyModifiers::NONE
        },
        body
    ));
    assert!(doc.scroll > 0);
    let source = doc.top_src();
    doc.toggle_wrap();
    doc.relayout(90, 20);
    assert_eq!(doc.top_src(), source);
    doc.toggle_wrap();
    doc.relayout(40, 20);
    assert!(
        doc.rows
            .iter()
            .any(|row| row.line.to_string().contains("unchanged_240"))
    );
    assert!(doc.rows.iter().all(|row| row.line.width() <= 38));
    assert!(!doc.overview_mouse(
        &MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE
        },
        Rect::new(0, 0, 1, 0)
    ));
}

#[test]
fn full_diff_capture() {
    if let Ok(theme) = std::env::var("HERDR_WORK_DIFF_CAPTURE_THEME") {
        crate::ui::set_color_theme(match theme.as_str() {
            "light" => crate::appearance::ColorTheme::Light,
            "terminal" => crate::appearance::ColorTheme::Terminal,
            _ => crate::appearance::ColorTheme::VsCode,
        });
    }
    let f = Fixture::new(
        &(1..=32)
            .map(|n| format!("let line_{n} = {n};\n"))
            .collect::<String>(),
    );
    f.write(
        &(1..=32)
            .map(|n| match n {
                7 => "let line_7 = compute_new();\n".into(),
                16 => "added_function();\nlet line_16 = 16;\n".into(),
                26 => String::new(),
                _ => format!("let line_{n} = {n};\n"),
            })
            .collect::<String>(),
    );
    let mut doc = f.load("worktree");
    let mut terminal = Terminal::new(TestBackend::new(120, 20)).unwrap();
    terminal
        .draw(|frame| {
            let body = doc.draw_diff_header(frame, frame.area());
            doc.draw_body(frame, body);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let all: String = buffer.content.iter().map(|c| c.symbol()).collect();
    assert!(
        all.contains("原")
            && all.contains("始")
            && all.contains("改")
            && all.contains("compute_new")
    );
    assert!(all.contains('+') && all.contains('−') && all.contains('~'));
    if let Ok(path) = std::env::var("HERDR_WORK_FULL_DIFF_CAPTURE") {
        let cells: Vec<_> = buffer.content.iter().map(|cell| serde_json::json!({"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg)})).collect();
        std::fs::write(
            path,
            serde_json::to_vec(&serde_json::json!({"width":120,"height":20,"cells":cells}))
                .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn copy_from_either_split_side_omits_the_other_side_gutters_and_wrap_padding() {
    let old = format!("old {} tail\nsecond original\n", "value ".repeat(15));
    let new = format!("new {} tail\nsecond changed\n", "value ".repeat(15));
    let f = Fixture::new(&old);
    f.write(&new);
    let mut doc = f.load("worktree");
    doc.relayout(90, 20);
    for (side, expected) in [(0, old.trim_end()), (1, new.trim_end())] {
        let parts: Vec<_> = doc
            .split
            .as_ref()
            .unwrap()
            .fragments
            .iter()
            .filter(|part| part.side == side)
            .collect();
        let first = parts.first().unwrap();
        let last = parts.last().unwrap();
        doc.selection.anchor = Some(RenderPos {
            row: first.row,
            col: first.column,
        });
        doc.selection.cursor = Some(RenderPos {
            row: last.row,
            col: last.column + last.text.chars().count(),
        });
        assert_eq!(doc.selected_text().as_deref(), Some(expected));
    }
}
