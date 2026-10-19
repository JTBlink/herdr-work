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
    assert!(text.contains("EXPLORER") && text.contains("visible.txt"));
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
    app.apply(
        0,
        Ok(Data::Preview(
            0,
            old.clone(),
            Ok(herdr_work_code::preview::git_show(
                "old".into(),
                String::new(),
                "old content",
            )),
        )),
    );
    assert_eq!(app.text, "current content");
    app.apply(
        0,
        Ok(Data::Preview(0, old, Err(anyhow::anyhow!("old failure")))),
    );
    assert_eq!(app.text, "current content");
    app.apply(
        0,
        Ok(Data::Preview(
            0,
            current.clone(),
            Ok(herdr_work_code::preview::git_show(
                "current".into(),
                String::new(),
                "fresh content",
            )),
        )),
    );
    assert_eq!(app.text, "fresh content");
    app.apply(
        0,
        Ok(Data::Preview(
            0,
            current,
            Err(anyhow::anyhow!("unreadable")),
        )),
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

#[test]
fn compact_top_aligns_titles_and_uses_one_row_of_footer_without_hotkeys() {
    let (_temp, mut app) = loaded_app();
    app.preferences.show_hotkeys = false;
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    for dock_right in [false, true] {
        app.preferences.dock_right = dock_right;
        let panes = app.panes(area);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();
        let row = |y| {
            (0..80)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        };
        assert!(row(0).contains("EXPLORER") && row(0).contains("Preview"));
        assert!(
            !row(0).contains("HerdrWork")
                && !row(0).contains(&app.root.to_string_lossy().to_string())
        );
        assert_eq!(panes.content.y, 0);
        assert_eq!(panes.footer.height, 1);
        assert!(row(1).contains("visible.txt"));
        assert!(row(23).contains("repo"));
    }
}

#[test]
fn compact_scrolled_file_rows_use_the_rendered_body_on_both_dock_sides() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (_temp, mut app) = loaded_app();
    for n in 0..34 {
        std::fs::write(app.root.join(format!("f{n:02}.txt")), "selected file\n").unwrap();
    }
    app.refresh().unwrap();
    receive(&mut app);
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    for dock_right in [false, true] {
        app.preferences.dock_right = dock_right;
        app.selection = app
            .code
            .as_ref()
            .unwrap()
            .files
            .iter()
            .position(|f| f.name == "f33.txt")
            .unwrap();
        let panes = app.panes(area);
        let body = app.sidebar_block(String::new()).inner(panes.sidebar);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();
        let first = (body.x..body.right())
            .map(|x| terminal.backend().buffer()[(x, body.y)].symbol())
            .collect::<String>();
        assert!(first.contains("f13.txt"), "First displayed row was {first}");
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: body.x + 4,
                row: body.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        receive(&mut app);
        assert_eq!(
            app.code.as_ref().unwrap().files[app.selection].name,
            "f13.txt"
        );
        // With f13 selected the list starts at f00, making f20 the final visible row.
        terminal.draw(|f| app.draw(f)).unwrap();
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: body.x + 4,
                row: body.bottom() - 1,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        receive(&mut app);
        assert_eq!(
            app.code.as_ref().unwrap().files[app.selection].name,
            "f20.txt"
        );
    }
}

#[test]
fn compact_long_unicode_document_keeps_one_divider_and_status_when_editing() {
    let (_temp, mut app) = loaded_app();
    let file = format!("{}.txt", "很长的中文文件名".repeat(8));
    std::fs::write(app.root.join(&file), "文档正文\n第二行\n").unwrap();
    app.preview_path = Some(app.root.join(&file));
    app.text = "文档正文\n第二行\n".into();
    app.code.as_mut().unwrap().branch = "feature/layout".into();
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    for dock_right in [false, true] {
        app.preferences.dock_right = dock_right;
        let panes = app.panes(area);
        let divider = if dock_right {
            panes.sidebar.x
        } else {
            panes.sidebar.right() - 1
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();
        for y in 0..panes.footer.y {
            assert_eq!(terminal.backend().buffer()[(divider, y)].symbol(), "│");
        }
        let body = app.content_block(String::new()).inner(panes.content);
        assert_eq!(body.y, 1);
        assert_eq!(terminal.backend().buffer()[(body.x, body.y)].symbol(), "文");
        assert_eq!(body.bottom(), panes.footer.y);
    }
    app.editor = Some(app.desk.edit_file(&app.root, &file).unwrap());
    app.status = "编辑中".into();
    app.preferences.show_hotkeys = false;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| app.draw(f)).unwrap();
    let status = (0..80)
        .map(|x| terminal.backend().buffer()[(x, 23)].symbol())
        .collect::<String>();
    assert!(
        status.contains("repo")
            && status.contains("feature/layout")
            && status.replace(' ', "").contains("编辑中"),
        "Actual status: {status}"
    );
    assert!(!status.contains("Ctrl+S"));
    capture_fixture(&mut app, "compact-editor");
}

