#![cfg(feature = "sidebar-ui")]

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use herdr_work_code::{
    activity::InitialView,
    icons::IconTheme,
    sidebar_component::Sidebar,
    state,
    ui::{ActivityIcon, activity_icon},
};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn projects_button_requests_the_host_without_closing_or_switching_sidebar() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("demo");
    std::fs::create_dir(&root).unwrap();
    state::configure_product_profile(&directory.path().join("profile")).unwrap();
    for theme in [IconTheme::Material, IconTheme::Emoji] {
        state::update_state(|settings| {
            settings.icons = Some(theme);
            settings.merged = true;
        });
        for initial in [InitialView::Explorer, InitialView::Search, InitialView::Git] {
            for width in [24, 28, 38, 80] {
                let mut sidebar = Sidebar::new(root.clone(), Some(initial));
                let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
                terminal.draw(|frame| sidebar.draw(frame)).unwrap();
                let buffer = terminal.backend().buffer();
                let (x, y) = (0..3)
                    .flat_map(|y| (0..width).map(move |x| (x, y)))
                    .find(|&(x, y)| {
                        buffer[(x, y)].symbol() == activity_icon(theme, ActivityIcon::Tasks)
                    })
                    .expect("Projects icon must fit before Settings");
                if let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES") {
                    std::fs::create_dir_all(&output).unwrap();
                    let cells = buffer.content.iter().map(|cell| serde_json::json!({
                        "text":cell.symbol(), "fg":format!("{:?}",cell.fg), "bg":format!("{:?}",cell.bg)
                    })).collect::<Vec<_>>();
                    std::fs::write(std::path::Path::new(&output).join(format!("projects-{theme:?}-{initial:?}-{width}.json")),
                        serde_json::to_vec(&serde_json::json!({"width":width,"height":30,"cells":cells,"fixture":true})).unwrap()).unwrap();
                }
                // The whole three-row chip is clickable, including its caps.
                for row in y.saturating_sub(1)..=y + 1 {
                    assert!(!sidebar.event(Event::Mouse(MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        column: x,
                        row,
                        modifiers: KeyModifiers::NONE,
                    })));
                    assert!(
                        sidebar.take_projects_request(),
                        "{initial:?} {theme:?} {width} row {row}"
                    );
                    assert!(!sidebar.take_projects_request());
                    assert_eq!(sidebar.root(), root);
                }
                assert!(!sidebar.event(Event::Key(KeyEvent::new(
                    KeyCode::Char('4'),
                    KeyModifiers::CONTROL
                ))));
                assert!(sidebar.take_projects_request());
                let view = state::load_state();
                assert_eq!(
                    view.active == state::View::SourceControl,
                    initial == InitialView::Git
                );
                assert_eq!(view.search_active, initial == InitialView::Search);
                assert!(sidebar.flush());
            }
        }
    }
}
