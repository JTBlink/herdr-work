//! Compact history interaction; rendering and async queries stay out of the SCM shell.
use super::*;
use crate::history::{Browser, display};

impl App {
    pub(super) fn refresh_history(&mut self) {
        let Some(root) = self.active_repo().map(|repo| repo.git.root().to_path_buf()) else {
            return;
        };
        if self.history.root != root {
            self.history = Browser::new(root);
            self.history_expanded = false;
        }
        if !self.history.loading() {
            self.history.refresh();
        }
    }
    pub(super) fn poll_history(&mut self) {
        if !self.history.poll() {
            return;
        }
        let selected = self.selected.and_then(|row| self.row_stable_id(row));
        let top = self.row_stable_id(self.scroll);
        let panel = &mut self.drawers[Drawer::Graph.index()];
        panel.lines = self.history.graph.legacy_lines();
        panel.refs = self
            .history
            .graph
            .rows
            .iter()
            .map(|row| {
                row.commit
                    .as_ref()
                    .map(|c| DrawerRef::Commit(c.oid.clone()))
                    .unwrap_or_default()
            })
            .collect();
        self.rebuild();
        if let Some(row) = selected.and_then(|id| self.find_row_by_stable_id(&id)) {
            self.selected = Some(row);
        }
        if let Some(row) = top.and_then(|id| self.find_row_by_stable_id(&id)) {
            self.scroll = row;
        }
    }
    pub(super) fn history_rows(&self) -> Vec<Row> {
        let mut rows = vec![];
        for (index, row) in self.history.graph.rows.iter().enumerate() {
            rows.push(Row::DrawerLine(Drawer::Graph, index));
            if !self.history_expanded
                || row.commit.as_ref().map(|c| &c.oid) != self.history.selected.as_ref()
            {
                continue;
            }
            match &self.history.detail {
                Some(Ok(details)) if !details.files.is_empty() => {
                    rows.extend(
                        (0..details.files.len().min(self.history.file_limit)).map(Row::HistoryFile),
                    );
                    if details.files.len() > self.history.file_limit {
                        rows.push(Row::HistoryMoreFiles);
                    }
                }
                _ => rows.push(Row::HistoryNotice),
            }
        }
        if self.history.error.is_some() || (rows.is_empty()) {
            rows.push(Row::HistoryNotice);
        }
        if self
            .history
            .graph
            .rows
            .iter()
            .filter(|r| r.commit.is_some())
            .count()
            >= self.history.limit
        {
            rows.push(Row::HistoryMore);
        }
        rows
    }
    pub(super) fn history_item(&self, row: Row, width: usize) -> ListItem<'static> {
        let line = match row {
            Row::DrawerLine(Drawer::Graph, index) => {
                let Some(row) = self.history.graph.rows.get(index) else {
                    return ListItem::new("");
                };
                let mut spans = self.history.graph.compact_gutter(&row.graph, width).spans;
                if let Some(commit) = &row.commit {
                    let expanded = self.history_expanded
                        && self.history.selected.as_ref() == Some(&commit.oid);
                    spans.push(Span::raw(format!(
                        "{} {}",
                        if expanded { "▾" } else { "▸" },
                        commit.subject
                    )));
                    if !commit.refs.is_empty() {
                        spans.push(Span::styled(
                            format!(
                                " [{}]",
                                commit
                                    .refs
                                    .join(", ")
                                    .replace("refs/heads/", "")
                                    .replace("refs/remotes/", "")
                            ),
                            Style::default().fg(palette().header_accent),
                        ));
                    }
                }
                return ListItem::new(Line::from(crate::git_graph::clip_spans(spans, width)));
            }
            Row::HistoryFile(index) => {
                let Some(file) = self.history.details().and_then(|d| d.files.get(index)) else {
                    return ListItem::new("");
                };
                let mut spans = self
                    .history
                    .graph
                    .compact_gutter(&self.history.continuation(), width)
                    .spans;
                spans.push(Span::raw("    "));
                spans.push(Span::styled(
                    file.compact_label(),
                    Style::default().fg(crate::ui::status_color(file.status)),
                ));
                return ListItem::new(Line::from(crate::git_graph::clip_spans(spans, width)));
            }
            Row::HistoryMoreFiles => format!(
                "  │   加载更多文件（共 {} 个）",
                self.history.details().map_or(0, |d| d.files.len())
            ),
            Row::HistoryMore => "  加载更多提交…".into(),
            _ => match &self.history.error {
                Some(e) => format!("  加载失败：{}（点击重试）", display(e)),
                None => match &self.history.detail {
                    Some(Err(e)) => format!("  │   读取失败：{}（点击重试）", display(e)),
                    Some(Ok(_)) if self.history_expanded => "  │   此提交没有文件变更".into(),
                    _ if self.history.loading() || self.history.selected.is_some() => {
                        "  │   正在加载…".into()
                    }
                    _ => "  暂无提交".into(),
                },
            },
        };
        ListItem::new(truncate_to(line, width))
    }
    pub(super) fn activate_history(&mut self, row: Row, fold_only: bool) {
        match row {
            Row::DrawerLine(Drawer::Graph, index) => {
                let Some(commit) = self.history.graph.commit(index).cloned() else {
                    return;
                };
                if fold_only && self.history.selected.as_ref() == Some(&commit.oid) {
                    self.history_expanded = !self.history_expanded;
                } else {
                    self.history.select(commit.oid.clone(), 0);
                    self.history_expanded = true;
                }
                if !fold_only {
                    self.open_commit(&commit.oid, 0, None);
                }
            }
            Row::HistoryFile(index) => {
                if let Some(d) = self.history.details().cloned()
                    && let Some(file) = d.files.get(index)
                {
                    self.open_commit(&d.oid, d.parent, Some(&file.path));
                }
            }
            Row::HistoryMoreFiles => self.history.file_limit += 100,
            Row::HistoryMore => {
                self.history.limit += 100;
                self.history.refresh();
            }
            Row::HistoryNotice => {
                if self.history.error.is_some() {
                    self.history.refresh();
                } else if let Some(oid) = self.history.selected.clone() {
                    self.history.select(oid, 0);
                }
            }
            _ => {}
        }
        self.rebuild();
    }
    pub(super) fn open_commit(&mut self, oid: &str, parent: usize, path: Option<&str>) {
        let Some(pane_id) = self.pane_ctl.as_ref().map(|c| c.pane_id.clone()) else {
            self.flash = Notice::new(("preview needs a herdr pane".into(), true));
            return;
        };
        let root = &self.history.root;
        let key = crate::viewer::doc_key_for_commit(root, oid, parent, path);
        let payload = crate::viewer::commit_request(root, oid, parent, path);
        match crate::viewer::open_in_pane(&pane_id, root, &key, &payload) {
            Ok(target) => self.last_preview = Some((key, target)),
            Err(error) => self.flash = Notice::new((error, true)),
        }
    }
    pub(super) fn history_key(&self, row: Row) -> Option<String> {
        let (oid, parent, path) = match row {
            Row::DrawerLine(Drawer::Graph, index) => {
                (self.history.graph.commit(index)?.oid.as_str(), 0, None)
            }
            Row::HistoryFile(index) => {
                let d = self.history.details()?;
                (
                    d.oid.as_str(),
                    d.parent,
                    Some(d.files.get(index)?.path.as_str()),
                )
            }
            _ => return None,
        };
        Some(crate::viewer::doc_key_for_commit(
            &self.history.root,
            oid,
            parent,
            path,
        ))
    }
}