#[test]
fn compact_quick_open_uses_only_its_query_row_and_clicks_the_first_result() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (_temp, mut app) = loaded_app();
    app.switch_tab(Tab::Search);
    app.search_quick = true;
    app.search_state = LoadState::Ready;
    app.search_rows = vec![herdr_work_engine::SearchRow {
        path: app.root.join("visible.txt"),
        label: "visible.txt".into(),
        line: 1,
        context: "fixture".into(),
    }];
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    for dock_right in [false, true] {
        app.preferences.dock_right = dock_right;
        let (header, body) = app.search_regions(app.panes(area).sidebar);
        assert_eq!(header.height, 1);
        assert_eq!(body.y, 2);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();
        let first = (body.x..body.right())
            .map(|x| terminal.backend().buffer()[(x, body.y)].symbol())
            .collect::<String>();
        assert!(first.contains("visible.txt"));
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: body.x + 2,
                row: body.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        receive(&mut app);
        assert_eq!(app.text, "fixture\n");
    }
}

#[test]
fn compact_task_rows_keep_two_line_items_and_ignore_unused_last_row() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (_temp, mut app) = loaded_app();
    app.tasks = (0..20)
        .map(|n| Task {
            id: TaskId(format!("task-{n}")),
            title: format!("Task {n}"),
            stage: "planned".into(),
            executions: vec![],
        })
        .collect();
    app.tasks_state = LoadState::Ready;
    app.switch_tab(Tab::Tasks);
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    for dock_right in [false, true] {
        app.preferences.dock_right = dock_right;
        app.task = 19;
        let body = app
            .sidebar_block(String::new())
            .inner(app.panes(area).sidebar);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();
        let row = (body.x..body.right())
            .map(|x| terminal.backend().buffer()[(x, body.y)].symbol())
            .collect::<String>();
        assert!(row.contains("Task 10"));
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Right),
                column: body.x + 2,
                row: body.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        assert_eq!(app.task, 10);
        app.overlay = None;
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Right),
                column: body.x + 2,
                row: body.bottom() - 1,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        assert_eq!(app.task, 10);
        assert!(app.overlay.is_none(), "Blank row opened a task menu");
    }
}

#[test]
fn passive_refresh_updates_external_changes_without_resetting_reader() {
    let (_temp, mut app) = loaded_app();
    let original = (0..100).map(|n| format!("old {n}\n")).collect::<String>();
    std::fs::write(app.root.join("visible.txt"), original).unwrap();
    app.key(crossterm::event::KeyCode::Enter).unwrap();
    receive(&mut app);
    app.focus = Focus::Content;
    screen(&mut app);
    for _ in 0..8 {
        app.input_key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Down,
                crossterm::event::KeyModifiers::NONE,
            ),
            ratatui::layout::Rect::new(0, 0, 160, 32),
        )
        .unwrap();
    }
    std::fs::write(
        app.root.join("visible.txt"),
        "external change\n".repeat(100),
    )
    .unwrap();
    std::fs::write(app.root.join("new-agent-file.txt"), "new file\n").unwrap();
    app.tick_refresh(std::time::Instant::now() + Duration::from_secs(3))
        .unwrap();
    receive(&mut app);
    assert!(app.text.starts_with("external change"));
    assert_eq!(app.preview_scroll, 8);
    assert!(app.focus == Focus::Content);
    assert_eq!(
        app.code.as_ref().unwrap().files[app.selection].name,
        "visible.txt"
    );
    assert!(
        app.code
            .as_ref()
            .unwrap()
            .files
            .iter()
            .any(|f| f.name == "new-agent-file.txt")
    );
}

