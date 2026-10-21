#![cfg(feature = "sidebar-ui")]

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use herdr_work_code::{activity::InitialView, sidebar_component::Sidebar, state};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn commit_controls_leave_room_for_files_and_keep_input_hit_testing() {
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
    std::fs::write(root.join("visible.txt"), "change\n").unwrap();
    state::configure_product_profile(&directory.path().join("profile")).unwrap();
    state::update_state(|settings| {
        settings.font_prompt_done = true;
        settings.icons = Some(herdr_work_code::icons::IconTheme::Emoji);
    });
    for (width, height) in [(28, 12), (38, 24)] {
        let mut sidebar = Sidebar::new(root.clone(), Some(InitialView::Git));
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| sidebar.draw(frame)).unwrap();
        let lines = |terminal: &Terminal<TestBackend>| {
            terminal
                .backend()
                .buffer()
                .content
                .chunks(width as usize)
                .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
                .collect::<Vec<_>>()
        };
        let before = lines(&terminal);
        let input = before
            .iter()
            .position(|line| line.contains("Message"))
            .unwrap_or_else(|| panic!("missing input: {before:#?}"));
        let commit = before
            .iter()
            .position(|line| line.contains("Commit") && !line.contains("Message"))
            .unwrap();
        assert_eq!(
            commit,
            input + 2,
            "one input line, underline, then one button"
        );
        assert!(before[commit + 1].contains("Changes"));
        assert!(before.iter().any(|line| line.contains("visible.txt")));
        if width == 38
            && let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES")
        {
            std::fs::create_dir_all(&output).unwrap();
            let cells = terminal.backend().buffer().content.iter().map(|cell|
                serde_json::json!({"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg)})
            ).collect::<Vec<_>>();
            std::fs::write(std::path::Path::new(&output).join("compact-scm.json"),
                serde_json::to_vec(&serde_json::json!({"width":width,"height":height,"cells":cells,"fixture":true})).unwrap()).unwrap();
        }
        sidebar.event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: input as u16,
            modifiers: KeyModifiers::NONE,
        }));
        for ch in "fix layout".chars() {
            sidebar.event(Event::Key(KeyEvent::new(
                KeyCode::Char(ch),
                KeyModifiers::NONE,
            )));
        }
        terminal.draw(|frame| sidebar.draw(frame)).unwrap();
        assert!(
            lines(&terminal)[input].contains("fix layout"),
            "input click must focus the rendered content row"
        );
        assert!(sidebar.flush());
        // Each viewport starts with an empty draft.
        let mut saved = state::load_scm_state(&root);
        saved.cleared_drafts.extend(saved.drafts.keys().cloned());
        saved.drafts.clear();
        assert!(state::save_scm_state(&root, &saved));
    }
}
