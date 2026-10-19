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
    let app = App::new(desk, root, Context::default());
    (temp, app)
}
fn screen(app: &App) -> String {
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
    let rendered = screen(&app);
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
        2,
        "Header plus one task list expected"
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
    let rendered = screen(&app);
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
    let rendered = screen(&app);
    assert!(rendered.contains("EXPLORER"));
    assert!(rendered.contains("visible.txt"));
    assert!(rendered.contains("Preview"));
    assert!(
        !rendered.contains("暂无任务"),
        "Tasks still occupies the default sidebar"
    );
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
            row: panes.activity.y + 5,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert!(app.tab == Tab::Scm);
    let rendered = screen(&app);
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
    assert!(screen(&app).contains("actualchangedcontent"));
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
}