#[test]
fn periodic_refresh_coalesces_and_preserves_active_input() {
    let (_temp, mut app) = loaded_app();
    app.prompt = Some(Prompt::Search(false));
    app.input = "保留输入".into();
    let start = std::time::Instant::now();
    for n in 1..10 {
        app.tick_refresh(start + Duration::from_secs(n * 3))
            .unwrap();
    }
    receive(&mut app);
    assert_eq!(app.input, "保留输入");
    assert!(app.prompt.is_some());
    assert!(
        app.jobs.results.try_recv().is_err(),
        "Passive refresh flooded the worker queue"
    );
}

#[test]
fn scm_repository_inputs_survive_refresh_and_failed_commit_without_crossing_roots() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = app();
    let nested = app.root.join("inner");
    std::fs::create_dir(&nested).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .arg(&nested)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(nested.join("visible.txt"), "inner").unwrap();
    app.code = Some(app.desk.code_snapshot(&app.root, &[], 1).unwrap());
    app.code_state = LoadState::Ready;
    app.switch_tab(Tab::Scm);
    let rows = app.scm_rows();
    assert!(
        rows.iter()
            .any(|r| matches!(r.3, crate::scm::ScmTarget::Message(0)))
    );
    assert!(
        rows.iter()
            .any(|r| matches!(r.3, crate::scm::ScmTarget::Message(1)))
    );
    app.scm_cursor = rows
        .iter()
        .position(|r| matches!(r.3, crate::scm::ScmTarget::Message(1)))
        .unwrap();
    app.activate_scm_row().unwrap();
    app.input_key(
        KeyEvent::new(KeyCode::Char('中'), KeyModifiers::NONE),
        ratatui::layout::Rect::new(0, 0, 160, 32),
    )
    .unwrap();
    app.input_key(
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        ratatui::layout::Rect::new(0, 0, 160, 32),
    )
    .unwrap();
    app.flush_git_drafts().unwrap();
    receive(&mut app);
    let draft = app.desk.git_draft(&nested).unwrap();
    assert_eq!(draft.message.iter().collect::<String>(), "中");
    assert_eq!(draft.cursor, 0);
    assert!(app.desk.git_draft(&app.root).unwrap().message.is_empty());
    app.apply(
        0,
        Ok(Data::Code(
            app.generation,
            Ok(app.desk.code_snapshot(&app.root, &[], 2).unwrap()),
        )),
    );
    assert_eq!(
        app.git.drafts[&nested].message.iter().collect::<String>(),
        "中"
    );
    // Failing before a commit has staged changes must keep the inline input.
    app.input_key(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        ratatui::layout::Rect::new(0, 0, 160, 32),
    )
    .unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    app.apply(epoch, data);
    assert_eq!(
        app.git.drafts[&nested].message.iter().collect::<String>(),
        "中"
    );
}

#[test]
fn scm_folder_action_requires_its_snapshot_confirmation_and_never_stages_another_root() {
    use crate::scm::ScmTarget;
    use crossterm::event::KeyCode;
    let (_temp, mut app) = app();
    std::fs::create_dir(app.root.join("dir")).unwrap();
    std::fs::write(app.root.join("dir/new.txt"), "new").unwrap();
    app.code = Some(app.desk.code_snapshot(&app.root, &[], 1).unwrap());
    app.code_state = LoadState::Ready;
    app.switch_tab(Tab::Scm);
    app.action(KeyCode::Char('t')).unwrap();
    app.scm_cursor = app
        .scm_rows()
        .iter()
        .position(|r| matches!(&r.3, ScmTarget::Directory(_, false, path) if path == "dir"))
        .unwrap();
    app.action(KeyCode::Char('s')).unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    app.apply(epoch, data);
    assert!(matches!(app.prompt, Some(Prompt::WorkGit(_))));
    assert!(screen(&mut app).contains("StageUnder"));
    app.input = "wrong".into();
    assert!(app.submit_prompt().is_err());
    assert!(matches!(app.prompt, Some(Prompt::WorkGit(_))));
    app.input = "APPLY".into();
    app.submit_prompt().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    app.apply(epoch, data);
    let snapshot = app.desk.code_snapshot(&app.root, &[], 2).unwrap();
    assert!(
        snapshot
            .changes
            .iter()
            .any(|c| c.path == "dir/new.txt" && c.staged)
    );
    assert!(
        !snapshot
            .changes
            .iter()
            .any(|c| c.path == "visible.txt" && c.staged)
    );
}

