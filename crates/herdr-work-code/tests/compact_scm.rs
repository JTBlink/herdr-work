#![cfg(feature = "sidebar-ui")]

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use herdr_work_code::{activity::InitialView, icons::IconTheme, sidebar_component::Sidebar, state};
use ratatui::{Terminal, backend::TestBackend};

fn export_frame(sidebar: &mut Sidebar, width: u16, height: u16, name: &str) {
    if let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES") {
        std::fs::create_dir_all(&output).unwrap();
        // Render a fresh screen: TestBackend retains skipped continuation cells
        // after narrow text is replaced by wide CJK glyphs.
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| sidebar.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let cells = buffer.content.iter().map(|cell|
            serde_json::json!({"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg)})
        ).collect::<Vec<_>>();
        std::fs::write(std::path::Path::new(&output).join(format!("{name}.json")),
            serde_json::to_vec(&serde_json::json!({"width":buffer.area.width,"height":buffer.area.height,"cells":cells,"fixture":true})).unwrap()).unwrap();
    }
}

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
    });
    for (theme, width, height) in [
        (IconTheme::Emoji, 28, 12),
        (IconTheme::Emoji, 38, 24),
        (IconTheme::Material, 24, 12),
        (IconTheme::Material, 38, 24),
    ] {
        state::update_state(|settings| settings.icons = Some(theme));
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
        assert!(
            before[input - 1].starts_with('┌') && before[input - 1].ends_with('┐'),
            "input needs a complete top edge, separate from the repository header: {before:#?}"
        );
        assert!(before[input + 1].starts_with('└') && before[input + 1].ends_with('┘'));
        assert_eq!(
            commit,
            input + 2,
            "one input line, underline, then one button"
        );
        assert!(before[commit + 1].contains("Changes"));
        assert!(before.iter().any(|line| line.contains("visible.txt")));
        if width == 38 {
            export_frame(&mut sidebar, width, height, "compact-scm");
        }
        sidebar.event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: input as u16,
            modifiers: KeyModifiers::NONE,
        }));
        terminal.draw(|frame| sidebar.draw(frame)).unwrap();
        assert!(
            lines(&terminal)[input].contains("Message"),
            "an empty focused input must retain its hint"
        );
        let cursor = terminal.get_cursor_position().unwrap();
        assert_eq!((cursor.x, cursor.y), (1, input as u16));
        if width == 38 {
            export_frame(
                &mut sidebar,
                width,
                height,
                &format!("scm-input-empty-{theme:?}"),
            );
        }
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
        sidebar.event(Event::Key(KeyEvent::new(
            KeyCode::Char('u'),
            KeyModifiers::CONTROL,
        )));
        for ch in "修复输入框布局和光标位置".repeat(5).chars() {
            sidebar.event(Event::Key(KeyEvent::new(
                KeyCode::Char(ch),
                KeyModifiers::NONE,
            )));
        }
        terminal.draw(|frame| sidebar.draw(frame)).unwrap();
        let wrapped = lines(&terminal);
        let cursor = terminal.get_cursor_position().unwrap();
        let bottom = wrapped
            .iter()
            .position(|line| line.starts_with('└'))
            .unwrap();
        assert!(cursor.x > 0 && cursor.x < width - 1);
        assert!(
            cursor.y >= input as u16 && cursor.y < bottom as u16,
            "wrapped cursor must stay inside the input: {cursor:?} {wrapped:#?}"
        );
        if width == 38 {
            export_frame(
                &mut sidebar,
                width,
                height,
                &format!("scm-input-wrapped-{theme:?}"),
            );
        }
        sidebar.event(Event::Key(KeyEvent::new(
            KeyCode::Char('u'),
            KeyModifiers::CONTROL,
        )));
        terminal.draw(|frame| sidebar.draw(frame)).unwrap();
        assert!(lines(&terminal)[input].contains("Message"));
        assert!(sidebar.flush());
        // Each viewport starts with an empty draft.
        let mut saved = state::load_scm_state(&root);
        saved.cleared_drafts.extend(saved.drafts.keys().cloned());
        saved.drafts.clear();
        assert!(state::save_scm_state(&root, &saved));
    }
}
