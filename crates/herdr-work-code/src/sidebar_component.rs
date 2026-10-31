//! The original Sidebar applications, hosted as a component by HerdrWork's
//! single terminal/event loop. Rendering and interaction stay in the source
//! apps; this adapter routes their Exit transitions and product collapse controls.
use crate::sidebar_chrome::{self, Action};
use crate::{
    activity::InitialView,
    explorer_app,
    launch::CwdFollower,
    scm_app,
    state::{self, Exit, View},
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
};
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
    roots: crate::root_memory::RootMemory,
    last_title: Option<&'static str>,
    projects_requested: bool,
    collapsed: bool,
    initial_rail_size_pending: bool,
    expanded_width: u16,
    area: Rect,
    pointer: Option<Position>,
    pane_chrome: Option<u16>,
    last_rail_check: Instant,
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
            roots,
            last_title: None,
            projects_requested: false,
            collapsed: false,
            initial_rail_size_pending: false,
            expanded_width: saved.sidebar_width,
            area: Rect::default(),
            pointer: None,
            pane_chrome: None,
            last_rail_check: Instant::now(),
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
        if self.collapsed {
            return;
        }
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
    /// Automatic tab creation starts with the rail, once terminal geometry is
    /// available. Explicit activity requests continue to open the full view.
    pub fn start_collapsed(&mut self) {
        self.collapsed = true;
        self.initial_rail_size_pending = true;
    }
    pub fn draw(&mut self, frame: &mut Frame) {
        self.area = frame.area();
        if !self.collapsed {
            match &mut self.app {
                ViewApp::Explorer(app) => app.draw(frame),
                ViewApp::Git(app) => app.draw(frame),
            }
        }
        if self.collapsed || self.chrome_available() {
            let (active, theme) = match &self.app {
                ViewApp::Git(app) => (Action::Git, app.chrome_theme()),
                ViewApp::Explorer(app) => (
                    if app.search_active() {
                        Action::Search
                    } else {
                        Action::Files
                    },
                    app.chrome_theme(),
                ),
            };
            sidebar_chrome::draw(frame, self.collapsed, active, theme, self.pointer);
        }
    }
    fn set_collapsed(&mut self, collapsed: bool) {
        if self.collapsed == collapsed {
            return;
        }
        self.initial_rail_size_pending = false;
        if collapsed {
            self.expanded_width = self.area.width.max(sidebar_chrome::RAIL_WIDTH);
            self.pane_chrome = None;
        }
        let target = if collapsed {
            sidebar_chrome::RAIL_WIDTH
        } else {
            self.expanded_width
        };
        self.resize_columns(self.area.width, target, Some(collapsed));
        self.collapsed = collapsed;
    }
    fn resize_columns(&mut self, current: u16, target: u16, collapsed: Option<bool>) {
        let chrome = match &self.app {
            ViewApp::Explorer(app) => {
                app.resize_sidebar(current, target, collapsed, self.pane_chrome)
            }
            ViewApp::Git(app) => app.resize_sidebar(current, target, collapsed, self.pane_chrome),
        };
        if chrome.is_some() {
            self.pane_chrome = chrome;
        }
        self.last_rail_check = Instant::now();
    }
    fn chrome_action(&mut self, action: Action) -> bool {
        match action {
            Action::Collapse => self.set_collapsed(true),
            Action::Expand => self.set_collapsed(false),
            Action::Close => {
                if !self.flush() {
                    self.set_collapsed(false);
                    return false;
                }
                let quit = match &mut self.app {
                    ViewApp::Explorer(app) => app.close_sidebar(),
                    ViewApp::Git(app) => app.close_sidebar(),
                };
                // If a host close is refused, show the source app's notice.
                if !quit {
                    self.set_collapsed(false);
                }
                return quit;
            }
            Action::Projects => self.projects_requested = true,
            Action::Settings => {
                self.set_collapsed(false);
                match &mut self.app {
                    ViewApp::Explorer(app) => app.open_settings(),
                    ViewApp::Git(app) => app.open_settings(),
                }
            }
            Action::Files | Action::Search | Action::Git => {
                self.set_collapsed(false);
                let code = KeyCode::F(match action {
                    Action::Files => 9,
                    Action::Search => 10,
                    _ => 11,
                });
                return self.event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
            }
        }
        false
    }
    fn chrome_available(&self) -> bool {
        match &self.app {
            ViewApp::Explorer(app) => app.chrome_available(),
            ViewApp::Git(app) => app.chrome_available(),
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
        self.set_collapsed(false);
        self.app = Self::build(root.to_owned(), view, Rc::clone(&self.follower));
        self.refresh_title();
        true
    }
    pub fn take_projects_request(&mut self) -> bool {
        std::mem::take(&mut self.projects_requested)
    }
    pub fn take_worktree_request(&mut self) -> Option<(PathBuf, PathBuf)> {
        match &mut self.app {
            ViewApp::Git(app) => app.take_worktree_request(),
            ViewApp::Explorer(_) => None,
        }
    }
    pub fn event(&mut self, event: Event) -> bool {
        if let Event::Mouse(mouse) = event {
            self.pointer = matches!(mouse.kind, MouseEventKind::Moved | MouseEventKind::Drag(_))
                .then_some(Position::new(mouse.column, mouse.row));
            if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && (self.collapsed || self.chrome_available())
                && let Some(action) = sidebar_chrome::hit(
                    self.area,
                    self.collapsed,
                    Position::new(mouse.column, mouse.row),
                )
            {
                return self.chrome_action(action);
            }
        }
        if self.collapsed
            && let Event::Key(key) = event
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('p' | 'f' | 'F'))
            && key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            self.set_collapsed(false);
        }
        if self.collapsed {
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let action = match key.code {
                        KeyCode::Char('1') | KeyCode::F(9) => Some(Action::Files),
                        KeyCode::Char('2') | KeyCode::F(10) => Some(Action::Search),
                        KeyCode::Char('3') | KeyCode::F(11) => Some(Action::Git),
                        KeyCode::Char('4')
                            if key.modifiers.is_empty()
                                || key.modifiers == KeyModifiers::CONTROL =>
                        {
                            Some(Action::Projects)
                        }
                        KeyCode::Char('s') => Some(Action::Settings),
                        KeyCode::Enter | KeyCode::Right => Some(Action::Expand),
                        KeyCode::Char('b' | 'q') => Some(Action::Close),
                        _ => None,
                    };
                    if let Some(action) = action {
                        return self.chrome_action(action);
                    }
                }
                // Never send collapsed dimensions to the source apps: their
                // preferred-width repair would immediately expand the rail.
                Event::Resize(width, height) => {
                    self.area.width = width;
                    self.area.height = height;
                    if !self.initial_rail_size_pending && width != sidebar_chrome::RAIL_WIDTH {
                        self.resize_columns(width, sidebar_chrome::RAIL_WIDTH, None);
                    }
                    return false;
                }
                Event::Mouse(_) => return false,
                _ => {}
            }
            return false;
        }
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
        if exit == Exit::Projects {
            self.projects_requested = true;
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
            Exit::Quit | Exit::Projects => unreachable!(),
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
        if self.initial_rail_size_pending && !self.area.is_empty() {
            self.initial_rail_size_pending = false;
            let preferred = self.expanded_width;
            self.resize_columns(self.area.width, sidebar_chrome::RAIL_WIDTH, Some(true));
            // Startup dimensions can belong to the host's temporary split.
            // Restore the configured normal width, not that transient width.
            self.expanded_width = preferred
                .saturating_sub(self.pane_chrome.unwrap_or_default())
                .max(sidebar_chrome::RAIL_WIDTH);
        }
        // At Herdr's four-column PTY minimum a smaller physical viewport no
        // longer emits Resize. Preserve the chrome measured before collapse
        // and check the actual split geometry even without a terminal event.
        if self.collapsed && self.last_rail_check.elapsed() >= Duration::from_secs(1) {
            self.resize_columns(self.area.width, sidebar_chrome::RAIL_WIDTH, None);
        }
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
                app.tick();
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
                for y in 0..height {
                    for x in 0..width {
                        if y == 0 && x >= width.saturating_sub(3) && !component.busy() {
                            continue;
                        }
                        assert_eq!(
                            actual.backend().buffer()[(x, y)],
                            expected.backend().buffer()[(x, y)],
                            "{view:?} {width}x{height} at {x},{y}"
                        );
                    }
                }
                if !component.busy() {
                    assert_eq!(actual.backend().buffer()[(width - 2, 0)].symbol(), "«");
                }
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
