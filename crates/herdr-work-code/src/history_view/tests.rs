use super::*;
use crate::history::tests::Repo;
use crossterm::event::KeyModifiers;
use ratatui::{Terminal, backend::TestBackend};

fn wait(view: &mut GraphView) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while view.browser.busy() {
        view.tick();
        assert!(
            std::time::Instant::now() < deadline,
            "history query stalled"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
fn click(x: u16, y: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn graph_single_click_shows_full_details_and_historical_file_target_at_all_sizes() {
    let repo = Repo::new();
    repo.write("example.rs", "fn main() {}\n");
    repo.run(&["add", "."]);
    repo.run(&[
        "commit",
        "-qm",
        "feat(ui): 展示提交详情\n\n完整说明正文，点击文件查看本次改动。",
    ]);
    let oid = repo.run(&["rev-parse", "HEAD"]);
    let mut view = GraphView::new(repo.0.path().into());
    view.refresh();
    wait(&mut view);
    let mut terminal = Terminal::new(TestBackend::new(140, 32)).unwrap();
    terminal
        .draw(|frame| view.draw(frame, frame.area()))
        .unwrap();
    assert!(view.detail.is_none());
    view.on_mouse(click(12, view.graph_area.y));
    wait(&mut view);
    terminal
        .draw(|frame| view.draw(frame, frame.area()))
        .unwrap();
    assert_eq!(view.details().unwrap().oid, oid);
    assert!(
        view.detail
            .as_ref()
            .unwrap()
            .plain_text()
            .contains("完整说明正文")
    );
    let doc = view.detail.as_ref().unwrap();
    let source = doc.commit_details().unwrap().lines().1;
    let row = doc.rows.iter().position(|row| row.src == source).unwrap();
    let target = view
        .on_mouse(click(5, view.detail_area.y + row as u16))
        .unwrap();
    assert_eq!(target.oid, oid);
    assert_eq!(target.path, "example.rs");
    assert_eq!(view.scroll, 0);
    if let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES") {
        let buffer = terminal.backend().buffer();
        let cells = buffer.content.iter().map(|c| serde_json::json!({"text":c.symbol(),"fg":format!("{:?}",c.fg),"bg":format!("{:?}",c.bg)})).collect::<Vec<_>>();
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(
            std::path::Path::new(&output).join("git-graph-details.json"),
            serde_json::to_vec(
                &serde_json::json!({"width":140,"height":32,"cells":cells,"fixture":true}),
            )
            .unwrap(),
        )
        .unwrap();
    }
    for (width, height) in [(80, 24), (38, 10), (12, 5)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| view.draw(frame, frame.area()))
            .unwrap();
        assert!(view.close_detail.width > 0);
        view.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        terminal
            .draw(|frame| view.draw(frame, frame.area()))
            .unwrap();
        assert!(!view.show_detail);
        view.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }
}

#[test]
fn graph_scope_removes_hidden_selection_and_rapid_commit_selection_keeps_latest_oid() {
    let repo = Repo::new();
    repo.run(&["commit", "--allow-empty", "-qm", "initial"]);
    let initial = repo.run(&["rev-parse", "HEAD"]);
    repo.run(&["checkout", "-qb", "feature"]);
    repo.run(&["commit", "--allow-empty", "-qm", "feature"]);
    let feature = repo.run(&["rev-parse", "HEAD"]);
    repo.run(&["checkout", "-q", "main"]);
    let mut view = GraphView::new(repo.0.path().into());
    view.refresh();
    wait(&mut view);
    let feature_row = view
        .browser
        .graph
        .rows
        .iter()
        .position(|r| r.commit.as_ref().is_some_and(|c| c.oid == feature))
        .unwrap();
    view.select(feature_row);
    view.browser.select(initial.clone(), 0);
    wait(&mut view);
    assert_eq!(view.details().unwrap().oid, initial);
    view.select(feature_row);
    wait(&mut view);
    view.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    wait(&mut view);
    assert!(view.selected.is_none());
    assert!(!view.show_detail);
}
