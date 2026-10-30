//! Diff toolbar: painting and mouse input share the same clipped rectangles.
use super::*;
use crossterm::event::MouseEvent;
use ratatui::layout::Position;

pub(super) fn area(header: Rect, doc: &Doc) -> Rect {
    let end = find::Find::entry(header).x;
    let available = end.saturating_sub(header.x.saturating_add(3));
    let width = if doc.changes.is_some() && available >= 6 {
        available.min(18)
    } else {
        0
    };
    Rect::new(
        end.saturating_sub(width),
        header.y,
        width,
        header.height.min(1),
    )
}

fn controls(area: Rect) -> [Rect; 3] {
    let count = area.width.saturating_sub(6);
    [
        Rect::new(area.x, area.y, count, area.height),
        Rect::new(area.x + count, area.y, area.width.min(3), area.height),
        Rect::new(
            area.x + count + 3,
            area.y,
            area.width.saturating_sub(3).min(3),
            area.height,
        ),
    ]
}

pub(super) fn draw(frame: &mut Frame, doc: &Doc, header: Rect) {
    let area = area(header, doc);
    if area.width == 0 {
        return;
    }
    let Some((active, total)) = doc.change_count() else {
        return;
    };
    let [count, previous, next] = controls(area);
    let style = crate::ui::chrome_button_style(false);
    let style = if total == 0 { style.dim() } else { style };
    frame.render_widget(
        Paragraph::new(format!(" {active}/{total} 处")).style(Style::default().dim()),
        count,
    );
    frame.render_widget(Paragraph::new(" ↑ ").style(style), previous);
    frame.render_widget(Paragraph::new(" ↓ ").style(style), next);
}

