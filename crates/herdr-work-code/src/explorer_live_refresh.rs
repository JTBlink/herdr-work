//! Refresh only visible directories off the input thread, including outside Git repos.
use super::*;
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

const TREE_REFRESH: Duration = Duration::from_secs(1);

pub(super) struct TreeRead {
    tree: Tree,
    rows: Vec<Row>,
}
#[derive(Default)]
pub(super) struct LiveTree {
    pending: Option<Receiver<TreeRead>>,
    last_run: Option<Instant>,
}

impl App {
    pub(super) fn tick_live_tree(&mut self) {
        self.collect_live_tree();
        let now = Instant::now();
        if self.live_tree.pending.is_some()
            || self
                .live_tree
                .last_run
                .is_some_and(|at| now.saturating_duration_since(at) < TREE_REFRESH)
        {
            return;
        }
        self.live_tree.last_run = Some(now);
        if !self.background_refresh && !self.pane_is_focused() {
            return;
        }
        let mut tree = Tree::new(self.tree.root_path());
        tree.set_expanded(self.tree.expanded_paths());
        tree.show_hidden = self.tree.show_hidden;
        let (tx, rx) = mpsc::channel();
        self.live_tree.pending = Some(rx);
        std::thread::spawn(move || {
            let rows = tree.rows();
            let _ = tx.send(TreeRead { tree, rows });
        });
    }
    fn collect_live_tree(&mut self) {
        let Some(result) = self.live_tree.pending.as_ref().map(Receiver::try_recv) else {
            return;
        };
        let fresh = match result {
            Ok(fresh) => fresh,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.live_tree.pending = None;
                return;
            }
        };
        self.live_tree.pending = None;
        // User navigation can change the query while the directory read is pending.
        if fresh.tree.root_path() != self.tree.root_path()
            || fresh.tree.expanded_paths() != self.tree.expanded_paths()
            || fresh.tree.show_hidden != self.tree.show_hidden
        {
            return;
        }
        if fresh.rows == self.rows {
            self.tree = fresh.tree;
            return;
        }
        let selected = self
            .selected
            .and_then(|i| self.rows.get(i).map(|r| r.path.clone()));
        let top = self.rows.get(self.scroll).map(|r| r.path.clone());
        let selection_index = self.selected;
        self.tree = fresh.tree;
        self.rows = fresh.rows;
        self.selected = selected
            .and_then(|path| self.rows.iter().position(|r| r.path == path))
            .or_else(|| {
                selection_index
                    .filter(|_| !self.rows.is_empty())
                    .map(|i| i.min(self.rows.len() - 1))
            });
        self.scroll = top
            .and_then(|path| self.rows.iter().position(|r| r.path == path))
            .unwrap_or(self.scroll)
            .min(self.rows.len().saturating_sub(1));
        self.hovered = None;
        self.last_click = None;
        self.invalidate_quick_index();
        // Do not persist on every poll: that would replace another pane's selection
        // and feed stale shared state back into this view.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.live_tree.pending.is_some() {
            app.collect_live_tree();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn outside_edits_refresh_visible_tree_and_preserve_selection_scroll_and_prompt() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("expanded")).unwrap();
        std::fs::write(root.path().join("expanded/keep.txt"), "keep").unwrap();
        let mut app = App::new(root.path().into(), Default::default());
        app.tree.expand(&root.path().join("expanded"));
        app.rebuild();
        let selected = app.rows.iter().position(|r| r.name == "keep.txt").unwrap();
        app.selected = Some(selected);
        app.scroll = selected;
        app.overlay = Some(Overlay::Prompt {
            title: "New file".into(),
            input: "draft.txt".into(),
            kind: PromptKind::NewFile(root.path().into()),
        });
        std::fs::write(root.path().join("expanded/added.txt"), "added").unwrap();
        app.tick_live_tree();
        settle(&mut app);
        assert!(app.rows.iter().any(|r| r.name == "added.txt"));
        assert_eq!(app.rows[app.selected.unwrap()].name, "keep.txt");
        assert_eq!(app.rows[app.scroll].name, "keep.txt");
        assert!(
            matches!(&app.overlay, Some(Overlay::Prompt { input, .. }) if input == "draft.txt")
        );
        std::fs::rename(
            root.path().join("expanded/added.txt"),
            root.path().join("expanded/renamed.txt"),
        )
        .unwrap();
        std::fs::remove_file(root.path().join("expanded/keep.txt")).unwrap();
        app.live_tree.last_run = None;
        app.tick_live_tree();
        settle(&mut app);
        assert!(app.rows.iter().any(|r| r.name == "renamed.txt"));
        assert!(
            !app.rows
                .iter()
                .any(|r| r.name == "keep.txt" || r.name == "added.txt")
        );
    }
    #[test]
    fn late_tree_read_cannot_reopen_a_directory_the_user_just_collapsed() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("folder")).unwrap();
        std::fs::write(root.path().join("folder/file.txt"), "content").unwrap();
        let mut app = App::new(root.path().into(), Default::default());
        app.tree.expand(&root.path().join("folder"));
        app.rebuild();
        app.tick_live_tree();
        app.tree.collapse(&root.path().join("folder"));
        app.rebuild();
        settle(&mut app);
        assert!(!app.tree.is_expanded(&root.path().join("folder")));
        assert!(!app.rows.iter().any(|r| r.name == "file.txt"));
    }
}
