use super::*;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn right_click_targets_clicked_file_and_scrolled_menu_renames_that_file() {
    let root = std::env::temp_dir().join(format!("hw-menu-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.txt"), "first").unwrap();
    std::fs::write(root.join("b.txt"), "second").unwrap();
    let mut app = App::new(root.clone(), Default::default());
    let mut terminal = Terminal::new(TestBackend::new(28, 10)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    app.select(0);
    let index = app.rows.iter().position(|r| r.name == "b.txt").unwrap();
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 4,
        row: app.body.top + index as u16,
        modifiers: KeyModifiers::CONTROL,
    });
    let Some(Overlay::Menu {
        target, entries, ..
    }) = &app.overlay
    else {
        panic!("Right click did not open the file menu");
    };
    assert_eq!(target.as_ref().unwrap().0, root.join("b.txt"));
    for action in [MenuAction::CopyPath, MenuAction::CopyRelativePath] {
        assert!(
            entries
                .iter()
                .any(|entry| matches!(entry, MenuEntry::Action(a, _) if *a == action))
        );
    }
    let rename = entries
        .iter()
        .position(|e| matches!(e, MenuEntry::Action(MenuAction::Rename, _)))
        .unwrap();
    // Use the real keyboard navigation, then click the rendered scrolled row.
    while let Some(Overlay::Menu { selected, .. }) = &app.overlay {
        if *selected == rename {
            break;
        }
        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let Some(Overlay::Menu {
        viewport, selected, ..
    }) = &app.overlay
    else {
        unreachable!()
    };
    assert!(viewport.offset > 0);
    let row = viewport.rect.y + 1 + (selected - viewport.offset) as u16;
    let col = viewport.rect.x + 2;
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(&app.overlay,
        Some(Overlay::Prompt { kind: PromptKind::Rename(path), .. }) if *path == root.join("b.txt")));
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
