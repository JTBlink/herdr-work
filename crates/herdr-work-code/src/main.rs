//! herdr-work-code — the VS Code sidebar for herdr: file explorer and source
//! control in ONE binary. In unified mode both views share a pane and the
//! activity bar switches between them IN PROCESS (instant, no flash); in
//! separated mode the same binary runs one pane per view, pinned with
//! `--view explorer|git`. `--preview <ctl>` runs the file-preview pane.
//!
//! The native `--ensure` / `--toggle*` modes drive pane lifecycle; the other
//! `--*` stdin→stdout helpers expose the unit-tested launch calculations.

mod explorer_app;
mod scm_app;

use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use herdr_work_code::{ensure, launch, settings_view, state, viewer};
use state::{Exit, View};

/// How often the source-control view re-reads `git status` while idle.
const REFRESH_EVERY: Duration = Duration::from_millis(1500);
const ANIMATION_FRAME: Duration = Duration::from_millis(120);

fn main() -> std::io::Result<()> {
    let mode = std::env::args().nth(1);
    match mode.as_deref() {
        Some("--ensure") => return ensure::run(ensure::Mode::Ensure),
        Some("--toggle") => {
            return ensure::run(ensure::Mode::Toggle(View::Explorer));
        }
        Some("--toggle-git") => {
            return ensure::run(ensure::Mode::Toggle(View::SourceControl));
        }
        Some("--show-explorer") => {
            return ensure::run(ensure::Mode::Activate(ensure::Target::Explorer));
        }
        Some("--show-search") => {
            return ensure::run(ensure::Mode::Activate(ensure::Target::Search));
        }
        Some("--show-git") => {
            return ensure::run(ensure::Mode::Activate(ensure::Target::SourceControl));
        }
        Some("--quick-open") => {
            return ensure::run(ensure::Mode::Activate(ensure::Target::QuickOpen));
        }
        Some("--update-latest") => {
            return herdr_work_code::updates::run().map_err(std::io::Error::other);
        }
        Some("--refresh-sidebars") => return herdr_work_code::updates::refresh(),
        Some("--run-custom-editor") => return herdr_work_code::actions::run_configured_editor(),
        Some("--launch-decision") => {
            // Optional second arg picks the source-control decision; default
            // is the explorer/sidebar decision.
            // Optional THIRD arg scopes the decision to a tab or workspace —
            // it must match the scope the hook docks into, or the decision
            // answers for one tab while the dock lands in another.
            let now = state::unix_now();
            let scope = std::env::args().nth(3).unwrap_or_default();
            let mut out = if std::env::args().nth(2).as_deref() == Some("git") {
                launch::launch_decision_git(&read_stdin()?, now)
            } else {
                launch::launch_decision_in(&read_stdin()?, now, &scope)
            };
            // Strict toggle (⚙ Settings): report an open-but-unfocused pane
            // as CLOSE so the toggle launchers close it instead of focusing
            // it first. Safe for the ensure hook, which ignores FOCUS and
            // CLOSE alike (it only acts on OPEN and REPLACE).
            if state::load_state().strict_toggle {
                out = launch::focus_as_close(&out);
            }
            println!("{out}");
            return Ok(());
        }
        Some("--focused-pane") => {
            // Optional scope (tab or workspace id) confines the lookup to the
            // tab being docked; without it the globally focused pane wins and
            // a new tab gets rooted in whatever project was last focused.
            let scope = std::env::args().nth(2).unwrap_or_default();
            println!("{}", launch::focused_pane_in(&read_stdin()?, &scope));
            return Ok(());
        }
        Some("--pane-has-token") => {
            let pane_id = std::env::args().nth(2).unwrap_or_default();
            let present = launch::pane_has_token(&read_stdin()?, &pane_id);
            println!("{}", if present { "yes" } else { "no" });
            return Ok(());
        }
        Some("--event-scope") => {
            let payload = std::env::var("HERDR_PLUGIN_EVENT_JSON").unwrap_or_default();
            println!("{}", launch::event_scope_in(&payload, &read_stdin()?));
            return Ok(());
        }
        Some("--open-plan") => {
            let state = state::load_state();
            println!(
                "{}",
                launch::open_plan(&read_stdin()?, state.dock_right, state.sidebar_width)
            );
            return Ok(());
        }
        Some("--event-kind") => {
            // Which event ran the ensure hook, so it can treat a brand-new
            // space differently from an ordinary focus. Empty when herdr
            // supplies no payload (e.g. a manual invocation).
            let payload = std::env::var("HERDR_PLUGIN_EVENT_JSON").unwrap_or_default();
            println!("{}", launch::event_kind(&payload));
            return Ok(());
        }
        Some("--focused-tab") => {
            println!("{}", launch::focused_tab(&read_stdin()?));
            return Ok(());
        }
        Some("--auto-open") => {
            // For the unix ensure hook: skip auto-docking when the user
            // turned "Auto-open sidebar" off in ⚙ Settings (issue #8).
            println!(
                "{}",
                if state::load_state().auto_open {
                    "on"
                } else {
                    "off"
                }
            );
            return Ok(());
        }
        Some("--focus-on-open") => {
            // For the unix toggle launchers: skip the open-then-focus zoom
            // cycle when the user turned "Focus on open" off in ⚙ Settings,
            // so the sidebar docks in the background.
            println!(
                "{}",
                if state::load_state().focus_on_open {
                    "on"
                } else {
                    "off"
                }
            );
            return Ok(());
        }
        Some("--dock-right") => {
            println!(
                "{}",
                if state::load_state().dock_right {
                    "right"
                } else {
                    "left"
                }
            );
            return Ok(());
        }
        Some("--preview") => {
            let Some(control) = std::env::args()
                .nth(2)
                .or_else(|| std::env::var(state::PREVIEW_CONTROL_ENV).ok())
            else {
                eprintln!(
                    "herdr-work-code: --preview needs {}",
                    state::PREVIEW_CONTROL_ENV
                );
                std::process::exit(2);
            };
            // The viewer is its own process: it must apply the persisted
            // color theme itself, or a preview pane keeps the default palette
            // (and a dark syntax theme) whatever the user chose.
            herdr_work_code::ui::set_color_theme(state::load_state().color_theme);
            return viewer::run(std::path::Path::new(&control));
        }
        Some("--view") => {}
        Some(other) => {
            eprintln!("herdr-work-code: unknown argument `{other}`");
            eprintln!(
                "usage: herdr-work-code [--view explorer|git|--preview [ctl]|--run-custom-editor|--ensure|--toggle|--toggle-git|--show-explorer|--show-search|--show-git|--quick-open|--launch-decision [git]|--focused-pane|--pane-has-token <id>|--open-plan|--focused-tab|--auto-open|--focus-on-open|--dock-right]"
            );
            std::process::exit(2);
        }
        None => {}
    }

    // Starting view: an explicit `--view` pin (separated panes), else the
    // last-active view when the unified sidebar is on.
    let pinned = if mode.as_deref() == Some("--view") {
        std::env::args()
            .nth(2)
            .as_deref()
            .and_then(View::from_view_flag)
    } else {
        None
    };
    let persisted = state::load_state();
    let initial_activity = std::env::var(state::INITIAL_ACTIVITY_ENV)
        .ok()
        .and_then(|value| ensure::Target::from_env_value(&value));
    herdr_work_code::ui::set_color_theme(persisted.color_theme);
    let mut view = initial_activity.map_or_else(
        || {
            pinned.unwrap_or(if persisted.merged {
                persisted.active
            } else {
                View::Explorer
            })
        },
        ensure::Target::initial_view,
    );

    // Unix launchers use plugin.pane.open so Herdr starts this argv directly,
    // with no shell prompt between the split and the TUI. Keep the host's cwd
    // at the plugin root for relative-command resolution, then adopt the
    // requested project cwd inside the process.
    if let Some(cwd) = std::env::var_os(state::SPAWN_CWD_ENV).filter(|cwd| !cwd.is_empty()) {
        std::env::set_current_dir(cwd)?;
    }
    // Mark the short interval before App::new applies its live identity. The
    // launcher also writes a live stamp after plugin.pane.open returns, so any
    // ordering between the two ends with App::new clearing this marker before
    // the TUI can accept edits.
    if let Some(pane_id) = std::env::var_os("HERDR_PANE_ID").filter(|id| !id.is_empty()) {
        let _ = herdr_work_code::ipc::report_starting_identity(
            &pane_id.to_string_lossy(),
            view,
            view == View::Explorer && persisted.merged,
        );
    }

    // ONE terminal session for every view: switching drops the old view's
    // state and draws the other in the same alternate screen — instant, and
    // the shell prompt underneath never flashes through.
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::Purge),
        crossterm::cursor::MoveTo(0, 0),
    );
    // A TUI's colors are interface, not pipeable output: ignore NO_COLOR,
    // which otherwise leaks in whenever the herdr server was (re)started
    // from an agent shell (Claude Code's tool env sets it) and silently
    // turns every pane we draw monochrome.
    crossterm::style::force_color_output(true);
    let mut terminal = ratatui::init();
    herdr_work_code::preview::initialize_terminal_images();
    let _ = crossterm::execute!(std::io::stdout(), EnableMouseCapture);
    // First run on a machine without a Nerd Font: offer to install one
    // before any icons render. The prompt stamps the pane's identity token
    // itself (the app loops haven't started yet, and a token-less pane gets
    // REPLACE-killed by the corpse rule while the user reads the prompt).
    herdr_work_code::fontsetup::maybe_prompt(&mut terminal, view, persisted.merged)?;
    let cwd_follower = Rc::new(RefCell::new(launch::CwdFollower::default()));
    let workspace_label = workspace_label();
    let spawn_cwd = std::env::current_dir()?;
    let root_key = remembered_root_key(&workspace_label, &spawn_cwd);
    // Resolved ONCE per process. A view switch rebuilds the app but keeps
    // this root: re-reading roots.json there let another workspace's entry
    // (see `RootMemory`) silently re-root a healthy sidebar.
    let mut roots = RootMemory {
        root: resolve_root(&root_key, &workspace_label, &spawn_cwd)?,
        startup_label: workspace_label,
        spawn_cwd,
        pending: false,
        last_attempt: None,
    };
    // Some(focus_query) opens the Search view on the next Explorer render;
    // None doesn't. A resumed search restores unfocused (a switch, not a find).
    let mut search_on_open: Option<bool> = if initial_activity == Some(ensure::Target::Search) {
        Some(false)
    } else {
        (pinned.is_none()
            && persisted.merged
            && persisted.active == View::Explorer
            && persisted.search_active)
            .then_some(false)
    };
    let mut quick_open_on_open = initial_activity == Some(ensure::Target::QuickOpen);
    let result = loop {
        let exit = match view {
            View::Explorer => run_explorer(
                &mut terminal,
                Rc::clone(&cwd_follower),
                &mut roots,
                std::mem::take(&mut search_on_open),
                std::mem::take(&mut quick_open_on_open),
            ),
            View::SourceControl => run_scm(&mut terminal, Rc::clone(&cwd_follower), &mut roots),
        };
        match exit {
            Ok(Exit::Quit) => {
                roots.flush();
                publish_pending(&roots);
                break Ok(());
            }
            Ok(Exit::Projects) => {
                eprintln!("Projects 入口请使用 herdr-work ui --sidebar");
            }
            Ok(Exit::Switch) => {
                view = view.other();
            }
            Ok(Exit::Search { focus_query }) => {
                view = View::Explorer;
                search_on_open = Some(focus_query);
            }
            Ok(Exit::QuickOpen) => {
                view = View::Explorer;
                quick_open_on_open = true;
            }
            Err(e) => {
                roots.flush();
                break Err(e);
            }
        }
    };
    let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn read_stdin() -> std::io::Result<String> {
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

pub use herdr_work_code::root_memory::*;

/// The explorer's event loop: short poll so the liveness heartbeat keeps
/// stamping even while idle.
fn run_explorer(
    terminal: &mut ratatui::DefaultTerminal,
    cwd_follower: Rc<RefCell<launch::CwdFollower>>,
    roots: &mut RootMemory,
    search_on_open: Option<bool>,
    quick_open_on_open: bool,
) -> std::io::Result<Exit> {
    let mut app = explorer_app::App::new(roots.root.clone(), cwd_follower);
    if let Some(focus_query) = search_on_open {
        app.open_content_search(focus_query);
    }
    if quick_open_on_open {
        app.open_quick_open();
    }
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        // 500ms: quick enough that a finished folder pick lands promptly,
        // still cheap for the heartbeat.
        let timeout = if app.is_syncing() {
            ANIMATION_FRAME
        } else {
            Duration::from_millis(500)
        };
        if event::poll(timeout)? {
            let exit = match event::read()? {
                Event::Key(key) => app.on_key(key),
                Event::Mouse(mouse) => app.on_mouse(mouse),
                Event::Resize(width, _) => {
                    app.on_resize(width);
                    None
                }
                _ => None, // resize, focus, … simply fall through to a redraw
            };
            if let Some(exit) = exit {
                if exit == Exit::Quit {
                    app.clear_identity();
                }
                return Ok(exit);
            }
        }
        app.heartbeat();
        app.poll_picker();
        app.tick();
        roots.record(&app.root_path());
    }
}

