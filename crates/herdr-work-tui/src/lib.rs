//! One terminal owner, with background jobs and upstream-backed views.
use anyhow::Result;
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyEventKind,
    },
    execute,
};
use herdr_work_engine::HerdrWork;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

mod actions;
mod editing;
mod layout;
mod loading;
mod menus;
mod mouse;
mod preview;
mod scm;
mod search;
mod sidebar_view;
mod state;
mod task_view;
mod view;
use state::*;

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            std::io::stdout(),
            DisableMouseCapture,
            DisableBracketedPaste
        );
        ratatui::restore();
    }
}

pub fn run(desk: HerdrWork, root: PathBuf) -> Result<()> {
    let root = root.canonicalize()?;
    let context = desk.load_context()?;
    let root = context
        .execution
        .as_ref()
        .map(|e| e.worktree.clone())
        .unwrap_or(root);
    let mut app = App::new(desk, root, context)?;
    herdr_work_code::ui::set_color_theme(app.color_theme());
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;
    execute!(std::io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
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
    app.refresh()?;
    app.load_tasks()?;
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|frame| app.draw(frame))?;
            match rx.recv()? {
                Message::Input(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    let size = terminal.size()?;
                    if app.input_key(
                        key,
                        ratatui::layout::Rect::new(0, 0, size.width, size.height),
                    )? {
                        break;
                    }
                }
                Message::Result(epoch, data) => app.apply(epoch, data),
                Message::Input(Event::Paste(text)) if app.overlay.is_none() => {
                    if app.prompt.is_some() {
                        if app.input.len() + text.len() <= 1024 * 1024 {
                            app.input.push_str(&text);
                        } else {
                            app.status = "粘贴内容过长".into();
                        }
                    } else if let Some(session) = &mut app.editor {
                        if text.len() + session.editor.text().len() <= 1024 * 1024 {
                            session.editor.insert_text(&text);
                        } else {
                            app.status = "粘贴后文本超过 1 MiB".into();
                        }
                    }
                }
                Message::Input(Event::Mouse(mouse)) => {
                    let area = terminal.size()?;
                    if let Err(error) = app.mouse(
                        mouse,
                        ratatui::layout::Rect::new(0, 0, area.width, area.height),
                    ) {
                        app.status = error.to_string();
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let _ = input.join();
    result
}