#[test]
fn late_git_preparation_cannot_replace_a_more_recent_modal() {
    use herdr_work_engine::WorkGitAction;
    let (_temp, mut app) = app();
    let plan = app
        .desk
        .prepare_work_git(&app.root, WorkGitAction::StageAll)
        .unwrap();
    app.git.request = 2;
    app.prompt = Some(Prompt::Folder);
    app.input = "user input".into();
    app.apply(0, Ok(Data::WorkGitReady(1, Ok(plan))));
    assert!(matches!(app.prompt, Some(Prompt::Folder)));
    assert_eq!(app.input, "user input");
    let plan = app
        .desk
        .prepare_work_git(&app.root, WorkGitAction::StageAll)
        .unwrap();
    app.apply(0, Ok(Data::WorkGitReady(2, Ok(plan))));
    assert!(matches!(app.prompt, Some(Prompt::Folder)));
    assert_eq!(app.input, "user input");
}

#[test]
fn failed_commit_keeps_the_edited_message_and_repository_draft() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = app();
    for args in [
        ["config", "user.name", "Fixture"],
        ["config", "user.email", "fixture@example.invalid"],
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
    let snapshot = app.desk.code_snapshot(&app.root, &[], 1).unwrap();
    app.desk.stage(&snapshot.changes[0], true).unwrap();
    app.code = Some(app.desk.code_snapshot(&app.root, &[], 2).unwrap());
    app.switch_tab(Tab::Scm);
    app.scm_cursor = app
        .scm_rows()
        .iter()
        .position(|r| matches!(r.3, crate::scm::ScmTarget::Message(0)))
        .unwrap();
    app.activate_scm_row().unwrap();
    app.paste_commit("keep my 中文 message").unwrap();
    app.edit_commit_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    app.apply(epoch, data);
    assert!(matches!(app.prompt, Some(Prompt::Commit(_))));
    // Change HEAD/index after preparation: the commit must fail, preserving input.
    std::fs::write(app.root.join("other.txt"), "external index edit").unwrap();
    assert!(
        Command::new("git")
            .args(["add", "other.txt"])
            .current_dir(&app.root)
            .status()
            .unwrap()
            .success()
    );
    app.submit_prompt().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    app.apply(epoch, data);
    assert!(app.status.contains("草稿已保留"));
    assert_eq!(
        app.git.drafts[&app.root].message.iter().collect::<String>(),
        "keep my 中文 message"
    );
    app.flush_git_drafts().unwrap();
    receive(&mut app);
    assert_eq!(
        app.desk.git_draft(&app.root).unwrap(),
        app.git.drafts[&app.root]
    );
}

#[test]
fn unknown_git_operation_has_an_explicit_tui_evidence_flow_without_automatic_retry() {
    use crate::menus::{Command as MenuCommand, Overlay};
    use crossterm::event::KeyCode;
    let (_temp, mut app) = app();
    let plan = app
        .desk
        .prepare_work_git(
            &app.root,
            herdr_work_engine::WorkGitAction::CreateBranch("bad..name".into()),
        )
        .unwrap();
    assert!(app.desk.execute_work_git(&plan).is_err());
    app.switch_tab(Tab::Scm);
    app.open_menu(false);
    let Some(Overlay::Menu { rows, selected, .. }) = app.overlay.as_mut() else {
        panic!("missing Git menu")
    };
    *selected = rows
        .iter()
        .position(|(cmd, _)| matches!(cmd, MenuCommand::Recovery))
        .unwrap();
    app.overlay_key(KeyCode::Enter).unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    app.apply(epoch, data);
    assert!(matches!(app.overlay, Some(Overlay::Recovery(_))));
    app.overlay_key(KeyCode::Enter).unwrap();
    assert!(matches!(app.prompt, Some(Prompt::Resolve(_))));
    assert!(screen(&mut app).contains("当前HEAD"));
    app.input = "Inspected HEAD and refs: invalid name was never created".into();
    app.submit_prompt().unwrap();
    let (epoch, data) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    app.apply(epoch, data);
    assert!(app.status.contains("没有自动重试"));
    assert!(app.desk.unresolved_git_operations().unwrap().is_empty());
}

