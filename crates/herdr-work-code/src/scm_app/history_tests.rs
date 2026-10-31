use super::*;
use crate::history::tests::Repo as Fixture;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn compact_graph_header_aligns_with_other_drawers() {
    let fixture = Fixture::new();
    let mut app = App::new(fixture.0.path().into(), Default::default());
    app.history_target = Some("preview.rs".into());
    app.rows = Drawer::ALL.into_iter().map(Row::DrawerHeader).collect();
    for width in [20, 28, 45] {
        for expanded in [false, true] {
            app.drawers[Drawer::Graph.index()].expanded = expanded;
            for scope in [crate::history::Scope::All, crate::history::Scope::Current] {
                app.history.scope = scope;
                let mut terminal = Terminal::new(TestBackend::new(width, 10)).unwrap();
                terminal
                    .draw(|frame| app.draw_list(frame, frame.area()))
                    .unwrap();
                let buffer = terminal.backend().buffer();
                for (row, kind) in Drawer::ALL.iter().enumerate() {
                    assert_eq!(buffer[(0, row as u16)].symbol(), " ");
                    assert!(matches!(buffer[(1, row as u16)].symbol(), "▸" | "▾"));
                    assert_eq!(buffer[(2, row as u16)].symbol(), " ");
                    assert_eq!(
                        buffer[(3, row as u16)].symbol(),
                        &kind.title()[..1],
                        "{kind:?} at width {width}"
                    );
                }
                assert_eq!(buffer[(width - 1, 0)].symbol(), "]");
            }
        }
    }
}

#[test]
fn compact_history_files_keep_status_colors_when_selected_or_hovered() {
    let fixture = Fixture::new();
    fixture.write("base.rs", "fn main() {}\n");
    fixture.run(&["add", "."]);
    fixture.run(&["commit", "-qm", "initial"]);
    let statuses = ['A', 'M', 'D', 'R', 'C'];
    let mut details = crate::history::CommitDetails::load(&fixture.git(), "HEAD", 0).unwrap();
    details.files = statuses
        .into_iter()
        .map(|status| crate::history::ChangedFile {
            path: "src/file.rs".into(),
            old_path: matches!(status, 'R' | 'C').then(|| "src/old.rs".into()),
            status,
            added: Some(1),
            removed: Some(1),
        })
        .collect();
    let mut app = App::new(fixture.0.path().into(), Default::default());
    app.history.detail = Some(Ok(details));
    app.rows = (0..statuses.len()).map(Row::HistoryFile).collect();
    app.focus = Focus::List;
    app.mouse_pos = Some((0, 4));
    for width in [20, 28, 45] {
        for selected in [None, Some(0), Some(1), Some(2), Some(3), Some(4)] {
            app.selected = selected;
            let mut terminal = Terminal::new(TestBackend::new(width, 8)).unwrap();
            terminal
                .draw(|frame| app.draw_list(frame, frame.area()))
                .unwrap();
            let buffer = terminal.backend().buffer();
            for (row, status) in statuses.iter().enumerate() {
                let color = crate::ui::status_color(*status);
                let marker = &buffer[(7, row as u16)];
                assert_eq!(marker.symbol(), status.to_string());
                assert_eq!(marker.fg, color, "status {status}, selected {selected:?}");
                assert_eq!(buffer[(9, row as u16)].fg, color, "file name {status}");
                if selected == Some(row) {
                    assert_eq!(marker.bg, palette().selection_bg);
                }
            }
        }
    }
}

#[test]
fn compact_graph_gutter_preserves_lanes_and_aligns_disclosure_and_subjects() {
    let fixture = Fixture::new();
    let mut app = App::new(fixture.0.path().into(), Default::default());
    app.history.graph.rows = ["* ", "\x1b[31m|\\  \x1b[0m", "| * "]
        .into_iter()
        .enumerate()
        .map(|(index, graph)| crate::git_graph::Row {
            graph: graph.into(),
            commit: (index != 1).then(|| crate::git_graph::Commit {
                oid: format!("{index:040x}"),
                short: format!("{index:08x}"),
                refs: vec![],
                subject: "subject".into(),
                date: String::new(),
                author: String::new(),
            }),
        })
        .collect();
    app.rows = (0..3)
        .map(|index| Row::DrawerLine(Drawer::Graph, index))
        .collect();
    for width in [20, 28, 45] {
        let mut terminal = Terminal::new(TestBackend::new(width, 6)).unwrap();
        terminal
            .draw(|frame| app.draw_list(frame, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(1, 0)].symbol(), "●");
        assert_eq!(buffer[(1, 1)].symbol(), "│");
        assert_eq!(buffer[(2, 1)].symbol(), "╲");
        assert_eq!(buffer[(1, 1)].fg, Color::Red);
        assert_eq!(buffer[(3, 2)].symbol(), "●");
        for row in [0, 2] {
            assert_eq!(buffer[(5, row)].symbol(), "▸");
            assert_eq!(buffer[(7, row)].symbol(), "s");
        }
    }
}

#[test]
fn full_graph_button_stays_visible_without_mouse_and_keeps_hit_zone_inside_header() {
    let fixture = Fixture::new();
    let mut app = App::new(fixture.0.path().into(), Default::default());
    app.repos[app.active].name = "a-very-long-repository-name".into();
    for theme in [IconTheme::Material, IconTheme::Emoji] {
        app.theme = theme;
        for width in [12, 20, 28, 45] {
            for last_mouse in [
                None,
                Some(std::time::Instant::now()),
                Some(std::time::Instant::now() - std::time::Duration::from_secs(5)),
            ] {
                app.last_mouse = last_mouse;
                let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
                terminal
                    .draw(|frame| app.draw_header(frame, frame.area()))
                    .unwrap();
                let zone = app
                    .title_zones
                    .iter()
                    .find(|(_, action)| *action == TitleAction::OpenGraph)
                    .unwrap()
                    .0;
                assert!(zone.x + zone.width <= width);
                let buffer = terminal.backend().buffer();
                let text = (zone.x..zone.x + zone.width)
                    .map(|x| buffer[(x, 0)].symbol())
                    .collect::<String>();
                assert!(text.replace(' ', "").contains("大图"));
            }
        }
    }
}

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
