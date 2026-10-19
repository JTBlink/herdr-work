use super::*;
use ratatui::{Terminal, backend::TestBackend};
use std::{process::Command, time::Duration};

fn app() -> (tempfile::TempDir, App) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(root.join("visible.txt"), "fixture\n").unwrap();
    let desk = HerdrWork::open(
        &temp.path().join("profile"),
        Some(temp.path().join("missing-backend")),
    )
    .unwrap();
    let app = App::new(desk, root, Context::default()).unwrap();
    (temp, app)
}
fn screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(160, 32)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
        .replace(' ', "")
}
fn capture_fixture(app: &mut App, name: &str) {
    let Some(directory) = std::env::var_os("HERDR_WORK_TEST_FRAMES") else {
        return;
    };
    let original = app.root.clone();
    app.root = "/example/herdr-work".into();
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal.draw(|f| app.draw(f)).unwrap();
    app.root = original;
    let cells=terminal.backend().buffer().content.iter().map(|c|serde_json::json!({"text":c.symbol(),"fg":format!("{:?}",c.fg),"bg":format!("{:?}",c.bg)})).collect::<Vec<_>>();
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        std::path::Path::new(&directory).join(format!("{name}.json")),
        serde_json::to_vec(
            &serde_json::json!({"width":120,"height":32,"cells":cells,"fixture":true}),
        )
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn completed_empty_startup_has_guidance_and_one_task_list() {
    let (_temp, mut app) = app();
    app.switch_tab(Tab::Tasks);
    app.apply(0, Ok(Data::Tasks(Ok(vec![]))));
    app.refresh().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert!(matches!(app.tasks_state, LoadState::Ready));
    assert!(matches!(app.code_state, LoadState::Ready));
    let rendered = screen(&mut app);
    assert!(
        !rendered.contains("Loading"),
        "Completed startup still looks stuck"
    );
    assert!(
        rendered.contains("暂无任务"),
        "Empty task view offers no explanation"
    );
    assert!(
        rendered.contains("创建任务"),
        "New projects need task creation guidance"
    );
    assert_eq!(
        rendered.matches("Tasks").count(),
        1,
        "One task list expected"
    );
}

#[test]
fn backend_failure_does_not_hide_loaded_files_or_look_pending() {
    let (_temp, mut app) = app();
    app.load_tasks().unwrap();
    app.refresh().unwrap();
    for _ in 0..2 {
        let (epoch, data) = app
            .jobs
            .results
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        app.apply(epoch, data);
    }
    assert!(
        app.code
            .as_ref()
            .unwrap()
            .files
            .iter()
            .any(|f| f.name == "visible.txt")
    );
    let rendered = screen(&mut app);
    assert!(!rendered.contains("Loading"));
    assert!(rendered.contains("任务后端不可用"));
}

#[test]
fn failed_code_load_can_retry_and_stale_errors_cannot_replace_ready_data() {
    let (_temp, mut app) = app();
    let root = app.root.clone();
    app.root = root.join("missing");
    app.refresh().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert!(matches!(app.code_state, LoadState::Failed(_)));
    app.root = root;
    app.refresh().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert!(matches!(app.code_state, LoadState::Ready));
    app.apply(
        epoch,
        Ok(Data::Code(1, Err(anyhow::anyhow!("old failure")))),
    );
    assert!(matches!(app.code_state, LoadState::Ready));
    assert!(app.code.is_some());
}

#[test]
fn default_activity_exposes_explorer_files_and_preview() {
    let (_temp, mut app) = app();
    assert!(
        app.tab == Tab::Files,
        "Default activity hides Explorer behind Tasks"
    );
    app.apply(0, Ok(Data::Tasks(Ok(vec![]))));
    app.refresh().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    let rendered = screen(&mut app);
    assert!(rendered.contains("EXPLORER"));
    assert!(rendered.contains("visible.txt"));
    assert!(rendered.contains("Preview"));
    assert!(
        !rendered.contains("暂无任务"),
        "Tasks still occupies the default sidebar"
    );
    capture_fixture(&mut app, "explorer");
}

fn loaded_app() -> (tempfile::TempDir, App) {
    let (temp, mut app) = app();
    app.refresh().unwrap();
    receive(&mut app);
    (temp, app)
}
fn receive(app: &mut App) {
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
}
#[test]
fn explorer_click_opens_file_and_content_scroll_keeps_tree_selection() {
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (_temp, mut app) = loaded_app();
    std::fs::write(
        app.root.join("visible.txt"),
        "first line\nsecond line\nthird line",
    )
    .unwrap();
    let panes = crate::layout::panes(ratatui::layout::Rect::new(0, 0, 100, 32), Focus::Sidebar);
    let index = app
        .code
        .as_ref()
        .unwrap()
        .files
        .iter()
        .position(|f| f.name == "visible.txt")
        .unwrap();
    app.mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: panes.sidebar.x + 3,
            row: panes.sidebar.y + 1 + index as u16,
            modifiers: KeyModifiers::NONE,
        },
        ratatui::layout::Rect::new(0, 0, 100, 32),
    )
    .unwrap();
    receive(&mut app);
    assert!(app.text.contains("first line"));
    let selected = app.selection;
    app.action(KeyCode::Tab).unwrap();
    app.action(KeyCode::Down).unwrap();
    assert_eq!(app.preview_scroll, 1);
    assert_eq!(app.selection, selected);
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| app.draw(f)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("EXPLORER") && text.contains("Preview"));
    assert!(text.contains("second line"));
}
#[test]
fn source_control_activity_and_pointer_show_real_diff() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (temp, mut app) = app();
    for args in [
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@example.invalid"],
        vec!["add", "."],
        vec!["commit", "-qm", "fixture"],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&app.root)
                .status()
                .unwrap()
                .success()
        );
    }
    std::fs::write(app.root.join("visible.txt"), "actual changed content\n").unwrap();
    app.refresh().unwrap();
    receive(&mut app);
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    let panes = crate::layout::panes(area, Focus::Sidebar);
    app.mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: panes.activity.y + 7,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert!(app.tab == Tab::Scm);
    let rendered = screen(&mut app);
    assert!(rendered.contains("SOURCECONTROL") && rendered.contains("Changes(1)"));
    let row = app.scm_rows().iter().position(|r| r.0 == Some(0)).unwrap();
    app.mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: panes.sidebar.x + 3,
            row: panes.sidebar.y + 1 + row as u16,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    receive(&mut app);
    assert!(
        app.tab == Tab::Scm,
        "Opening a Git diff changed stage into send"
    );
    assert!(app.focus == Focus::Sidebar);
    assert!(screen(&mut app).contains("actualchangedcontent"));
    assert!(temp.path().join("profile/reviews").exists());
}
#[test]
fn late_preview_for_another_file_cannot_replace_the_current_file() {
    let (_temp, mut app) = app();
    let old = app.root.join("old.txt");
    let current = app.root.join("current.txt");
    app.preview_path = Some(current.clone());
    app.text = "current content".into();
    app.apply(0, Ok(Data::Preview(old.clone(), Ok("old content".into()))));
    assert_eq!(app.text, "current content");
    app.apply(
        0,
        Ok(Data::Preview(old, Err(anyhow::anyhow!("old failure")))),
    );
    assert_eq!(app.text, "current content");
    app.apply(
        0,
        Ok(Data::Preview(current.clone(), Ok("fresh content".into()))),
    );
    assert_eq!(app.text, "fresh content");
    app.apply(
        0,
        Ok(Data::Preview(current, Err(anyhow::anyhow!("unreadable")))),
    );
    assert!(app.text.contains("无法预览文件"));
    app.context.epoch = 2;
    let status = app.status.clone();
    app.apply(1, Err(anyhow::anyhow!("stale context error")));
    assert_eq!(app.status, status);
}