#[test]
fn staged_section_cannot_stage_unselected_changes_from_the_other_group() {
    use crossterm::event::KeyCode;
    let (_temp, mut app) = app();
    let snapshot = app.desk.code_snapshot(&app.root, &[], 1).unwrap();
    app.desk.stage(&snapshot.changes[0], true).unwrap();
    std::fs::write(app.root.join("other.txt"), "must stay unstaged").unwrap();
    app.code = Some(app.desk.code_snapshot(&app.root, &[], 2).unwrap());
    app.switch_tab(Tab::Scm);
    app.scm_cursor = app
        .scm_rows()
        .iter()
        .position(|r| matches!(r.3, crate::scm::ScmTarget::Section(_, true)))
        .unwrap();
    assert!(app.action(KeyCode::Char('s')).is_err());
    assert!(app.prompt.is_none());
}

#[test]
fn readonly_preview_wrap_selection_and_pinned_documents_share_source_model() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = loaded_app();
    let source = format!("{}\nsecond line", "alpha界".repeat(30));
    std::fs::write(app.root.join("visible.txt"), &source).unwrap();
    app.open_preview(app.root.join("visible.txt"), None)
        .unwrap();
    receive(&mut app);
    app.focus = Focus::Content;
    let area = ratatui::layout::Rect::new(0, 0, 90, 32);
    let mut terminal = Terminal::new(TestBackend::new(90, 32)).unwrap();
    terminal.draw(|f| app.draw(f)).unwrap();
    let wrapped = app.preview_doc.as_ref().unwrap().row_count();
    app.input_key(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        area,
    )
    .unwrap();
    assert_eq!(
        app.preview_doc
            .as_ref()
            .unwrap()
            .selection_text()
            .as_deref(),
        Some(source.as_str())
    );
    app.input_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), area)
        .unwrap();
    terminal.draw(|f| app.draw(f)).unwrap();
    assert!(app.preview_doc.as_ref().unwrap().row_count() < wrapped);
    app.input_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE), area)
        .unwrap();
    std::fs::write(app.root.join("second.txt"), "another document").unwrap();
    app.open_preview(app.root.join("second.txt"), None).unwrap();
    receive(&mut app);
    app.input_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE), area)
        .unwrap();
    assert!(app.preview_path.as_ref().unwrap().ends_with("visible.txt"));
    assert!(!app.preview_doc.as_ref().unwrap().wrap());
    assert!(screen(&mut app).contains("visible.txt"));
}

#[test]
fn repeated_preview_requests_coalesce_and_same_path_old_load_cannot_win() {
    let (_temp, mut app) = loaded_app();
    let path = app.root.join("visible.txt");
    app.open_preview(path.clone(), None).unwrap();
    for _ in 0..20 {
        app.open_preview(path.clone(), Some(1)).unwrap();
    }
    let (epoch, old) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    assert!(
        app.jobs.results.try_recv().is_err(),
        "obsolete preview requests filled the worker queue"
    );
    std::fs::write(&path, "latest content").unwrap();
    app.apply(epoch, old);
    receive(&mut app);
    assert_eq!(app.text, "latest content");
    assert!(app.jobs.results.try_recv().is_err());
}

#[test]
fn compact_scm_repository_input_click_matches_the_scrolled_rendered_row() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    for dock_right in [false, true] {
        let (_temp, mut app) = app();
        let nested = app.root.join("inner");
        std::fs::create_dir(&nested).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .arg(&nested)
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(nested.join("own.txt"), "nested").unwrap();
        app.code = Some(app.desk.code_snapshot(&app.root, &[], 1).unwrap());
        app.code_state = LoadState::Ready;
        app.preferences.dock_right = dock_right;
        app.preferences.show_hotkeys = false;
        app.switch_tab(Tab::Scm);
        app.ensure_draft(&nested).unwrap();
        app.git.drafts.get_mut(&nested).unwrap().message = "inner-draft".chars().collect();
        app.scm_cursor = app
            .scm_rows()
            .iter()
            .position(|r| matches!(r.3, crate::scm::ScmTarget::Message(1)))
            .unwrap();
        let area = ratatui::layout::Rect::new(0, 0, 100, 10);
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal.draw(|f| app.draw(f)).unwrap();
        let row = (0..area.height)
            .find(|y| {
                (0..area.width)
                    .map(|x| terminal.backend().buffer()[(x, *y)].symbol())
                    .collect::<String>()
                    .contains("inner-draft")
            })
            .expect("selected repository input must be visible in a short panel");
        let body = app
            .sidebar_block(String::new())
            .inner(app.panes(area).sidebar);
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: body.x + 2,
                row,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        assert_eq!(app.git.editing, Some(nested.clone()));
        assert_eq!(app.git.active, Some(nested));
    }
}

