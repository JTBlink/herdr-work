#![cfg(feature = "sidebar-ui")]

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use herdr_work_code::{activity::InitialView, icons::IconTheme, sidebar_component::Sidebar, state};
use ratatui::{Terminal, backend::TestBackend};

fn click(sidebar: &mut Sidebar, column: u16, row: u16) -> bool {
    sidebar.event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }))
}

fn render(sidebar: &mut Sidebar, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| sidebar.draw(frame)).unwrap();
    terminal.backend().buffer().clone()
}

fn text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

#[test]
fn collapse_keeps_a_clickable_rail_and_restores_views_without_losing_tree_state() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("demo");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/visible.txt"), "fixture\n").unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    state::configure_product_profile(&fixture.path().join("profile")).unwrap();
    for theme in [IconTheme::Emoji, IconTheme::Material] {
        for right in [false, true] {
            state::update_state(|saved| {
                saved.icons = Some(theme);
                saved.dock_right = right;
                saved.merged = true;
            });
            for initial in [InitialView::Explorer, InitialView::Search, InitialView::Git] {
                let mut sidebar = Sidebar::new(root.clone(), Some(initial));
                let first = render(&mut sidebar, 38, 24);
                if initial == InitialView::Explorer && !text(&first).contains("visible.txt") {
                    click(&mut sidebar, 4, 4);
                }
                let before = render(&mut sidebar, 38, 24);
                if initial == InitialView::Explorer {
                    assert!(text(&before).contains("visible.txt"));
                }
                assert_eq!(before[(36, 0)].symbol(), "×");
                click(&mut sidebar, 36, 23);
                sidebar.event(Event::Resize(4, 24));
                let rail = render(&mut sidebar, 4, 24);
                assert!(!text(&rail).contains("demo"));
                if theme == IconTheme::Emoji
                    && !right
                    && initial == InitialView::Explorer
                    && let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES")
                {
                    std::fs::create_dir_all(&output).unwrap();
                    for (label, buffer) in
                        [("sidebar-expanded", &before), ("sidebar-collapsed", &rail)]
                    {
                        let cells = buffer.content.iter().map(|cell| serde_json::json!({"text":cell.symbol(), "fg":format!("{:?}",cell.fg), "bg":format!("{:?}",cell.bg)})).collect::<Vec<_>>();
                        std::fs::write(std::path::Path::new(&output).join(format!("{label}.json")), serde_json::to_vec(&serde_json::json!({"width":buffer.area.width, "height":buffer.area.height, "cells":cells, "fixture":true})).unwrap()).unwrap();
                    }
                }
                sidebar.event(Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Moved,
                    column: 2,
                    row: 1,
                    modifiers: KeyModifiers::NONE,
                }));
                let hovered = text(&render(&mut sidebar, 4, 24));
                assert!(
                    !hovered.contains("文件")
                        && !hovered.contains("搜索")
                        && !hovered.contains("设置"),
                    "collapsed rail must remain icon-only on hover"
                );
                assert_eq!(rail[(1, 22)].symbol(), "»");
                assert_eq!(rail[(1, 23)].symbol(), "×");
                for (row, icon) in [
                    (1, herdr_work_code::ui::ActivityIcon::Files),
                    (4, herdr_work_code::ui::ActivityIcon::Search),
                    (7, herdr_work_code::ui::ActivityIcon::SourceControl),
                    (10, herdr_work_code::ui::ActivityIcon::Tasks),
                    (13, herdr_work_code::ui::ActivityIcon::Settings),
                ] {
                    let row_text = (0..4).map(|x| rail[(x, row)].symbol()).collect::<String>();
                    assert!(row_text.contains(herdr_work_code::ui::activity_icon(theme, icon)));
                    assert_eq!(rail[(0, row)].symbol(), " ");
                    assert_eq!(rail[(3, row)].symbol(), " ");
                    assert_eq!(
                        rail[(1, row)].symbol(),
                        herdr_work_code::ui::activity_icon(theme, icon)
                    );
                }
                // Projects opens independently; the sidebar stays collapsed.
                click(&mut sidebar, 2, 10);
                assert!(sidebar.take_projects_request());
                assert!(!sidebar.take_projects_request());
                assert_eq!(render(&mut sidebar, 4, 24)[(1, 22)].symbol(), "»");
                click(&mut sidebar, 2, 22);
                let after = render(&mut sidebar, 38, 24);
                assert_eq!(before, after, "expand must preserve the current app");
                click(&mut sidebar, 36, 23);
                render(&mut sidebar, 4, 24);
                click(&mut sidebar, 2, 4);
                assert!(state::load_state().search_active);
                assert!(!text(&render(&mut sidebar, 38, 24)).contains('»'));
                click(&mut sidebar, 36, 23);
                render(&mut sidebar, 4, 24);
                click(&mut sidebar, 2, 7);
                assert_eq!(state::load_state().active, state::View::SourceControl);
                render(&mut sidebar, 38, 24);
                click(&mut sidebar, 36, 23);
                render(&mut sidebar, 4, 24);
                click(&mut sidebar, 2, 13);
                assert!(sidebar.busy());
                let settings = render(&mut sidebar, 38, 24);
                assert_eq!(settings[(0, 0)].symbol(), "┌");
                sidebar.event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
                assert!(!sidebar.busy());
                assert!(sidebar.flush());
            }
        }
    }
    let mut sidebar = Sidebar::new(root.clone(), Some(InitialView::Git));
    render(&mut sidebar, 38, 24);
    click(&mut sidebar, 3, 5);
    for c in "保留的提交草稿".chars() {
        sidebar.event(Event::Key(KeyEvent::new(
            KeyCode::Char(c),
            KeyModifiers::NONE,
        )));
    }
    assert!(sidebar.busy(), "commit input should retain focus");
    render(&mut sidebar, 38, 24);
    click(&mut sidebar, 36, 23);
    assert!(text(&render(&mut sidebar, 4, 24)).contains('»'));
    sidebar.event(Event::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert!(
        text(&render(&mut sidebar, 38, 24))
            .replace(' ', "")
            .contains("保留的提交草稿")
    );
    assert!(sidebar.flush());
    if std::env::var_os("HERDR_PANE_ID").is_none() {
        assert!(
            click(&mut sidebar, 36, 0),
            "without a host, × must request terminal shutdown"
        );
        let mut sidebar = Sidebar::new(root, Some(InitialView::Explorer));
        render(&mut sidebar, 38, 24);
        click(&mut sidebar, 36, 23);
        render(&mut sidebar, 4, 24);
        assert!(
            click(&mut sidebar, 2, 23),
            "collapsed × must also request terminal shutdown"
        );
    }
}
