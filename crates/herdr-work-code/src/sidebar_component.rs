//! The original Sidebar applications, hosted as a component by HerdrWork's
//! single terminal/event loop. Rendering and interaction stay in the source
//! apps; this adapter only routes their existing Exit transitions.
use crate::{
    activity::InitialView,
    explorer_app,
    launch::CwdFollower,
    scm_app,
    state::{self, Exit, View},
};
use crossterm::event::Event;
use ratatui::Frame;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

enum ViewApp {
    Explorer(Box<explorer_app::App>),
    Git(Box<scm_app::App>),
}
pub struct Sidebar {
    app: ViewApp,
    follower: Rc<RefCell<CwdFollower>>,
    last_tick: Instant,
    roots: crate::root_memory::RootMemory,
    last_title: Option<&'static str>,
}
impl Sidebar {
    pub fn new(root: PathBuf, initial: Option<InitialView>) -> Self {
        let follower = Rc::new(RefCell::new(CwdFollower::default()));
        let saved = state::load_state();
        let initial = initial.unwrap_or(if saved.active == View::SourceControl {
            InitialView::Git
        } else if saved.search_active {
            InitialView::Search
        } else {
            InitialView::Explorer
        });
        let startup_label = crate::root_memory::workspace_label();
        let spawn_cwd = root;
        let key = crate::root_memory::remembered_root_key(&startup_label, &spawn_cwd);
        let root = crate::root_memory::resolve_root(&key, &startup_label, &spawn_cwd)
            .unwrap_or_else(|_| spawn_cwd.clone());
        let roots = crate::root_memory::RootMemory {
            root: root.clone(),
            startup_label,
            spawn_cwd,
            pending: false,
            last_attempt: None,
        };
        let app = Self::build(root, initial, Rc::clone(&follower));
        let mut sidebar = Self {
            app,
            follower,
            last_tick: Instant::now(),
            roots,
            last_title: None,
        };
        sidebar.refresh_title();
        sidebar
    }
    fn build(root: PathBuf, initial: InitialView, follower: Rc<RefCell<CwdFollower>>) -> ViewApp {
        state::update_state(|saved| {
            saved.active = if initial == InitialView::Git {
                View::SourceControl
            } else {
                View::Explorer
            };
            saved.search_active = initial == InitialView::Search;
        });
        match initial {
            InitialView::Git => ViewApp::Git(Box::new(scm_app::App::new(root, follower))),
            _ => {
                let mut app = explorer_app::App::new(root, follower);
                if initial == InitialView::Search {
                    app.open_content_search(false);
                }
                if initial == InitialView::QuickOpen {
                    app.open_quick_open();
                }
                ViewApp::Explorer(Box::new(app))
            }
        }
    }
    fn refresh_title(&mut self) {
        let title = match &self.app {
            ViewApp::Explorer(app) => app.host_title(),
            ViewApp::Git(app) => app.host_title(),
        };
        if self.last_title == Some(title) {
            return;
        }
        match &self.app {
            ViewApp::Explorer(app) => app.refresh_host_title(),
            ViewApp::Git(app) => app.refresh_host_title(),
        }
        self.last_title = Some(title);
    }
    pub fn root(&self) -> PathBuf {
        match &self.app {
            ViewApp::Explorer(app) => app.root_path(),
            ViewApp::Git(app) => app.root_path().to_owned(),
        }
    }
    pub fn draw(&mut self, frame: &mut Frame) {
        match &mut self.app {
            ViewApp::Explorer(app) => app.draw(frame),
            ViewApp::Git(app) => app.draw(frame),
        }
    }
    pub fn busy(&self) -> bool {
        match &self.app {
            ViewApp::Explorer(app) => app.host_request_busy(),
            ViewApp::Git(app) => app.host_request_busy(),
        }
    }
    pub fn notice(&mut self, message: String) {
        match &mut self.app {
            ViewApp::Explorer(app) => app.host_notice(message),
            ViewApp::Git(app) => app.host_notice(message),
        }
    }
    pub fn activate(&mut self, root: &Path, view: InitialView) -> bool {
        if self.busy() {
            self.notice("已聚焦 Sidebar；请先结束当前输入或弹窗".into());
            return false;
        }
        if let ViewApp::Git(app) = &mut self.app
            && !app.persist_scm()
        {
            return false;
        }
        self.app = Self::build(root.to_owned(), view, Rc::clone(&self.follower));
        self.refresh_title();
        true
    }
    pub fn event(&mut self, event: Event) -> bool {
        let exit = match (&mut self.app, event) {
            (ViewApp::Explorer(app), Event::Key(key)) => app.on_key(key),
            (ViewApp::Git(app), Event::Key(key)) => app.on_key(key),
            (ViewApp::Explorer(app), Event::Mouse(mouse)) => app.on_mouse(mouse),
            (ViewApp::Git(app), Event::Mouse(mouse)) => app.on_mouse(mouse),
            (ViewApp::Explorer(app), Event::Resize(width, _)) => {
                app.on_resize(width);
                None
            }
            (ViewApp::Git(app), Event::Resize(width, _)) => {
                app.on_resize(width);
                None
            }
            _ => None,
        };
        self.refresh_title();
        let Some(exit) = exit else { return false };
        if let ViewApp::Git(app) = &mut self.app
            && !app.persist_scm()
        {
            return false;
        }
        if exit == Exit::Quit {
            match &self.app {
                ViewApp::Explorer(app) => app.clear_identity(),
                ViewApp::Git(app) => app.clear_identity(),
            };
            return true;
        }
        let target = match exit {
            Exit::Switch => {
                if matches!(self.app, ViewApp::Explorer(_)) {
                    InitialView::Git
                } else {
                    InitialView::Explorer
                }
            }
            Exit::Search { .. } => InitialView::Search,
            Exit::QuickOpen => InitialView::QuickOpen,
            Exit::Quit => unreachable!(),
        };
        let focus_query = matches!(exit, Exit::Search { focus_query: true });
        self.app = Self::build(self.root(), target, Rc::clone(&self.follower));
        if focus_query && let ViewApp::Explorer(app) = &mut self.app {
            app.open_content_search(true);
        }
        self.refresh_title();
        false
    }
    pub fn tick(&mut self) {
        self.roots.record(&self.root());
        match &mut self.app {
            ViewApp::Explorer(app) => {
                app.heartbeat();
                app.poll_picker();
                app.tick();
            }
            ViewApp::Git(app) => {
                app.heartbeat();
                app.poll_picker();
                if self.last_tick.elapsed() >= Duration::from_millis(1500) {
                    app.tick();
                    self.last_tick = Instant::now();
                }
            }
        }
    }
    pub fn flush(&mut self) -> bool {
        self.roots.flush();
        match &mut self.app {
            ViewApp::Explorer(_) => true,
            ViewApp::Git(app) => app.persist_scm(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn default_component_draws_the_original_apps_without_a_second_layout() {
        let name = format!(
            "herdr-work-source-ui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let fixture = std::env::temp_dir().join(name);
        let root = fixture.join("herdr-work");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("README.md"), "Sidebar fixture\n").unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "Sidebar fixture",
            ],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(
            root.join("src/main.rs"),
            "fn main() { println!(\"fixture\"); }\n",
        )
        .unwrap();
        for view in [InitialView::Explorer, InitialView::Search, InitialView::Git] {
            for (width, height) in [(28, 24), (32, 24), (48, 32), (80, 24)] {
                let mut component = Sidebar::new(root.clone(), Some(view));
                let mut actual = Terminal::new(TestBackend::new(width, height)).unwrap();
                actual.draw(|f| component.draw(f)).unwrap();
                let follower = Rc::new(RefCell::new(CwdFollower::default()));
                let mut expected = Terminal::new(TestBackend::new(width, height)).unwrap();
                match view {
                    InitialView::Git => {
                        let mut original = scm_app::App::new(root.clone(), follower);
                        expected.draw(|f| original.draw(f)).unwrap();
                    }
                    _ => {
                        let mut original = explorer_app::App::new(root.clone(), follower);
                        if view == InitialView::Search {
                            original.open_content_search(false);
                        }
                        expected.draw(|f| original.draw(f)).unwrap();
                    }
                }
                assert_eq!(
                    actual.backend().buffer(),
                    expected.backend().buffer(),
                    "{view:?} {width}x{height}"
                );
                if width == 28
                    && let Some(dir) = std::env::var_os("HERDR_WORK_TEST_FRAMES")
                {
                    std::fs::create_dir_all(&dir).unwrap();
                    let cells=actual.backend().buffer().content.iter().map(|c|serde_json::json!({"text":c.symbol(),"fg":format!("{:?}",c.fg),"bg":format!("{:?}",c.bg)})).collect::<Vec<_>>();
                    let label = match view {
                        InitialView::Explorer => "source-explorer",
                        InitialView::Search => "source-search",
                        _ => "source-git",
                    };
                    std::fs::write(std::path::Path::new(&dir).join(format!("{label}.json")),serde_json::to_vec(&serde_json::json!({"width":width,"height":height,"cells":cells,"fixture":true})).unwrap()).unwrap();
                }
            }
        }
        std::fs::remove_dir_all(fixture).unwrap();
    }
}