#[test]
fn pending_editor_cannot_open_after_switching_activity() {
    let (_temp, mut app) = loaded_app();
    app.selection = app
        .code
        .as_ref()
        .unwrap()
        .files
        .iter()
        .position(|r| r.name == "visible.txt")
        .unwrap();
    app.start_editor().unwrap();
    app.switch_tab(Tab::Scm);
    receive(&mut app);
    assert!(app.editor.is_none());
    assert!(app.tab == Tab::Scm);
}

#[test]
fn settings_and_row_menu_are_visible_and_keyboard_accessible() {
    use crossterm::event::KeyCode;
    let (_temp, mut app) = loaded_app();
    assert!(
        screen(&mut app).contains("设置"),
        "Default workspace hides Sidebar settings"
    );
    app.key(KeyCode::Char(',')).unwrap();
    assert!(screen(&mut app).contains("侧栏设置"));
    app.key(KeyCode::Esc).unwrap();
    app.key(KeyCode::Char('m')).unwrap();
    assert!(screen(&mut app).contains("文件菜单"));
    assert!(screen(&mut app).contains("预览文件"));
}

#[test]
fn original_editor_saves_text_undoes_and_protects_unsaved_and_external_changes() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = loaded_app();
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    app.selection = app
        .code
        .as_ref()
        .unwrap()
        .files
        .iter()
        .position(|r| r.name == "visible.txt")
        .unwrap();
    app.start_editor().unwrap();
    receive(&mut app);
    let key = |code, modifiers| KeyEvent::new(code, modifiers);
    app.input_key(key(KeyCode::Char('q'), KeyModifiers::NONE), area)
        .unwrap();
    assert!(
        app.editor.as_ref().unwrap().editor.dirty,
        "q was treated as global quit"
    );
    app.input_key(key(KeyCode::Char('z'), KeyModifiers::CONTROL), area)
        .unwrap();
    assert_eq!(app.editor.as_ref().unwrap().editor.text(), "fixture\n");
    app.input_key(key(KeyCode::Char('y'), KeyModifiers::CONTROL), area)
        .unwrap();
    app.input_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL), area)
        .unwrap();
    receive(&mut app);
    assert_eq!(
        std::fs::read_to_string(app.root.join("visible.txt")).unwrap(),
        "qfixture\n"
    );
    receive(&mut app);
    app.input_key(key(KeyCode::Char('x'), KeyModifiers::NONE), area)
        .unwrap();
    app.input_key(key(KeyCode::Esc, KeyModifiers::NONE), area)
        .unwrap();
    assert!(screen(&mut app).contains("未保存的文本"));
    app.input_key(key(KeyCode::Esc, KeyModifiers::NONE), area)
        .unwrap();
    assert!(app.editor.as_ref().unwrap().editor.dirty);
    std::fs::write(app.root.join("visible.txt"), "external writer\n").unwrap();
    app.input_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL), area)
        .unwrap();
    receive(&mut app);
    assert!(app.status.contains("外部修改"));
    assert_eq!(
        std::fs::read_to_string(app.root.join("visible.txt")).unwrap(),
        "external writer\n"
    );
    assert!(app.editor.as_ref().unwrap().editor.dirty);
}

