//! One terminal owner, with background jobs and upstream-backed views.
use anyhow::Result;
use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
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
mod activation;
mod drafts;

mod editing;
mod footer;
mod hover;
mod input;
mod interaction;
mod layout;
mod loading;
mod menus;
mod mouse;
mod persistence;
mod preferences;
mod preview;
mod refresh;
mod scm;
mod search;
mod sidebar_view;
mod state;
mod task_view;
mod timers;
mod view;
pub use herdr_work_engine::InitialView;
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
    run_with_initial(desk, root, None)
}

pub fn run_with_initial(
    desk: HerdrWork,
    root: PathBuf,
    initial: Option<InitialView>,
) -> Result<()> {
    let root = root.canonicalize()?;
    let mut context = desk.load_context()?;
    let root = if initial.is_some() {
        if context
            .execution
            .as_ref()
            .is_some_and(|execution| execution.worktree != root)
        {
            context.epoch = context.epoch.wrapping_add(1);
            context.execution = None;
        }
        root
    } else {
        context
            .execution
            .as_ref()
            .map(|e| e.worktree.clone())
            .unwrap_or(root)
    };
    let mut app = App::new(desk, root, context)?;
    if let Some(view) = initial {
        app.activate_initial_view(view);
    }
    app.start_activation()?;
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
            if tx.send(Message::Result(epoch, Box::new(result))).is_err() {
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
                Some(Message::Input(event)) => {
                    let size = terminal.size()?;
                    if app.input_event(
                        event,
                        ratatui::layout::Rect::new(0, 0, size.width, size.height),
                    ) {
                        break;
                    }
                }
                Some(Message::Result(epoch, data)) => app.apply(epoch, *data),
                _ => {}
            }
            let now = Instant::now();
            if now.duration_since(last_tick) >= Duration::from_millis(100) {
                redraw |= app.tick_background(now, &mut last_editor_poll);
                last_tick = now;
            }
        }
        Ok(())
    })();
    // Lifecycle persistence is completed before releasing the exclusive profile.
    let saved = app.persist_workspace_on_exit();
    stop.store(true, Ordering::Relaxed);
    let _ = input.join();
    let preferences_flush = app.flush_preferences();
    let draft_flush = app.flush_git_drafts();
    result.and(saved).and(preferences_flush).and(draft_flush)
}
