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
    time::{Duration, Instant},
};

mod actions;
mod drafts;
mod editing;
mod hover;
mod layout;
mod loading;
mod menus;
mod mouse;
mod preview;
mod refresh;
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
    let result = (|| -> Result<()> {
        app.refresh()?;
        app.load_tasks()?;
        let mut last_tick = Instant::now();
        let mut last_editor_poll = Instant::now();
        let mut redraw = true;
        loop {
            if redraw {
                terminal.draw(|frame| app.draw(frame))?;
            }
            let message = match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(message) => Some(message),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(error) => return Err(error.into()),
            };
            // Timers enqueue background reads; their results wake the loop and
            // redraw. Idle debounce ticks must not repaint a large document.
            redraw = message.is_some();
            match message {
                Some(Message::Input(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    let size = terminal.size()?;
                    if app.input_key(
                        key,
                        ratatui::layout::Rect::new(0, 0, size.width, size.height),
                    )? {
                        break;
                    }
                }
                Some(Message::Result(epoch, data)) => app.apply(epoch, data),
                Some(Message::Input(Event::Paste(text))) if app.overlay.is_none() => {
                    if app.prompt.is_some() {
                        if app.input.len() + text.len() <= 1024 * 1024 {
                            app.input.push_str(&text);
                            if let Err(error) = app.persist_commit_prompt() {
                                app.status = error.to_string();
                            }
                            app.search_input_changed()?;
                        } else {
                            app.status = "粘贴内容过长".into();
                        }
                    } else if app.git.editing.is_some() {
                        if let Err(error) = app.paste_commit(&text) {
                            app.status = error.to_string();
                        }
                    } else if let Some(session) = &mut app.editor {
                        if text.len() + session.editor.text().len() <= 1024 * 1024 {
                            session.editor.insert_text(&text);
                        } else {
                            app.status = "粘贴后文本超过 1 MiB".into();
                        }
                    }
                }
                Some(Message::Input(Event::Mouse(mouse))) => {
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
            let now = Instant::now();
            if now.duration_since(last_tick) >= Duration::from_millis(100) {
                app.tick_search(now)?;
                app.tick_git_drafts(now)?;
                redraw |= app.tick_preview();
                redraw |= app.tick_hover();
                app.tick_refresh(now)?;
                if now.duration_since(last_editor_poll) >= Duration::from_secs(2) {
                    app.poll_editor()?;
                    last_editor_poll = now;
                }
                last_tick = now;
            }
        }
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let _ = input.join();
    let draft_flush = app.flush_git_drafts();
    result.and(draft_flush)
}
