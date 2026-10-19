//! One terminal owner, with background jobs and upstream-backed views.
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use workdesk_engine::{JobQueue, Workdesk};

mod actions;
mod loading;
mod state;
mod task_view;
mod view;
use state::*;

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

pub fn run(desk: Workdesk, root: PathBuf) -> Result<()> {
    let root = root.canonicalize()?;
    let context = desk.load_context()?;
    let root = context
        .execution
        .as_ref()
        .map(|e| e.worktree.clone())
        .unwrap_or(root);
    let mut app = App {
        desk,
        context,
        tasks: vec![],
        task: 0,
        tab: Tab::Tasks,
        selection: 0,
        root,
        code: None,
        diff: None,
        review: None,
        text: String::new(),
        status: String::new(),
        tasks_state: loading::LoadState::Loading,
        code_state: loading::LoadState::Loading,
        expanded: vec![],
        generation: 0,
        prompt: None,
        input: String::new(),
        jobs: JobQueue::default(),
    };
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let input_tx = tx.clone();
    let stopping = stop.clone();
    let input = std::thread::spawn(move || {
        while !stopping.load(Ordering::Relaxed) {
            if crossterm::event::poll(Duration::from_millis(100)).unwrap_or(false) {
                match crossterm::event::read() {
                    Ok(e) => {
                        if input_tx.send(Message::Input(e)).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });
    let (replace_tx, empty_rx) = mpsc::channel();
    drop(replace_tx);
    let results = std::mem::replace(&mut app.jobs.results, empty_rx);
    std::thread::spawn(move || {
        while let Ok((epoch, result)) = results.recv() {
            if tx.send(Message::Result(epoch, result)).is_err() {
                break;
            }
        }
    });
    app.load_tasks()?;
    app.refresh()?;
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|frame| app.draw(frame))?;
            match rx.recv()? {
                Message::Input(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    if app.key(key.code)? {
                        break;
                    }
                }
                Message::Result(epoch, data) => app.apply(epoch, data),
                _ => {}
            }
        }
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let _ = input.join();
    result
}
