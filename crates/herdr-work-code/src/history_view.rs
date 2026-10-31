//! Full Git Graph component. Owns no terminal, input loop or host lifecycle.
use crate::{
    history::{Browser, CommitDetails},
    preview::{self, Doc},
    ui::{palette, selection_style},
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};
use std::path::PathBuf;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTarget {
    pub root: PathBuf,
    pub oid: String,
    pub parent: usize,
    pub path: String,
}

pub struct GraphView {
    pub browser: Browser,
    pub initialized: bool,
    pub selected: Option<usize>,
    pub scroll: usize,
    pub detail: Option<Doc>,
    show_detail: bool,
    detail_focus: bool,
    graph_area: Rect,
    detail_area: Rect,
    scope_area: Rect,
    close_detail: Rect,
    more_area: Rect,
}
impl GraphView {
    pub fn new(root: PathBuf) -> Self {
        let mut browser = Browser::new(root);
        browser.limit = 200;
        Self {
            browser,
            initialized: false,
            selected: None,
            scroll: 0,
            detail: None,
            show_detail: false,
            detail_focus: false,
            graph_area: Rect::default(),
            detail_area: Rect::default(),
            scope_area: Rect::default(),
            close_detail: Rect::default(),
            more_area: Rect::default(),
        }
    }
    pub fn refresh(&mut self) {
        self.browser.refresh();
    }
    pub fn tick(&mut self) -> bool {
        if !self.browser.poll() {
            return false;
        }
        self.initialized = true;
        if let Some(oid) = &self.browser.selected {
            self.selected = self
                .browser
                .graph
                .rows
                .iter()
                .position(|r| r.commit.as_ref().is_some_and(|c| &c.oid == oid));
        } else {
            self.selected = None;
            self.show_detail = false;
        }
        if self.detail.as_ref().and_then(|doc| doc.commit_details()) != self.browser.details() {
            self.detail = self
                .browser
                .details()
                .cloned()
                .map(|d| preview::commit::summary(&self.browser.root, d));
        }
        true
    }
    pub fn select(&mut self, index: usize) {
        let Some(commit) = self.browser.graph.commit(index) else {
            return;
        };
        self.selected = Some(index);
        self.show_detail = true;
        if self.browser.selected.as_ref() != Some(&commit.oid) {
            self.detail = None;
        }
        let parent = self
            .browser
            .details()
            .filter(|d| d.oid == commit.oid)
            .map_or(0, |d| d.parent);
        self.browser.select(commit.oid.clone(), parent);
    }
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        self.graph_area = Rect::default();
        self.detail_area = Rect::default();
        self.close_detail = Rect::default();
        self.more_area = Rect::default();
        if area.height == 0 {
            return;
        }
        self.scope_area = Rect::new(area.x, area.y, area.width.min(40), 1);
        frame.render_widget(
            Paragraph::new(format!(
                "[{}]  s 范围 · r 刷新 · Enter 详情",
                self.browser.scope.label()
            ))
            .style(Style::default().fg(palette().header_accent)),
            self.scope_area,
        );
        let body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );
        let detail_height = if self.show_detail {
            if body.height < 16 {
                body.height
            } else {
                (body.height / 2).max(8)
            }
        } else {
            0
        };
        let graph_height = body.height.saturating_sub(detail_height);
        if graph_height > 0 {
            frame.render_widget(
                Paragraph::new(self.browser.graph.header(body.width)),
                Rect::new(body.x, body.y, body.width, 1),
            );
            self.graph_area = Rect::new(
                body.x,
                body.y + 1,
                body.width,
                graph_height.saturating_sub(1),
            );
            let more = self
                .browser
                .graph
                .rows
                .iter()
                .filter(|r| r.commit.is_some())
                .count()
                >= self.browser.limit;
            if more && self.graph_area.height > 1 {
                self.graph_area.height -= 1;
                self.more_area = Rect::new(body.x, self.graph_area.bottom(), body.width, 1);
                frame.render_widget(
                    Paragraph::new("[加载更多提交]  L")
                        .style(Style::default().fg(palette().header_accent)),
                    self.more_area,
                );
            }
            self.scroll = self
                .scroll
                .min(self.browser.graph.rows.len().saturating_sub(1));
            let rows = self
                .browser
                .graph
                .lines(body.width)
                .into_iter()
                .enumerate()
                .skip(self.scroll)
                .map(|(i, line)| {
                    if self.selected == Some(i) {
                        line.style(selection_style(!self.detail_focus))
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>();
            frame.render_widget(Paragraph::new(rows), self.graph_area);
            if let Some(error) = &self.browser.error {
                frame.render_widget(
                    Paragraph::new(format!("加载失败：{error} · r 重试")),
                    self.graph_area,
                );
            } else if self.browser.loading() && self.browser.graph.rows.is_empty() {
                frame.render_widget(Paragraph::new("正在加载提交…"), self.graph_area);
            }
        }
        if detail_height > 0 {
            let title = Rect::new(body.x, body.y + graph_height, body.width, 1);
            self.close_detail = Rect::new(
                title.right().saturating_sub(title.width.min(12)),
                title.y,
                title.width.min(12),
                1,
            );
            frame.render_widget(
                Paragraph::new("─ 提交详情 · Tab 切换 · ↑↓ 滚动 · Enter 打开文件 · P 父提交")
                    .style(Style::default().fg(palette().header_accent).bold()),
                title,
            );
            frame.render_widget(
                Paragraph::new("[返回图谱]").style(Style::default().fg(palette().header_accent)),
                self.close_detail,
            );
            self.detail_area = Rect::new(
                body.x,
                title.y + 1,
                body.width,
                detail_height.saturating_sub(1),
            );
            if let Some(doc) = &mut self.detail {
                doc.draw_body(frame, self.detail_area);
            } else {
                frame.render_widget(
                    Paragraph::new(match &self.browser.detail {
                        Some(Err(e)) => format!("读取失败：{e} · r 重试"),
                        _ => "正在读取提交详情…".into(),
                    }),
                    self.detail_area,
                );
            }
        }
    }
    pub fn target(&self, path: &str) -> Option<FileTarget> {
        let d = self.browser.details()?;
        Some(FileTarget {
            root: self.browser.root.clone(),
            oid: d.oid.clone(),
            parent: d.parent,
            path: path.into(),
        })
    }
    pub fn on_mouse(&mut self, event: MouseEvent) -> Option<FileTarget> {
        let point = (event.column, event.row).into();
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            if self.close_detail.contains(point) {
                self.show_detail = false;
                self.detail_focus = false;
            } else if self.scope_area.contains(point) {
                self.browser.scope.toggle();
                self.refresh();
            } else if self.more_area.contains(point) {
                self.browser.limit += 200;
                self.refresh();
            } else if self.graph_area.contains(point) {
                self.detail_focus = false;
                self.select(self.scroll + usize::from(event.row - self.graph_area.y));
            } else if self.detail_area.contains(point) {
                self.detail_focus = true;
                if let Some(file) = self
                    .detail
                    .as_ref()
                    .and_then(|doc| doc.commit_file_at(self.detail_area, event.column, event.row))
                {
                    return self.target(&file.path);
                }
            }
        }
        if self.detail_area.contains(point) {
            if let Some(doc) = &mut self.detail {
                match event.kind {
                    MouseEventKind::ScrollUp => doc.set_scroll(doc.scroll().saturating_sub(3)),
                    MouseEventKind::ScrollDown => doc.set_scroll(doc.scroll() + 3),
                    _ => {
                        doc.on_mouse(&event, self.detail_area);
                    }
                }
            }
        } else {
            match event.kind {
                MouseEventKind::ScrollUp => self.scroll = self.scroll.saturating_sub(3),
                MouseEventKind::ScrollDown => self.scroll = self.scroll.saturating_add(3),
                _ => {}
            }
        }
        None
    }
    pub fn on_key(&mut self, event: KeyEvent) -> Option<FileTarget> {
        match event.code {
            KeyCode::Tab => self.detail_focus = !self.detail_focus,
            KeyCode::Backspace => {
                self.show_detail = false;
                self.detail_focus = false;
            }
            KeyCode::Char('s') => {
                self.browser.scope.toggle();
                self.refresh();
            }
            KeyCode::Char('L') => {
                self.browser.limit += 200;
                self.refresh();
            }
            KeyCode::Char('r') => {
                self.refresh();
                if let Some(oid) = self.browser.selected.clone() {
                    let parent = self.browser.details().map_or(0, |d| d.parent);
                    self.browser.detail = None;
                    self.browser.select(oid, parent);
                }
            }
            KeyCode::Char('P') => {
                if let Some(d) = self.browser.details() {
                    let oid = d.oid.clone();
                    let parent = (d.parent + 1) % d.parents.len().max(1);
                    self.detail = None;
                    self.browser.select(oid, parent);
                }
            }
            KeyCode::Enter if self.detail_focus => {
                if let Some(doc) = &self.detail {
                    let d = doc.commit_details()?;
                    let index = doc
                        .top_src()
                        .saturating_sub(d.lines().1)
                        .min(d.files.len().saturating_sub(1));
                    let path = d.files.get(index)?.path.clone();
                    return self.target(&path);
                }
            }
            KeyCode::Down
            | KeyCode::Char('j')
            | KeyCode::Up
            | KeyCode::Char('k')
            | KeyCode::PageDown
            | KeyCode::PageUp => {
                let down = matches!(
                    event.code,
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::PageDown
                );
                let step = if matches!(event.code, KeyCode::PageDown | KeyCode::PageUp) {
                    usize::from(self.graph_area.height.max(1))
                } else {
                    1
                };
                if self.detail_focus {
                    if let Some(doc) = &mut self.detail {
                        doc.set_scroll(if down {
                            doc.scroll() + step
                        } else {
                            doc.scroll().saturating_sub(step)
                        });
                    }
                } else {
                    let start = self.selected.unwrap_or(self.scroll);
                    let next = if self.selected.is_none() {
                        (start..self.browser.graph.rows.len())
                            .find(|i| self.browser.graph.commit(*i).is_some())
                    } else if down {
                        ((start + step)..self.browser.graph.rows.len())
                            .find(|i| self.browser.graph.commit(*i).is_some())
                    } else {
                        (0..start.saturating_sub(step - 1))
                            .rev()
                            .find(|i| self.browser.graph.commit(*i).is_some())
                    };
                    if let Some(i) = next {
                        self.select(i);
                        self.keep_visible();
                    }
                }
            }
            KeyCode::Home => {
                self.scroll = 0;
                if let Some(i) = (0..self.browser.graph.rows.len())
                    .find(|i| self.browser.graph.commit(*i).is_some())
                {
                    self.select(i);
                }
            }
            KeyCode::End => {
                if let Some(i) = (0..self.browser.graph.rows.len())
                    .rev()
                    .find(|i| self.browser.graph.commit(*i).is_some())
                {
                    self.select(i);
                    self.keep_visible();
                }
            }
            KeyCode::Enter => {
                if let Some(i) = self.selected.or_else(|| {
                    (self.scroll..self.browser.graph.rows.len())
                        .find(|i| self.browser.graph.commit(*i).is_some())
                }) {
                    self.select(i);
                }
            }
            _ => {}
        }
        None
    }
    fn keep_visible(&mut self) {
        if let Some(i) = self.selected {
            if i < self.scroll {
                self.scroll = i;
            } else if i >= self.scroll + usize::from(self.graph_area.height) {
                self.scroll =
                    i.saturating_sub(usize::from(self.graph_area.height).saturating_sub(1));
            }
        }
    }
    pub fn details(&self) -> Option<&CommitDetails> {
        self.browser.details()
    }
}
