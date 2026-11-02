#![cfg(feature = "sidebar-ui")]

use crossterm::event::{Event, KeyModifiers, MouseEvent, MouseEventKind};
use herdr_work_code::{activity::InitialView, icons::IconTheme, sidebar_component::Sidebar, state};
use ratatui::{Terminal, backend::TestBackend};

fn screen(sidebar: &mut Sidebar, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(38, height)).unwrap();
    terminal.draw(|frame| sidebar.draw(frame)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(38)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn wheel_to_bottom(sidebar: &mut Sidebar) {
    for _ in 0..30 {
        sidebar.event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 3,
            row: 10,
            modifiers: KeyModifiers::NONE,
        }));
    }
}

#[test]
fn changed_files_remain_visible_after_scrolling_resizing_and_restoring() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("demo");
    std::fs::create_dir(&root).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    for i in 0..13 {
        std::fs::write(root.join(format!("change-{i:02}.txt")), "change\n").unwrap();
    }
    state::configure_product_profile(&directory.path().join("profile")).unwrap();
    state::update_state(|settings| settings.font_prompt_done = true);
    for theme in [IconTheme::Material, IconTheme::Emoji] {
        state::update_state(|settings| settings.icons = Some(theme));
        let mut saved = state::load_scm_state(&root);
        saved.scroll = 0;
        saved.selected = None;
        assert!(state::save_scm_state(&root, &saved));
        let mut sidebar = Sidebar::new(root.clone(), Some(InitialView::Git));
        assert!(screen(&mut sidebar, 48).contains("change-00.txt"));
        wheel_to_bottom(&mut sidebar);
        let text = screen(&mut sidebar, 48);
        assert!(
            text.contains("Changes") && text.contains("change-00.txt"),
            "scrolling a list that fits must not hide changed files: {text}"
        );

        screen(&mut sidebar, 18);
        wheel_to_bottom(&mut sidebar);
        let narrow = screen(&mut sidebar, 18);
        assert!(narrow.contains("Tags") && narrow.contains("change-12.txt"));
        let grown = screen(&mut sidebar, 48);
        assert!(grown.contains("Changes") && grown.contains("change-00.txt"));
        assert!(sidebar.flush());

        let mut saved = state::load_scm_state(&root);
        saved.scroll = usize::MAX;
        assert!(state::save_scm_state(&root, &saved));
        let mut reopened = Sidebar::new(root.clone(), Some(InitialView::Git));
        let restored = screen(&mut reopened, 48);
        assert!(restored.contains("Changes") && restored.contains("change-00.txt"));
    }
}