#[test]
fn sidebar_preferences_persist_and_apply_to_the_original_tree() {
    use crossterm::event::KeyCode;
    let (_temp, mut app) = loaded_app();
    std::fs::write(app.root.join(".hidden"), "hidden").unwrap();
    app.open_settings();
    app.key(KeyCode::Right).unwrap();
    assert_eq!(app.desk.ui_preferences().unwrap().sidebar_width, 36);
    for _ in 0..4 {
        app.key(KeyCode::Down).unwrap();
    }
    app.key(KeyCode::Enter).unwrap();
    receive(&mut app);
    assert!(!app.desk.ui_preferences().unwrap().show_hidden);
    assert!(
        !app.code
            .as_ref()
            .unwrap()
            .files
            .iter()
            .any(|r| r.name == ".hidden")
    );
    app.key(KeyCode::Esc).unwrap();
    let restored = App::new(app.desk.clone(), app.root.clone(), Context::default()).unwrap();
    assert_eq!(restored.preferences, app.preferences);
}

#[test]
fn search_and_eight_git_drawers_use_original_queries() {
    use herdr_work_code::scm::Drawer;
    let (_temp, mut app) = loaded_app();
    for args in [
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@example.invalid"],
        vec!["add", "."],
        vec!["commit", "-qm", "shared query fixture"],
        vec!["tag", "fixture-tag"],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&app.root)
                .status()
                .unwrap()
                .success()
        );
    }
    app.refresh().unwrap();
    receive(&mut app);
    app.apply(app.context.epoch, Ok(Data::Tasks(Ok(vec![]))));
    app.search_query = "fixture".into();
    app.search_quick = false;
    app.switch_tab(Tab::Search);
    app.search().unwrap();
    receive(&mut app);
    assert_eq!(app.search_rows.len(), 1);
    assert_eq!(app.search_rows[0].label, "visible.txt");
    app.open_search_result().unwrap();
    receive(&mut app);
    assert!(app.text.contains("fixture"));
    app.switch_tab(Tab::Scm);
    let collapsed = screen(&mut app);
    for kind in Drawer::ALL {
        assert!(collapsed.contains(&kind.title().replace(' ', "")));
        app.load_drawer(kind).unwrap();
        receive(&mut app);
        assert!(matches!(app.drawer_state, crate::loading::LoadState::Ready));
    }
    assert!(
        app.drawers[&Drawer::Graph]
            .lines
            .iter()
            .any(|r| r.contains("shared query fixture"))
    );
    assert!(
        app.drawers[&Drawer::Tags]
            .lines
            .iter()
            .any(|r| r == "fixture-tag")
    );
    app.load_drawer(Drawer::Graph).unwrap();
    let previous = app.drawer_request;
    app.load_drawer(Drawer::Tags).unwrap();
    receive(&mut app);
    receive(&mut app);
    app.apply(
        app.context.epoch,
        Ok(Data::Show(previous, Ok("stale graph".into()))),
    );
    assert!(app.drawer_show.is_none());
    app.load_drawer(Drawer::Graph).unwrap();
    receive(&mut app);
    app.drawers.get_mut(&Drawer::Graph).unwrap().root = "/example/herdr-work".into();
    capture_fixture(&mut app, "source-control");
}

