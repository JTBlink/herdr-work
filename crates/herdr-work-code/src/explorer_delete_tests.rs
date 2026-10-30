use super::*;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    terminal.backend().buffer().clone()
}
fn key(app: &mut App, code: KeyCode) {
    assert!(
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE))
            .is_none()
    );
}
fn open(app: &mut App, path: &Path, is_dir: bool) {
    app.show_menu(0, 0, Some((path.to_owned(), is_dir)));
    if let Some(Overlay::Menu {
        entries, selected, ..
    }) = &mut app.overlay
    {
        *selected = entries
            .iter()
            .position(|entry| matches!(entry, MenuEntry::Action(MenuAction::Delete, _)))
            .unwrap();
    }
    app.activate_menu_entry();
    assert!(matches!(app.overlay, Some(Overlay::ConfirmDelete(_))));
}
fn point(buffer: &Buffer, symbol: &str) -> (u16, u16) {
    let index = buffer
        .content
        .iter()
        .rposition(|cell| cell.symbol() == symbol)
        .unwrap();
    (
        index as u16 % buffer.area.width,
        index as u16 / buffer.area.width,
    )
}
fn click(app: &mut App, at: (u16, u16)) {
    assert!(
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: at.0,
            row: at.1,
            modifiers: KeyModifiers::NONE
        })
        .is_none()
    );
}

#[test]
fn delete_cancel_close_and_default_enter_leave_the_target_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("keep.txt");
    std::fs::write(&path, "keep").unwrap();
    let mut app = App::new(temp.path().into(), Default::default());
    for code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Char('n')] {
        open(&mut app, &path, false);
        render(&mut app, 28, 24);
        key(&mut app, code);
        assert!(app.overlay.is_none());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep");
    }
    for symbol in ["取", "×"] {
        open(&mut app, &path, false);
        let buffer = render(&mut app, 28, 24);
        click(&mut app, point(&buffer, symbol));
        assert!(app.overlay.is_none());
        assert!(path.exists());
    }
}

#[test]
fn delete_mouse_confirms_the_menu_target_not_the_selected_neighbor() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("a.txt");
    let neighbor = temp.path().join("b.txt");
    std::fs::write(&target, "remove").unwrap();
    std::fs::write(&neighbor, "keep").unwrap();
    let mut app = App::new(temp.path().into(), Default::default());
    let index = app
        .rows
        .iter()
        .position(|row| row.path == neighbor)
        .unwrap();
    app.select(index);
    open(&mut app, &target, false);
    let buffer = render(&mut app, 28, 24);
    let confirm = point(&buffer, "确");
    click(&mut app, confirm);
    assert!(!target.exists());
    assert_eq!(std::fs::read_to_string(&neighbor).unwrap(), "keep");
    assert!(app.overlay.is_none());
    assert!(app.rows.iter().all(|row| row.path != target));
    click(&mut app, confirm);
    assert!(
        neighbor.exists(),
        "a repeated click must not delete the next tree row"
    );
}

#[test]
fn delete_keyboard_navigation_confirms_a_nonempty_directory() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("folder");
    std::fs::create_dir_all(target.join("nested")).unwrap();
    std::fs::write(target.join("nested/file.txt"), "remove").unwrap();
    let mut app = App::new(temp.path().into(), Default::default());
    open(&mut app, &target, true);
    render(&mut app, 28, 24);
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Enter);
    assert!(!target.exists());
    assert!(app.overlay.is_none());
}

#[test]
fn delete_failure_keeps_the_target_and_allows_an_explicit_retry() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("changed.txt");
    std::fs::write(&target, "initial").unwrap();
    let mut app = App::new(temp.path().into(), Default::default());
    open(&mut app, &target, false);
    let buffer = render(&mut app, 28, 24);
    // A filesystem failure that works without relying on user permissions.
    std::fs::remove_file(&target).unwrap();
    std::fs::create_dir(&target).unwrap();
    click(&mut app, point(&buffer, "确"));
    let Some(Overlay::ConfirmDelete(dialog)) = &app.overlay else {
        panic!("failure dismissed the modal")
    };
    assert_eq!(dialog.target().0, target);
    let buffer = render(&mut app, 60, 24);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
    assert!(text.contains('失') && text.contains('败'));
    assert!(target.is_dir());
    std::fs::remove_dir(&target).unwrap();
    std::fs::write(&target, "retry").unwrap();
    key(&mut app, KeyCode::Char('y'));
    assert!(!target.exists());
    assert!(app.overlay.is_none());
}

