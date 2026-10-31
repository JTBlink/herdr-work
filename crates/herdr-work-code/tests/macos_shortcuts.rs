#![cfg(feature = "sidebar-ui")]

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use herdr_work_code::{activity::InitialView, sidebar_component::Sidebar, state};
use ratatui::{Terminal, backend::TestBackend};

fn press(sidebar: &mut Sidebar, c: char, modifiers: KeyModifiers) {
    sidebar.event(Event::Key(KeyEvent::new(KeyCode::Char(c), modifiers)));
}

fn draw(sidebar: &mut Sidebar) {
    let mut terminal = Terminal::new(TestBackend::new(38, 32)).unwrap();
    terminal.draw(|frame| sidebar.draw(frame)).unwrap();
}

#[test]
fn board_and_find_shortcuts_work_without_function_keys_and_preserve_typed_digits() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("demo");
    std::fs::create_dir(&root).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    state::configure_product_profile(&fixture.path().join("profile")).unwrap();
    state::update_state(|settings| settings.merged = true);
    for initial in [InitialView::Explorer, InitialView::Search, InitialView::Git] {
        for collapsed in [false, true] {
            let mut sidebar = Sidebar::new(root.clone(), Some(initial));
            if collapsed {
                sidebar.start_collapsed();
            }
            draw(&mut sidebar);
            press(&mut sidebar, '4', KeyModifiers::NONE);
            assert!(
                sidebar.take_projects_request(),
                "4 must open the board from {initial:?}, collapsed={collapsed}"
            );
            assert!(!sidebar.take_projects_request());
        }
        for shortcut in ['f', 'p'] {
            for collapsed in [false, true] {
                let mut sidebar = Sidebar::new(root.clone(), Some(initial));
                if collapsed {
                    sidebar.start_collapsed();
                }
                draw(&mut sidebar);
                press(&mut sidebar, shortcut, KeyModifiers::SUPER);
                assert!(
                    sidebar.busy(),
                    "Command+{shortcut} must focus an input from {initial:?}, collapsed={collapsed}"
                );
                press(&mut sidebar, '4', KeyModifiers::NONE);
                assert!(
                    !sidebar.take_projects_request(),
                    "typing 4 must not open the board"
                );
            }
        }
    }
    let mut sidebar = Sidebar::new(root, Some(InitialView::Git));
    draw(&mut sidebar);
    press(&mut sidebar, 'c', KeyModifiers::NONE);
    assert!(sidebar.busy(), "commit input must have focus");
    press(&mut sidebar, '4', KeyModifiers::NONE);
    assert!(
        !sidebar.take_projects_request(),
        "commit digits must stay in the draft"
    );
}