#[test]
fn clicking_readonly_content_exits_search_input_and_uses_preview_keys() {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    let (_temp, mut app) = loaded_app();
    app.open_preview(app.root.join("visible.txt"), None)
        .unwrap();
    receive(&mut app);
    app.query_prompt(false);
    receive(&mut app);
    let area = ratatui::layout::Rect::new(0, 0, 160, 32);
    let body = app.preview_body(app.panes(area).content);
    app.mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: body.x + 4,
            row: body.y,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    app.input_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), area)
        .unwrap();
    assert!(app.prompt.is_none());
    assert!(app.search_query.is_empty());
    assert!(!app.preview_doc.as_ref().unwrap().wrap());
    assert!(screen(&mut app).contains("fixture"));
}

#[test]
fn commit_draft_edits_coalesce_in_the_worker_and_late_save_keeps_new_input() {
    let (_temp, mut app) = loaded_app();
    app.switch_tab(Tab::Scm);
    app.scm_cursor = app
        .scm_rows()
        .iter()
        .position(|r| matches!(r.3, crate::scm::ScmTarget::Message(0)))
        .unwrap();
    app.activate_scm_row().unwrap();
    app.paste_commit("first").unwrap();
    assert!(
        app.desk.git_draft(&app.root).unwrap().message.is_empty(),
        "input events must not write drafts to disk"
    );
    let future = std::time::Instant::now() + Duration::from_secs(1);
    app.tick_git_drafts(future).unwrap();
    app.paste_commit("-new").unwrap();
    receive(&mut app);
    assert_eq!(
        app.git.drafts[&app.root].message.iter().collect::<String>(),
        "first-new"
    );
    app.flush_git_drafts().unwrap();
    let saved = app.desk.git_draft(&app.root).unwrap();
    assert_eq!(saved.message.iter().collect::<String>(), "first-new");
    assert_eq!(saved.cursor, "first-new".chars().count());
}

#[test]
fn scm_hover_button_uses_the_visible_file_and_is_disabled_when_too_narrow() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (_temp, mut app) = loaded_app();
    app.switch_tab(Tab::Scm);
    let area = ratatui::layout::Rect::new(0, 0, 160, 32);
    let body = app
        .sidebar_block(String::new())
        .inner(app.panes(area).sidebar);
    let index = app.scm_rows().iter().position(|r| r.0 == Some(0)).unwrap();
    app.scm_cursor = index;
    let x = body.right() - 4;
    let y = body.y + index as u16;
    let mouse = |kind, column, row| MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    app.mouse(mouse(MouseEventKind::Moved, x, y), area).unwrap();
    assert!(
        screen(&mut app).contains("↶"),
        "hover actions must be drawn before they can be clicked"
    );
    app.mouse(mouse(MouseEventKind::Down(MouseButton::Left), x, y), area)
        .unwrap();
    receive(&mut app);
    assert!(
        app.desk
            .code_snapshot(&app.root, &[], 2)
            .unwrap()
            .changes
            .iter()
            .any(|c| c.path == "visible.txt" && c.staged)
    );
    // A second file in a narrow sidebar must only open the row, never hit an invisible +.
    std::fs::write(app.root.join("second.txt"), "new").unwrap();
    app.code = Some(app.desk.code_snapshot(&app.root, &[], 3).unwrap());
    app.preferences.sidebar_width = 12;
    let body = app
        .sidebar_block(String::new())
        .inner(app.panes(area).sidebar);
    let selected = app
        .code
        .as_ref()
        .unwrap()
        .changes
        .iter()
        .position(|c| c.path == "second.txt")
        .unwrap();
    app.scm_cursor = app
        .scm_rows()
        .iter()
        .position(|r| r.0 == Some(selected))
        .unwrap();
    let y = body.y + app.scm_cursor as u16;
    app.mouse(mouse(MouseEventKind::Moved, body.right() - 4, y), area)
        .unwrap();
    app.mouse(
        mouse(MouseEventKind::Down(MouseButton::Left), body.right() - 4, y),
        area,
    )
    .unwrap();
    assert!(
        !app.desk
            .code_snapshot(&app.root, &[], 4)
            .unwrap()
            .changes
            .iter()
            .any(|c| c.path == "second.txt" && c.staged)
    );
}