#[test]
fn delete_modal_owns_shortcuts_and_background_mouse_input() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("keep.txt");
    std::fs::write(&target, "keep").unwrap();
    let mut app = App::new(temp.path().into(), Default::default());
    open(&mut app, &target, false);
    render(&mut app, 60, 24);
    let selected = app.selected;
    let scroll = app.scroll;
    for (code, modifiers) in [
        (KeyCode::Char('p'), KeyModifiers::CONTROL),
        (KeyCode::Char('q'), KeyModifiers::CONTROL),
        (KeyCode::Char('3'), KeyModifiers::CONTROL),
        (KeyCode::Char('y'), KeyModifiers::ALT),
        (KeyCode::F(9), KeyModifiers::NONE),
    ] {
        assert!(app.on_key(KeyEvent::new(code, modifiers)).is_none());
        assert!(matches!(app.overlay, Some(Overlay::ConfirmDelete(_))));
        assert!(target.exists());
    }
    click(&mut app, (0, 0));
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(app.overlay, Some(Overlay::ConfirmDelete(_))));
    assert_eq!(app.selected, selected);
    assert_eq!(app.scroll, scroll);
    key(&mut app, KeyCode::Enter);
    assert!(target.exists());
}

#[test]
fn delete_confirmation_adapts_to_narrow_panes_and_captures_the_actual_component() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("demo-project");
    let target = root.join("assets");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(root.join("README.md"), "fixture").unwrap();
    let mut app = App::new(root, Default::default());
    app.theme = IconTheme::Emoji;
    open(&mut app, &target, true);
    for (width, height) in [(80, 24), (28, 24), (20, 20)] {
        let buffer = render(&mut app, width, height);
        let cancel = point(&buffer, "取");
        let confirm = point(&buffer, "确");
        assert_ne!(cancel, confirm);
        assert!(cancel.0 < width && confirm.0 < width);
        assert!(cancel.1 < height && confirm.1 < height);
        if let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES") {
            std::fs::create_dir_all(&output).unwrap();
            let cells: Vec<_> = buffer.content.iter().map(|cell| serde_json::json!({
                "text":cell.symbol(), "fg":format!("{:?}",cell.fg), "bg":format!("{:?}",cell.bg)
            })).collect();
            std::fs::write(Path::new(&output).join(format!("delete-confirmation-{width}.json")),
                serde_json::to_vec(&serde_json::json!({"width":width,"height":height,"cells":cells,"fixture":true})).unwrap()).unwrap();
        }
    }
    for (width, height) in [(1, 1), (8, 5), (12, 6)] {
        render(&mut app, width, height);
        key(&mut app, KeyCode::Char('y'));
        assert!(target.exists());
        assert!(matches!(app.overlay, Some(Overlay::ConfirmDelete(_))));
    }
    key(&mut app, KeyCode::Esc);
    assert!(target.exists());
}

#[test]
fn long_unicode_delete_targets_wrap_without_covering_buttons() {
    let temp = tempfile::tempdir().unwrap();
    let name = format!("{}.txt", "这是需要确认删除的文件".repeat(3));
    let target = temp.path().join(&name);
    std::fs::write(&target, "keep").unwrap();
    let mut app = App::new(temp.path().into(), Default::default());
    open(&mut app, &target, false);
    let buffer = render(&mut app, 28, 24);
    let text: String = buffer
        .content
        .iter()
        .flat_map(|cell| cell.symbol().chars())
        .filter(|ch| !ch.is_whitespace() && *ch != '│')
        .collect();
    assert!(text.contains(&name), "expected {name}; got {text}");
    let cancel = point(&buffer, "取");
    let confirm = point(&buffer, "确");
    assert_ne!(cancel, confirm);
    click(&mut app, cancel);
    assert!(target.exists());
}
