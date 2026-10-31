//! Product history state reuses Code's compact model, full graph and commit viewer.
use crate::{
    scm::{ScmRow, ScmTarget},
    state::*,
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use herdr_work_code::{
    history::Browser,
    history_view::{FileTarget, GraphView},
    preview::Doc,
    scm::Drawer,
};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

#[derive(Default)]
pub(crate) struct HistoryUi {
    pub sidebar: Browser,
    pub expanded: bool,
    pub graph: Option<GraphView>,
    target: Option<(PathBuf, String, usize, Option<String>)>,
    pending: Option<Receiver<Doc>>,
    from_graph: bool,
}
impl HistoryUi {
    pub(crate) fn clear_preview(&mut self) {
        self.target = None;
        self.pending = None;
    }
}
impl App {
    pub(crate) fn tick_history(&mut self) -> bool {
        let mut changed = self.history.sidebar.poll();
        if let Some(view) = &mut self.history.graph {
            changed |= view.tick();
        }
        if let Some(doc) = self
            .history
            .pending
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
        {
            self.history.pending = None;
            if self.active_drawer != Some(Drawer::Graph) {
                return changed;
            }
            self.drawer_doc = Some(doc);
            self.drawer_show = Some("history".into());
            changed = true;
        }
        changed
    }
    pub(crate) fn ensure_history(&mut self) {
        let root = self.selected_repository();
        if self.history.sidebar.root != root {
            self.history.sidebar = Browser::new(root);
            self.history.expanded = false;
        }
        self.history.sidebar.refresh();
    }
    pub(crate) fn open_graph(&mut self) {
        self.drawer_request += 1;
        self.drawer_scroll = 0;
        let root = self.selected_repository();
        if self
            .history
            .graph
            .as_ref()
            .is_none_or(|v| v.browser.root != root)
        {
            let mut graph = GraphView::new(root);
            graph.refresh();
            self.history.graph = Some(graph);
        }
        self.history.target = None;
        self.history.pending = None;
        self.drawer_doc = None;
        self.drawer_show = None;
        self.active_drawer = Some(Drawer::Graph);
        self.tab = Tab::Scm;
        self.focus = Focus::Content;
    }
    pub(crate) fn history_sidebar_rows(&self) -> Vec<ScmRow> {
        let browser = &self.history.sidebar;
        let mut rows = vec![(
            None,
            format!(
                "{} Graph  [{}]",
                if self.drawer_open[Drawer::Graph.index()] {
                    "▾"
                } else {
                    "▸"
                },
                browser.scope.label()
            ),
            ' ',
            ScmTarget::Drawer(Drawer::Graph),
        )];
        if !self.drawer_open[Drawer::Graph.index()] {
            return rows;
        }
        for (index, row) in browser.graph.rows.iter().enumerate() {
            let graph = herdr_work_code::ansi::to_lines(&row.graph)[0].to_string();
            let expanded = self.history.expanded
                && row.commit.as_ref().map(|c| &c.oid) == browser.selected.as_ref();
            let label = row
                .commit
                .as_ref()
                .map(|c| format!("{graph}{} {}", if expanded { "▾" } else { "▸" }, c.subject))
                .unwrap_or(graph);
            rows.push((None, label, ' ', ScmTarget::HistoryCommit(index)));
            if expanded {
                match &browser.detail {
                    Some(Ok(d)) => {
                        rows.extend(d.files.iter().take(browser.file_limit).enumerate().map(
                            |(i, f)| {
                                (
                                    None,
                                    format!("{}    {}", browser.continuation(), f.compact_label()),
                                    f.status,
                                    ScmTarget::HistoryFile(i),
                                )
                            },
                        ));
                        if d.files.is_empty() {
                            rows.push((
                                None,
                                "  │   此提交没有文件变更".into(),
                                ' ',
                                ScmTarget::HistoryRetry,
                            ));
                        }
                        if d.files.len() > browser.file_limit {
                            rows.push((
                                None,
                                "  │   加载更多文件…".into(),
                                ' ',
                                ScmTarget::HistoryMoreFiles,
                            ));
                        }
                    }
                    Some(Err(e)) => rows.push((
                        None,
                        format!("  │   读取失败：{e}（点击重试）"),
                        ' ',
                        ScmTarget::HistoryRetry,
                    )),
                    None => rows.push((
                        None,
                        "  │   正在加载变更文件…".into(),
                        ' ',
                        ScmTarget::HistoryRetry,
                    )),
                }
            }
        }
        if let Some(e) = &browser.error {
            rows.push((
                None,
                format!("读取失败：{e}（点击重试）"),
                ' ',
                ScmTarget::HistoryRetry,
            ));
        } else if browser.graph.rows.is_empty() {
            rows.push((
                None,
                if browser.loading() {
                    "正在加载提交…"
                } else {
                    "暂无提交"
                }
                .into(),
                ' ',
                ScmTarget::HistoryRetry,
            ));
        }
        if browser
            .graph
            .rows
            .iter()
            .filter(|r| r.commit.is_some())
            .count()
            >= browser.limit
        {
            rows.push((None, "加载更多提交…".into(), ' ', ScmTarget::HistoryMore));
        }
        rows
    }
    pub(crate) fn activate_history_row(&mut self, target: ScmTarget, fold: bool) {
        match target {
            ScmTarget::HistoryCommit(index) => {
                let Some(c) = self.history.sidebar.graph.commit(index).cloned() else {
                    return;
                };
                if fold && self.history.sidebar.selected.as_ref() == Some(&c.oid) {
                    self.history.expanded = !self.history.expanded;
                } else {
                    self.history.sidebar.select(c.oid.clone(), 0);
                    self.history.expanded = true;
                }
                if !fold {
                    self.open_history_doc(self.history.sidebar.root.clone(), c.oid, 0, None, false);
                }
            }
            ScmTarget::HistoryFile(index) => {
                if let Some(d) = self.history.sidebar.details().cloned()
                    && let Some(f) = d.files.get(index)
                {
                    self.open_history_doc(
                        self.history.sidebar.root.clone(),
                        d.oid,
                        d.parent,
                        Some(f.path.clone()),
                        false,
                    );
                }
            }
            ScmTarget::HistoryMoreFiles => self.history.sidebar.file_limit += 100,
            ScmTarget::HistoryMore => {
                self.history.sidebar.limit += 100;
                self.history.sidebar.refresh();
            }
            ScmTarget::HistoryRetry => {
                if self.history.sidebar.error.is_some() {
                    self.history.sidebar.refresh();
                } else if let Some(oid) = self.history.sidebar.selected.clone() {
                    self.history.sidebar.select(oid, 0);
                }
            }
            _ => {}
        }
    }
    fn open_history_doc(
        &mut self,
        root: PathBuf,
        oid: String,
        parent: usize,
        path: Option<String>,
        from_graph: bool,
    ) {
        let target = (root.clone(), oid.clone(), parent, path.clone());
        if self.history.target.as_ref() == Some(&target) {
            return;
        }
        self.history.target = Some(target);
        self.history.from_graph = from_graph;
        let (tx, rx) = mpsc::channel();
        self.history.pending = Some(rx);
        self.drawer_request += 1;
        self.active_drawer = Some(Drawer::Graph);
        self.drawer_show = Some("history".into());
        self.drawer_doc = Some(herdr_work_code::preview::git_show(
            "提交详情".into(),
            String::new(),
            "正在读取…",
        ));
        std::thread::spawn(move || {
            let _ = tx.send(herdr_work_code::preview::commit::load(
                &root,
                &oid,
                parent,
                path.as_deref(),
            ));
        });
    }
    pub(crate) fn draw_history(&mut self, frame: &mut Frame, area: Rect) {
        let block = self.content_block(String::new());
        let body = block.inner(area);
        frame.render_widget(block, area);
        if let Some(doc) = &mut self.drawer_doc {
            doc.draw_body(frame, body);
        } else if let Some(graph) = &mut self.history.graph {
            graph.draw(frame, body);
        } else {
            frame.render_widget(
                Paragraph::new("选择提交查看详情，或点击 Git 工具栏打开大图"),
                body,
            );
        }
    }
    fn history_file(&mut self, file: FileTarget) {
        self.open_history_doc(file.root, file.oid, file.parent, Some(file.path), true);
    }
    pub(crate) fn history_key(&mut self, key: KeyEvent) -> bool {
        if self.tab != Tab::Scm
            || self.prompt.is_some()
            || self.overlay.is_some()
            || self.active_drawer != Some(Drawer::Graph)
            || self.focus != Focus::Content
        {
            return false;
        }
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
            return false;
        }
        if key.code == KeyCode::Backspace && self.history.target.is_some() {
            let (root, oid, parent, path) = self.history.target.take().unwrap();
            if path.is_some() && !self.history.from_graph {
                self.open_history_doc(root, oid, parent, None, false);
            } else {
                self.drawer_doc = None;
                self.drawer_show = None;
                self.history.pending = None;
            }
            return true;
        }
        if self.history.target.is_none() {
            if let Some(file) = self.history.graph.as_mut().and_then(|v| v.on_key(key)) {
                self.history_file(file);
            }
            return true;
        }
        if key.code == KeyCode::Enter
            && let Some((root, oid, parent, None)) = self.history.target.clone()
            && let Some(path) = self
                .drawer_doc
                .as_ref()
                .and_then(|d| d.commit_details())
                .and_then(|d| d.files.first())
                .map(|f| f.path.clone())
        {
            self.open_history_doc(root, oid, parent, Some(path), false);
            return true;
        }
        false
    }
    pub(crate) fn history_mouse(&mut self, event: MouseEvent, area: Rect) -> bool {
        let content = self.panes(area).content;
        if self.tab != Tab::Scm
            || self.active_drawer != Some(Drawer::Graph)
            || event.row == content.y
            || !content.contains((event.column, event.row).into())
        {
            return false;
        }
        self.focus = Focus::Content;
        if self.history.target.is_none() {
            if let Some(file) = self.history.graph.as_mut().and_then(|v| v.on_mouse(event)) {
                self.history_file(file);
            }
            return true;
        }
        let body = self.preview_body(content);
        if event.kind == MouseEventKind::Down(MouseButton::Left)
            && let Some(path) = self
                .drawer_doc
                .as_ref()
                .and_then(|d| d.commit_file_at(body, event.column, event.row))
                .map(|f| f.path.clone())
            && let Some((root, oid, parent, _)) = self.history.target.clone()
        {
            self.open_history_doc(root, oid, parent, Some(path), false);
            return true;
        }
        false
    }
}