#[test]
fn explorer_title_buttons_follow_visible_geometry_on_both_sides_and_hide_when_narrow() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    for dock_right in [false, true] {
        let (_temp, mut app) = loaded_app();
        let folder = app.root.join("folder");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("child.txt"), "child").unwrap();
        app.expanded.push(folder.clone());
        app.code = Some(app.desk.code_snapshot(&app.root, &app.expanded, 1).unwrap());
        app.preferences.dock_right = dock_right;
        let area = ratatui::layout::Rect::new(0, 0, 120, 24);
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert!(!(0..area.width).any(|x| terminal.backend().buffer()[(x, 0)].symbol() == "⊟"));
        let sidebar = app.panes(area).sidebar;
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Moved,
                column: sidebar.x + 3,
                row: sidebar.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let x = (sidebar.x..sidebar.right())
            .find(|x| terminal.backend().buffer()[(*x, 0)].symbol() == "⊟")
            .expect("hover must render the actual collapse control");
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        assert!(app.expanded.is_empty());
        assert!(app.overlay.is_none());
        app.expanded.push(folder);
        app.preferences.sidebar_width = 12;
        let sidebar = app.panes(area).sidebar;
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Moved,
                column: sidebar.x + 3,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert!(
            !(sidebar.x..sidebar.right())
                .any(|x| terminal.backend().buffer()[(x, 0)].symbol() == "⊟")
        );
        app.mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: sidebar.x + 3,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        assert!(
            !app.expanded.is_empty(),
            "hidden title controls must not receive clicks"
        );
    }
}

#[test]
fn refreshed_disk_draft_never_replaces_text_typed_while_the_worker_was_loading() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_temp, mut app) = app();
    app.desk
        .save_git_draft(
            &app.root,
            &herdr_work_engine::CommitDraft {
                message: "旧稿".chars().collect(),
                cursor: 1,
            },
        )
        .unwrap();
    app.refresh().unwrap();
    receive(&mut app);
    app.switch_tab(Tab::Scm);
    app.scm_cursor = app
        .scm_rows()
        .iter()
        .position(|row| matches!(row.3, crate::scm::ScmTarget::Message(0)))
        .unwrap();
    app.activate_scm_row().unwrap();
    assert_eq!(app.git.drafts[&app.root].cursor, 1);
    app.refresh().unwrap();
    let (epoch, result) = app
        .jobs
        .results
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    app.edit_commit_key(KeyEvent::new(KeyCode::Char('新'), KeyModifiers::NONE))
        .unwrap();
    app.apply(epoch, result);
    assert_eq!(
        app.git.drafts[&app.root].message.iter().collect::<String>(),
        "旧新稿"
    );
    assert_eq!(app.git.drafts[&app.root].cursor, 2);
}

#[test]
fn exit_flush_waits_for_a_full_worker_queue_and_saves_the_last_draft() {
    let (_temp, mut app) = loaded_app();
    app.switch_tab(Tab::Scm);
    app.scm_cursor = app
        .scm_rows()
        .iter()
        .position(|row| matches!(row.3, crate::scm::ScmTarget::Message(0)))
        .unwrap();
    app.activate_scm_row().unwrap();
    app.paste_commit("last unsaved input").unwrap();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    app.job(move |_| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(Data::GitDraftFlush)
    })
    .unwrap();
    started_rx.recv().unwrap();
    for _ in 0..32 {
        app.job(|_| Ok(Data::GitDraftFlush)).unwrap();
    }
    app.tick_git_drafts(std::time::Instant::now() + Duration::from_secs(1))
        .unwrap();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        release_tx.send(()).unwrap();
    });
    app.flush_git_drafts().unwrap();
    assert_eq!(
        app.desk
            .git_draft(&app.root)
            .unwrap()
            .message
            .iter()
            .collect::<String>(),
        "last unsaved input"
    );
}
