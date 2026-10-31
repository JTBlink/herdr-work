//! Passive Git reads never own the terminal or replace commit inputs.
use super::*;
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

const STATUS_REFRESH: Duration = Duration::from_millis(1500);

struct StatusRead {
    cwd: PathBuf,
    repos: Vec<(Git, Result<Status, String>)>,
    drawer_root: Option<PathBuf>,
    drawers: Vec<(Drawer, Result<Vec<String>, String>)>,
}
#[derive(Default)]
pub(super) struct LiveStatus {
    pending: Option<Receiver<StatusRead>>,
    last_run: Option<Instant>,
}
impl LiveStatus {
    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.last_run = Some(Instant::now());
    }
}

impl App {
    pub(super) fn tick_live_status(&mut self) {
        // Drawer menu targets contain row indices; defer changing their backing list
        // until the menu is closed. Message editing remains live.
        if self.overlay.is_some() {
            return;
        }
        self.collect_live_status();
        let now = Instant::now();
        if self.live_status.pending.is_some()
            || self
                .live_status
                .last_run
                .is_some_and(|at| now.saturating_duration_since(at) < STATUS_REFRESH)
        {
            return;
        }
        self.live_status.last_run = Some(now);
        if !self.background_refresh && !self.pane_is_focused() {
            return;
        }
        let cwd = self.cwd.clone();
        let mut repos = self
            .repos
            .iter()
            .map(|repo| repo.git.clone())
            .collect::<Vec<_>>();
        let active = self.active_repo().map(|repo| repo.git.clone());
        let kinds = Drawer::ALL
            .into_iter()
            .filter(|kind| !kind.opens_preview() && self.drawers[kind.index()].expanded)
            .collect::<Vec<_>>();
        let file = self.history_target.clone();
        let (tx, rx) = mpsc::channel();
        self.live_status.pending = Some(rx);
        std::thread::spawn(move || {
            if repos.is_empty() {
                repos = Git::discover_all(&cwd);
            }
            let repos = repos
                .into_iter()
                .map(|git| {
                    let status = git.status();
                    (git, status)
                })
                .collect();
            let drawers = active
                .as_ref()
                .map(|git| {
                    kinds
                        .into_iter()
                        .map(|kind| (kind, kind.load(git, file.as_deref(), DRAWER_LIMIT)))
                        .collect()
                })
                .unwrap_or_default();
            let _ = tx.send(StatusRead {
                cwd,
                repos,
                drawer_root: active.map(|git| git.root().into()),
                drawers,
            });
        });
    }
    fn collect_live_status(&mut self) {
        let Some(result) = self.live_status.pending.as_ref().map(Receiver::try_recv) else {
            return;
        };
        let read = match result {
            Ok(read) => read,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.live_status.pending = None;
                return;
            }
        };
        self.live_status.pending = None;
        if read.cwd != self.cwd {
            return;
        }
        let selection = self.selected.and_then(|i| self.row_stable_id(i));
        let top = self.row_stable_id(self.scroll);
        let mut changed = false;
        for (git, status) in read.repos {
            match status {
                Ok(status) => {
                    if let Some(repo) = self
                        .repos
                        .iter_mut()
                        .find(|repo| repo.git.root() == git.root())
                    {
                        if repo.status != status {
                            repo.status = status;
                            changed = true;
                        }
                    } else {
                        let mut repo = Repo::new(git);
                        repo.status = status;
                        self.repos.push(repo);
                        self.discover_err.clear();
                        changed = true;
                    }
                }
                Err(error) => {
                    self.flash = Notice::new((format!("Git 自动刷新失败：{error}"), true))
                }
            }
        }
        if read.drawer_root.as_deref() == self.active_repo().map(|r| r.git.root()) {
            for (kind, lines) in read.drawers {
                if self.drawers[kind.index()].expanded {
                    self.set_drawer_lines(kind, lines);
                    changed = true;
                }
            }
        }
        if self.drawers[Drawer::Graph.index()].expanded {
            self.refresh_history();
        }
        if !changed {
            return;
        }
        // Rebuilding with a stale row index can switch the active repository.
        // Restore by identity before asking follow_selection to update its context.
        let old_index = self.selected.take();
        self.rebuild();
        self.selected = selection
            .and_then(|id| self.find_row_by_stable_id(&id))
            .or_else(|| {
                old_index
                    .filter(|_| !self.rows.is_empty())
                    .map(|i| i.min(self.rows.len() - 1))
            });
        if let Some(index) = top.and_then(|id| self.find_row_by_stable_id(&id)) {
            self.scroll = index;
        }
        if self.focus == Focus::List {
            self.follow_selection();
        }
        self.hovered = None;
        self.last_click = None;
    }
    pub(super) fn set_drawer_lines(&mut self, kind: Drawer, lines: Result<Vec<String>, String>) {
        let panel = &mut self.drawers[kind.index()];
        panel.lines = match lines {
            Ok(lines) if lines.is_empty() => vec!["(none)".into()],
            Ok(lines) => lines,
            Err(error) => vec![format!("({error})")],
        };
        panel.refs = panel
            .lines
            .iter()
            .map(|line| parse_drawer_ref(kind, line))
            .collect();
        match kind {
            Drawer::Worktrees => {
                panel.lines = panel
                    .lines
                    .iter()
                    .map(|line| pretty_worktree_line(line))
                    .collect()
            }
            Drawer::Remotes => {
                panel.lines = panel
                    .lines
                    .iter()
                    .map(|line| pretty_remote_line(line))
                    .collect()
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::tests::Repo as Fixture;

    fn settle(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.live_status.pending.is_some() {
            app.collect_live_status();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn changed_file(app: &App, path: &str) -> Option<usize> {
        app.rows.iter().position(|row| matches!(row, Row::Unstaged(r,i) if app.repos[*r].status.unstaged[*i].path == path))
    }
    #[test]
    fn outside_git_changes_refresh_while_editing_and_keep_file_identity_and_draft() {
        let fixture = Fixture::new();
        fixture.write("keep.txt", "initial\n");
        fixture.write("delete.txt", "delete\n");
        fixture.run(&["add", "."]);
        fixture.run(&["commit", "-qm", "initial"]);
        fixture.write("keep.txt", "modified\n");
        let mut app = App::new(fixture.0.path().into(), Default::default());
        let index = changed_file(&app, "keep.txt").unwrap();
        app.selected = Some(index);
        app.scroll = index;
        app.focus = Focus::Message;
        app.repos[app.active].message = "未完成的提交草稿".chars().collect();
        app.repos[app.active].cursor = 3;
        fixture.write("aaa.txt", "new\n");
        std::fs::remove_file(fixture.0.path().join("delete.txt")).unwrap();
        app.live_status.last_run = None;
        app.tick_live_status();
        settle(&mut app);
        assert!(changed_file(&app, "aaa.txt").is_some());
        assert!(changed_file(&app, "delete.txt").is_some());
        assert_eq!(app.selected, changed_file(&app, "keep.txt"));
        assert_eq!(Some(app.scroll), changed_file(&app, "keep.txt"));
        assert!(app.focus == Focus::Message);
        assert_eq!(app.repos[app.active].cursor, 3);
        assert_eq!(
            app.repos[app.active].message.iter().collect::<String>(),
            "未完成的提交草稿"
        );
        fixture.run(&["add", "aaa.txt"]);
        app.live_status.last_run = None;
        app.tick_live_status();
        settle(&mut app);
        assert!(
            app.repos[app.active]
                .status
                .staged
                .iter()
                .any(|e| e.path == "aaa.txt")
        );
        assert!(changed_file(&app, "aaa.txt").is_none());
    }
    #[test]
    fn manual_refresh_discards_an_older_passive_read() {
        let fixture = Fixture::new();
        fixture.run(&["commit", "--allow-empty", "-qm", "initial"]);
        let mut app = App::new(fixture.0.path().into(), Default::default());
        let (tx, rx) = mpsc::channel();
        app.live_status.pending = Some(rx);
        fixture.write("fresh.txt", "fresh\n");
        app.refresh();
        assert!(
            tx.send(StatusRead {
                cwd: app.cwd.clone(),
                repos: vec![(fixture.git(), Ok(Status::default()))],
                drawer_root: None,
                drawers: vec![]
            })
            .is_err()
        );
        assert!(changed_file(&app, "fresh.txt").is_some());
    }

    #[test]
    fn disappearing_selected_change_cannot_retarget_an_active_multi_repo_draft() {
        let first = Fixture::new();
        first.write("keep.txt", "initial\n");
        first.run(&["add", "."]);
        first.run(&["commit", "-qm", "initial"]);
        first.write("keep.txt", "modified\n");
        let second = Fixture::new();
        second.run(&["commit", "--allow-empty", "-qm", "initial"]);
        let mut app = App::new(first.0.path().into(), Default::default());
        app.repos.push(Repo::new(second.git()));
        app.rebuild();
        app.selected = changed_file(&app, "keep.txt");
        app.focus = Focus::Message;
        app.active = 0;
        app.repos[0].message = "draft".chars().collect();
        app.repos[0].cursor = 5;
        first.run(&["restore", "keep.txt"]);
        app.live_status.last_run = None;
        app.tick_live_status();
        settle(&mut app);
        assert_eq!(app.active, 0);
        app.on_message_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));
        assert_eq!(app.repos[0].message.iter().collect::<String>(), "draft!");
        assert!(app.repos[1].message.is_empty());
    }
}