#[test]
fn editor_wheel_scrolls_without_moving_cursor_or_selection() {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    let (_temp, mut app) = loaded_app();
    std::fs::write(
        app.root.join("visible.txt"),
        (0..120).map(|n| format!("line {n}\n")).collect::<String>(),
    )
    .unwrap();
    app.selection = app
        .code
        .as_ref()
        .unwrap()
        .files
        .iter()
        .position(|f| f.name == "visible.txt")
        .unwrap();
    app.start_editor().unwrap();
    receive(&mut app);
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    let content = app.panes(area).content;
    let cursor = app.editor.as_ref().unwrap().editor.cursor();
    app.mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: content.x + 5,
            row: content.y + 4,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert!(screen(&mut app).contains("line3"));
    assert_eq!(app.editor.as_ref().unwrap().editor.scroll, 3);
    assert_eq!(app.editor.as_ref().unwrap().editor.cursor(), cursor);
}

#[test]
fn clean_editor_reloads_external_changes_but_pending_check_preserves_new_input() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = loaded_app();
    app.selection = app
        .code
        .as_ref()
        .unwrap()
        .files
        .iter()
        .position(|f| f.name == "visible.txt")
        .unwrap();
    app.start_editor().unwrap();
    receive(&mut app);
    std::fs::write(app.root.join("visible.txt"), "from disk\n").unwrap();
    app.poll_editor().unwrap();
    receive(&mut app);
    assert_eq!(app.editor.as_ref().unwrap().editor.text(), "from disk\n");
    std::fs::write(app.root.join("visible.txt"), "new external\n").unwrap();
    app.poll_editor().unwrap();
    // Wait until observation is complete, then type before its result is applied.
    let (epoch, result) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.input_key(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        ratatui::layout::Rect::new(0, 0, 120, 32),
    )
    .unwrap();
    let cursor = app.editor.as_ref().unwrap().editor.cursor();
    app.apply(epoch, result);
    assert_eq!(app.editor.as_ref().unwrap().editor.text(), "xfrom disk\n");
    assert_eq!(app.editor.as_ref().unwrap().editor.cursor(), cursor);
    assert!(screen(&mut app).contains("磁盘已改变"));
}

fn editing_app() -> (tempfile::TempDir, App) {
    let (temp, mut app) = loaded_app();
    app.selection = app
        .code
        .as_ref()
        .unwrap()
        .files
        .iter()
        .position(|f| f.name == "visible.txt")
        .unwrap();
    app.start_editor().unwrap();
    receive(&mut app);
    (temp, app)
}

#[test]
fn save_conflict_menu_can_cancel_reload_or_explicitly_overwrite() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = editing_app();
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
    app.input_key(key(KeyCode::Char('x')), area).unwrap();
    std::fs::write(app.root.join("visible.txt"), "external writer\n").unwrap();
    app.save_editor().unwrap();
    receive(&mut app);
    let rendered = screen(&mut app);
    assert!(rendered.contains("保存冲突") && rendered.contains("明确覆盖"));
    app.input_key(key(KeyCode::Enter), area).unwrap(); // Default continues editing.
    assert!(app.overlay.is_none());
    assert_eq!(app.editor.as_ref().unwrap().editor.text(), "xfixture\n");
    assert_eq!(
        std::fs::read_to_string(app.root.join("visible.txt")).unwrap(),
        "external writer\n"
    );
    app.save_editor().unwrap();
    receive(&mut app);
    app.input_key(key(KeyCode::Down), area).unwrap();
    app.input_key(key(KeyCode::Enter), area).unwrap(); // Reload explicitly discards buffer.
    receive(&mut app);
    assert_eq!(
        app.editor.as_ref().unwrap().editor.text(),
        "external writer\n"
    );
    app.input_key(key(KeyCode::Char('z')), area).unwrap();
    let wanted = app.editor.as_ref().unwrap().editor.text();
    std::fs::write(app.root.join("visible.txt"), "another writer\n").unwrap();
    app.save_editor().unwrap();
    receive(&mut app);
    app.input_key(key(KeyCode::Down), area).unwrap();
    app.input_key(key(KeyCode::Down), area).unwrap();
    app.input_key(key(KeyCode::Enter), area).unwrap();
    receive(&mut app);
    assert_eq!(
        std::fs::read_to_string(app.root.join("visible.txt")).unwrap(),
        wanted
    );
    assert!(!app.editor.as_ref().unwrap().editor.dirty);
}