/// The source-control view's event loop: poll + tick so external changes and
/// finished background work (✧ suggestions, syncs) show up on their own.
fn run_scm(
    terminal: &mut ratatui::DefaultTerminal,
    cwd_follower: Rc<RefCell<launch::CwdFollower>>,
    roots: &mut RootMemory,
) -> std::io::Result<Exit> {
    let mut app = scm_app::App::new(roots.root.clone(), cwd_follower);
    let mut last_tick = std::time::Instant::now();
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        let mut timeout = REFRESH_EVERY.saturating_sub(last_tick.elapsed());
        if app.is_syncing() {
            timeout = timeout.min(ANIMATION_FRAME);
        }
        if event::poll(timeout)? {
            let exit = match event::read()? {
                Event::Key(key) => app.on_key(key),
                Event::Mouse(mouse) => app.on_mouse(mouse),
                Event::Resize(width, _) => {
                    app.on_resize(width);
                    None
                }
                _ => None,
            };
            if let Some(exit) = exit {
                // Switching drops this App and rebuilds it later, so it needs
                // the same persistence gate as quitting or a draft would be
                // lost despite the process staying alive.
                if !app.persist_scm() {
                    continue;
                }
                if exit == Exit::Quit {
                    app.clear_identity();
                }
                return Ok(exit);
            }
        }
        app.heartbeat();
        app.poll_picker();
        if last_tick.elapsed() >= REFRESH_EVERY {
            app.tick();
            last_tick = std::time::Instant::now();
        }
        roots.record(app.root_path());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn memory(root: &str, label: &str, spawn_cwd: &str) -> RootMemory {
        RootMemory {
            root: root.into(),
            startup_label: label.into(),
            spawn_cwd: spawn_cwd.into(),
            pending: false,
            last_attempt: None,
        }
    }

    /// The reported bug: a workspace created from sx-flow's folder is named
    /// "sx-flow" until its pane moves to GCP. Its sidebar must not save GCP
    /// under the real sx-flow workspace's key.
    #[test]
    fn a_followed_root_is_saved_under_the_workspace_s_current_label() {
        let mut roots = memory("/dev/sx-flow", "sx-flow", "/dev/sx-flow");
        let now = std::time::Instant::now();
        let key = roots.save_key(Path::new("/dev/GCP"), now, || Some("GCP".into()));
        assert_eq!(key.as_deref(), Some("GCP::/dev/sx-flow"));
        assert_ne!(
            key,
            Some(remembered_root_key("sx-flow", Path::new("/dev/sx-flow")))
        );
        assert_eq!(roots.root, Path::new("/dev/GCP"));
    }

    #[test]
    fn an_unchanged_root_is_not_rewritten() {
        let mut roots = memory("/dev/app", "app", "/dev/app");
        let now = std::time::Instant::now();
        let key = roots.save_key(Path::new("/dev/app"), now, || panic!("no lookup needed"));
        assert_eq!(key, None);
    }

    /// Astra #2: a failed lookup must never fall back to the startup label
    /// (that recreates the cross-workspace overwrite). The save waits, retries
    /// at a bounded rate, and lands under the live label once it is known.
    #[test]
    fn a_failed_label_lookup_defers_the_save_instead_of_guessing() {
        let mut roots = memory("/dev/sx-flow", "sx-flow", "/dev/sx-flow");
        let start = std::time::Instant::now();
        assert_eq!(roots.save_key(Path::new("/dev/GCP"), start, || None), None);
        assert_eq!(roots.root, Path::new("/dev/GCP"), "the view still follows");
        assert_eq!(
            roots.save_key(Path::new("/dev/GCP"), start, || panic!("retry throttled")),
            None
        );
        let later = start + ROOT_SAVE_RETRY;
        let key = roots.save_key(Path::new("/dev/GCP"), later, || Some("GCP".into()));
        assert_eq!(key.as_deref(), Some("GCP::/dev/sx-flow"));
        assert_eq!(
            roots.save_key(Path::new("/dev/GCP"), later + ROOT_SAVE_RETRY, || {
                panic!("nothing pending")
            }),
            None
        );
    }

    /// Astra rounds 2+3 #1/#2: over many ordinary loop iterations, a slow
    /// failed lookup is retried once per window measured from when it
    /// FINISHED — not instantly, and not pushed back by throttled iterations.
    #[test]
    fn pending_saves_retry_once_per_window_across_loop_iterations() {
        let mut roots = memory("/dev/a", "a", "/dev/a");
        let start = std::time::Instant::now();
        let now = std::cell::Cell::new(start);
        let lookups = std::cell::Cell::new(0);
        let slow_failure = || {
            lookups.set(lookups.get() + 1);
            now.set(now.get() + Duration::from_secs(5)); // the socket timed out
            None
        };
        assert_eq!(
            roots.record_with(Path::new("/dev/b"), || now.get(), slow_failure),
            None
        );
        assert_eq!(lookups.get(), 1);
        let finished = now.get();
        // Ordinary iterations, 1s apart, up to just before the window ends.
        while now.get() + Duration::from_secs(1) < finished + ROOT_SAVE_RETRY {
            now.set(now.get() + Duration::from_secs(1));
            let key = roots.record_with(
                Path::new("/dev/b"),
                || now.get(),
                || panic!("throttled: no lookup before the window ends"),
            );
            assert_eq!(key, None);
        }
        now.set(finished + ROOT_SAVE_RETRY);
        let key = roots.record_with(Path::new("/dev/b"), || now.get(), || Some("b".into()));
        assert_eq!(
            key.as_deref(),
            Some("b::/dev/a"),
            "retried once the window passed"
        );
    }

    /// Astra round 3 #3: the apps' own close paths reach the pending save.
    #[test]
    fn a_pending_root_is_published_for_the_apps_close_paths() {
        let mut roots = memory("/dev/a", "a", "/dev/a");
        let start = std::time::Instant::now();
        roots.record_with(Path::new("/dev/b"), || start, || None);
        publish_pending(&roots);
        let published = PENDING_ROOT
            .lock()
            .unwrap()
            .take()
            .expect("pending save published");
        assert_eq!(published.root, Path::new("/dev/b"));
        assert!(published.pending);
        roots.pending = false;
        publish_pending(&roots);
        assert!(
            PENDING_ROOT.lock().unwrap().is_none(),
            "nothing pending, nothing published"
        );
    }

    #[test]
    fn a_process_that_never_had_a_label_still_saves_by_folder() {
        let mut roots = memory("/dev/app", "", "/dev/app");
        let now = std::time::Instant::now();
        let key = roots.save_key(Path::new("/dev/app/sub"), now, || None);
        assert_eq!(key, Some(remembered_root_key("", Path::new("/dev/app"))));
    }

    #[test]
    fn remembered_root_keys_are_project_stable_not_tab_scoped() {
        let key = remembered_root_key("acme", std::path::Path::new(r"C:\Repo\Web"));
        assert!(key.starts_with("acme::"));
        assert!(!key.contains('\\'));
        assert!(!key.contains("w1:t"));
        if cfg!(windows) {
            assert_eq!(key, "acme::c:/repo/web");
        }
    }
}
