use super::*;
use crate::history::tests::Repo as Fixture;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn compact_graph_expands_commit_files_and_keeps_full_oid_when_subject_looks_like_hash() {
    let fixture = Fixture::new();
    fixture.write("example.rs", "fn main() {}\n");
    fixture.run(&["add", "."]);
    fixture.run(&["commit", "-qm", "deadbeef 展开提交\n\n完整正文"]);
    let oid = fixture.run(&["rev-parse", "HEAD"]);
    let mut app = App::new(fixture.0.path().into(), Default::default());
    app.drawers[Drawer::Graph.index()].expanded = true;
    app.reload_expanded_drawers();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.history.loading() {
        app.poll_history();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.rebuild();
    let row = app
        .rows
        .iter()
        .position(|row| matches!(row, Row::DrawerLine(Drawer::Graph, 0)))
        .unwrap();
    app.selected = Some(row);
    app.activate();
    while app.history.busy() {
        app.poll_history();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(app.history.details().unwrap().oid, oid);
    let file_row = app
        .rows
        .iter()
        .position(|row| matches!(row, Row::HistoryFile(0)))
        .unwrap();
    assert!(app.doc_key_for_row(file_row).unwrap().contains(&oid));
    app.selected = Some(file_row);
    app.fold_selected(false);
    assert_eq!(app.selected, Some(row));
    app.fold_selected(false);
    assert!(!app.history_expanded);
    assert!(
        !app.rows
            .iter()
            .any(|row| matches!(row, Row::HistoryFile(_)))
    );
    app.fold_selected(true);
    assert!(app.history_expanded);
    app.flash = Default::default();
    app.theme = IconTheme::Emoji;
    app.repos[app.active].name = "demo".into();
    app.last_mouse = Some(std::time::Instant::now());
    let mut terminal = Terminal::new(TestBackend::new(45, 32)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let graph_button = app
        .title_zones
        .iter()
        .find(|(_, action)| *action == TitleAction::OpenGraph)
        .unwrap()
        .0;
    app.mouse_pos = Some((graph_button.x, graph_button.y));
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let screen = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(screen.contains("Graph"));
    assert!(!screen.contains("Commits"));
    assert!(screen.contains("example.rs"));
    if let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES") {
        let buffer = terminal.backend().buffer();
        let cells = buffer.content.iter().map(|c| serde_json::json!({"text":c.symbol(),"fg":format!("{:?}",c.fg),"bg":format!("{:?}",c.bg)})).collect::<Vec<_>>();
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(
            Path::new(&output).join("git-compact-history.json"),
            serde_json::to_vec(
                &serde_json::json!({"width":45,"height":32,"cells":cells,"fixture":true}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}