pub(super) fn mouse(doc: &mut Doc, mouse: MouseEvent, header: Rect) -> bool {
    if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
        return false;
    }
    let area = area(header, doc);
    if area.width == 0 {
        return false;
    }
    let at = Position::new(mouse.column, mouse.row);
    let [_, previous, next] = controls(area);
    if previous.contains(at) || next.contains(at) {
        doc.step_change(previous.contains(at));
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::{ChangeNavigation, apply_diff_refresh};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    fn doc(patch: &str) -> Doc {
        let (lines, starts) = crate::diffview::render_with_changes("example.rs", patch);
        let mut doc = crate::preview::git_show("example.rs".into(), "working tree diff".into(), "");
        doc.lines = lines;
        doc.changes = Some(ChangeNavigation::new(starts));
        doc
    }

    const PATCH: &str = "@@ -1,6 +1,6 @@\n context\n-old\n+new\n between\n-last\n+updated\n end\n";

    fn render(doc: &mut Doc, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                super::super::draw_doc(
                    frame,
                    doc,
                    IconTheme::Emoji,
                    false,
                    None,
                    &mut find::Find::default(),
                );
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn change_blocks_cycle_independently_even_in_one_clamped_viewport() {
        let mut doc = doc(PATCH);
        render(&mut doc, 80, 20);
        assert_eq!(doc.change_count(), Some((0, 2)));
        for expected in [1, 2, 1] {
            doc.step_change(false);
            render(&mut doc, 80, 20);
            assert_eq!(doc.change_count(), Some((expected, 2)));
            assert_eq!(doc.scroll, 0, "short document is fully visible");
        }
        doc.step_change(true);
        render(&mut doc, 80, 20);
        assert_eq!(doc.change_count(), Some((2, 2)));
    }

    #[test]
    fn jumps_map_source_blocks_through_wrapping_and_resize() {
        let patch = format!(
            "@@ -1,3 +1,3 @@\n {}\n-old\n+new\n",
            "long context ".repeat(40)
        );
        let mut doc = doc(&patch);
        render(&mut doc, 32, 4);
        doc.step_change(false);
        render(&mut doc, 32, 4);
        assert!(doc.scroll > 1, "wrapped context occupies multiple rows");
        assert_eq!(doc.rows[doc.scroll].src, 1);
        doc.toggle_wrap();
        render(&mut doc, 32, 4);
        assert_eq!(doc.change_count(), Some((1, 1)));
        doc.toggle_wrap();
        render(&mut doc, 18, 4);
        doc.step_change(true);
        render(&mut doc, 18, 4);
        assert_eq!(doc.rows[doc.scroll].src, 1);
    }

    #[test]
    fn scrolling_reanchors_navigation_and_refresh_discards_stale_blocks() {
        let mut doc = doc(PATCH);
        render(&mut doc, 80, 3);
        doc.step_change(false);
        render(&mut doc, 80, 3);
        doc.set_scroll(4);
        doc.step_change(false);
        render(&mut doc, 80, 3);
        assert_eq!(doc.change_count(), Some((2, 2)));
        apply_diff_refresh(&mut doc, self::doc(PATCH));
        assert_eq!(
            doc.change_count(),
            Some((2, 2)),
            "unchanged refresh preserves cursor"
        );
        apply_diff_refresh(&mut doc, self::doc("(no changes)"));
        doc.step_change(false);
        render(&mut doc, 80, 3);
        assert_eq!(doc.change_count(), Some((0, 0)));
    }

    #[test]
    fn toolbar_clicks_match_painted_arrows_without_overlapping_close_or_find() {
        let mut doc = doc(PATCH);
        for width in [1, 3, 8, 12, 20, 32, 44, 45, 80, 120] {
            let buffer = render(&mut doc, width, 8);
            let header = Rect::new(0, 0, width, 1);
            let area = area(header, &doc);
            if area.width == 0 {
                continue;
            }
            let [_, previous, next] = controls(area);
            assert!(previous.x >= 3);
            assert!(next.right() <= find::Find::entry(header).x);
            assert_eq!(buffer[(previous.x + 1, 0)].symbol(), "↑");
            assert_eq!(buffer[(next.x + 1, 0)].symbol(), "↓");
            let click = |x| MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: 0,
                modifiers: KeyModifiers::NONE,
            };
            doc.changes.as_mut().unwrap().active = None;
            assert!(mouse(&mut doc, click(next.x + 1), header));
            assert_eq!(doc.change_count(), Some((1, 2)));
            assert!(mouse(&mut doc, click(previous.x + 1), header));
            assert_eq!(doc.change_count(), Some((2, 2)));
            assert!(!mouse(&mut doc, click(0), header));
            assert!(!mouse(&mut doc, click(find::Find::entry(header).x), header));
        }
        let mut file = crate::preview::git_show("example.rs".into(), "file".into(), "plain");
        assert_eq!(area(Rect::new(0, 0, 80, 1), &file).width, 0);
        file.step_change(false);
        assert_eq!(file.change_count(), None);
    }

    #[test]
    fn real_git_diffs_expose_blocks_for_worktree_index_and_untracked_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        git(&["config", "user.name", "Diff Fixture"]);
        git(&["config", "user.email", "fixture@example.invalid"]);
        std::fs::write(root.join("example.rs"), "old\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "fixture"]);
        std::fs::write(root.join("example.rs"), "new\n").unwrap();
        let worktree = crate::preview::load_diff(root, "example.rs", "worktree");
        assert_eq!(worktree.change_count(), Some((0, 1)));
        git(&["add", "."]);
        let staged = crate::preview::load_diff(root, "example.rs", "staged");
        assert_eq!(staged.change_count(), Some((0, 1)));
        assert_eq!(
            crate::preview::load_diff(root, "example.rs", "worktree").change_count(),
            Some((0, 0))
        );
        std::fs::write(root.join("new.rs"), "added\n").unwrap();
        assert_eq!(
            crate::preview::load_diff(root, "new.rs", "untracked").change_count(),
            Some((0, 1))
        );
        std::fs::write(root.join("image.bin"), b"\0binary").unwrap();
        assert_eq!(
            crate::preview::load_diff(root, "image.bin", "untracked").change_count(),
            Some((0, 0))
        );
    }

    #[test]
    fn navigation_capture() {
        let mut doc = doc(PATCH);
        doc.step_change(false);
        let buffer = render(&mut doc, 100, 12);
        let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(text.contains("1/2"));
        assert!(text.contains("F7"));
        assert_eq!(
            buffer
                .content
                .iter()
                .filter(|cell| cell.symbol() == "↑")
                .count(),
            1
        );
        assert_eq!(
            buffer
                .content
                .iter()
                .filter(|cell| cell.symbol() == "↓")
                .count(),
            1
        );
        if let Ok(path) = std::env::var("HERDR_WORK_DIFF_CAPTURE") {
            let cells: Vec<_> = buffer.content.iter().map(|cell| serde_json::json!({
                "text": cell.symbol(), "fg": format!("{:?}", cell.fg), "bg": format!("{:?}", cell.bg)
            })).collect();
            std::fs::write(
                path,
                serde_json::to_vec(&serde_json::json!({"width":100,"height":12,"cells":cells}))
                    .unwrap(),
            )
            .unwrap();
        }
    }
}