#[test]
fn input_arriving_while_save_or_reload_is_pending_is_never_lost() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = editing_app();
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    let key = |ch| KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
    app.input_key(key('x'), area).unwrap();
    app.save_editor().unwrap();
    let (epoch, result) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.input_key(key('y'), area).unwrap();
    app.apply(epoch, result);
    assert_eq!(
        std::fs::read_to_string(app.root.join("visible.txt")).unwrap(),
        "xfixture\n"
    );
    assert_eq!(app.editor.as_ref().unwrap().editor.text(), "xyfixture\n");
    assert!(app.editor.as_ref().unwrap().editor.dirty);
    receive(&mut app); // Snapshot refresh after save.
    app.reload_editor().unwrap();
    let (epoch, result) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.input_key(key('z'), area).unwrap();
    app.apply(epoch, result);
    assert_eq!(app.editor.as_ref().unwrap().editor.text(), "xyzfixture\n");
    assert!(app.editor.as_ref().unwrap().editor.dirty);
}

#[test]
fn clean_reload_preserves_cursor_and_selection_for_the_next_edit() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = editing_app();
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    app.input_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), area)
        .unwrap();
    let cursor = app.editor.as_ref().unwrap().editor.cursor();
    std::fs::write(app.root.join("visible.txt"), "fixture\nnew line\n").unwrap();
    app.poll_editor().unwrap();
    receive(&mut app);
    assert_eq!(app.editor.as_ref().unwrap().editor.cursor(), cursor);
    app.input_key(KeyEvent::new(KeyCode::Char('界'), KeyModifiers::NONE), area)
        .unwrap();
    assert_eq!(
        app.editor.as_ref().unwrap().editor.text(),
        "界ixture\nnew line\n"
    );
}

#[test]
fn old_observation_cannot_replace_a_reopened_editor_at_the_same_path() {
    let (_temp, mut app) = editing_app();
    std::fs::write(app.root.join("visible.txt"), "observed old file\n").unwrap();
    app.poll_editor().unwrap();
    let (epoch, old) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.finish_editor().unwrap();
    receive(&mut app);
    std::fs::write(app.root.join("visible.txt"), "reopened file\n").unwrap();
    app.start_editor().unwrap();
    receive(&mut app);
    app.apply(epoch, old);
    assert_eq!(
        app.editor.as_ref().unwrap().editor.text(),
        "reopened file\n"
    );
}

#[test]
fn binding_context_change_during_save_does_not_strand_the_same_editor() {
    let (_temp, mut app) = editing_app();
    app.editor.as_mut().unwrap().editor.insert_text("local ");
    app.save_editor().unwrap();
    let (epoch, result) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    // A previously queued Agent binding may complete while this editor remains open.
    app.context.epoch += 1;
    app.apply(epoch, result);
    assert!(!app.editor.as_ref().unwrap().editor.dirty);
    assert!(app.editor_write.is_none());
}

#[test]
fn quick_open_filters_while_typing_and_keeps_results_navigable() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = app();
    std::fs::write(app.root.join("other.rs"), "other\n").unwrap();
    let area = ratatui::layout::Rect::new(0, 0, 160, 32);
    app.input_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        area,
    )
    .unwrap();
    for _ in 0..1 {
        let (epoch, data) = app
            .jobs
            .results
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        app.apply(epoch, data);
    }
    for c in "visible".chars() {
        app.key(KeyCode::Char(c)).unwrap();
    }
    assert_eq!(app.search_rows.len(), 1);
    assert_eq!(app.search_rows[0].label, "visible.txt");
    assert!(screen(&mut app).contains("visible.txt"));
    app.key(KeyCode::Down).unwrap();
    app.key(KeyCode::Enter).unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert_eq!(app.text, "fixture\n");
}

