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
    let desk = Workdesk::open(
        &temp.path().join("profile"),
        Some(temp.path().join("missing-backend")),
    )
    .unwrap();
    let app = App {
        desk,
        root,
        context: Context::default(),
        tasks: vec![],
        task: 0,
        tab: Tab::Tasks,
        selection: 0,
        code: None,
        diff: None,
        review: None,
        text: String::new(),
        status: String::new(),
        tasks_state: LoadState::Loading,
        code_state: LoadState::Loading,
        expanded: vec![],
        generation: 0,
        prompt: None,
        input: String::new(),
        jobs: JobQueue::default(),
    };
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
        rendered.contains("import"),
        "Existing Projects users need import guidance"
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