#[test]
fn content_search_debounces_and_hit_line_survives_result_navigation() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::time::Instant;
    let (_temp, mut app) = app();
    let text = format!(
        "{}needle\n{}needle\n",
        "line\n".repeat(79),
        "line\n".repeat(19)
    );
    std::fs::write(app.root.join("visible.txt"), text).unwrap();
    let area = ratatui::layout::Rect::new(0, 0, 160, 32);
    app.input_key(
        KeyEvent::new(
            KeyCode::Char('F'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
        area,
    )
    .unwrap();
    // Drain the initial empty query; subsequent input stays quiet for 300 ms.
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    for c in "needle".chars() {
        app.key(KeyCode::Char(c)).unwrap();
    }
    let now = Instant::now();
    app.tick_search(now + Duration::from_millis(299)).unwrap();
    assert!(app.jobs.results.try_recv().is_err());
    app.tick_search(now + Duration::from_millis(301)).unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    app.key(KeyCode::Down).unwrap(); // query -> results
    app.key(KeyCode::Enter).unwrap();
    app.key(KeyCode::Down).unwrap(); // selection changes while file is loading
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert_eq!(app.preview_scroll, 79);
    app.key(KeyCode::Up).unwrap();
    app.start_editor().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert_eq!(app.editor.as_ref().unwrap().editor.cursor().line, 79);
}

#[test]
fn file_history_uses_current_change_even_with_another_preview() {
    use herdr_work_code::scm::Drawer;
    let (_temp, mut app) = app();
    for args in [
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@example.invalid"],
        vec!["add", "."],
        vec!["commit", "-qm", "visible history"],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&app.root)
                .status()
                .unwrap()
                .success()
        );
    }
    std::fs::write(app.root.join("visible.txt"), "changed\n").unwrap();
    std::fs::write(app.root.join("other.txt"), "other\n").unwrap();
    app.refresh().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    app.switch_tab(Tab::Scm);
    app.selection = app
        .code
        .as_ref()
        .unwrap()
        .changes
        .iter()
        .position(|r| r.path == "visible.txt")
        .unwrap();
    app.preview_path = Some(app.root.join("other.txt"));
    app.load_drawer(Drawer::FileHistory).unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert!(
        app.drawers[&Drawer::FileHistory]
            .lines
            .iter()
            .any(|r| r.contains("visible history"))
    );
}

#[test]
fn search_mouse_filter_and_include_field_remain_live() {
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (_temp, mut app) = app();
    app.query_prompt(false);
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    let area = ratatui::layout::Rect::new(0, 0, 160, 32);
    let sidebar = app.panes(area).sidebar;
    app.mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: sidebar.x + 2,
            row: sidebar.y + 2,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert!(app.search_options.match_case);
    app.mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: sidebar.x + 2,
            row: sidebar.y + 3,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    for c in "*.txt".chars() {
        app.key(KeyCode::Char(c)).unwrap();
    }
    assert_eq!(app.search_include, "*.txt");
    app.key(KeyCode::Tab).unwrap();
    assert!(matches!(app.prompt, Some(Prompt::SearchExclude)));
}

#[test]
fn fast_content_query_changes_coalesce_and_old_query_never_renders() {
    use crossterm::event::KeyCode;
    use std::time::Instant;
    let (_temp, mut app) = app();
    app.query_prompt(false); // Leave this request in flight, even if the worker finishes.
    for c in "missing".chars() {
        app.key(KeyCode::Char(c)).unwrap();
    }
    app.tick_search(Instant::now() + Duration::from_secs(1))
        .unwrap();
    for _ in 0..7 {
        app.key(KeyCode::Backspace).unwrap();
    }
    for c in "fixture".chars() {
        app.key(KeyCode::Char(c)).unwrap();
    }
    app.tick_search(Instant::now() + Duration::from_secs(1))
        .unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert!(
        app.jobs.results.try_recv().is_err(),
        "queries queued while an older one was in flight"
    );
    assert!(matches!(
        app.search_state,
        crate::loading::LoadState::Loading
    ));
    app.tick_search(Instant::now() + Duration::from_secs(1))
        .unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.apply(epoch, data);
    assert_eq!(app.search_rows.len(), 1);
    assert_eq!(app.search_rows[0].context, "fixture");
}
