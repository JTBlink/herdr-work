//! Sidebar drawer navigation, backed by its extracted shared model and queries.
use crate::{loading::LoadState, state::*};
use anyhow::{Context, Result};
use herdr_work_code::scm::Drawer;
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

#[derive(Clone, Copy)]
pub(crate) enum ScmTarget {
    Commit,
    Section,
    Change(usize),
    Drawer(Drawer),
    History(Drawer, usize),
}
pub(crate) type ScmRow = (Option<usize>, String, char, ScmTarget);
impl App {
    pub(crate) fn load_drawer(&mut self, kind: Drawer) -> Result<()> {
        let preserve_context = self.active_drawer == Some(kind) && self.focus == Focus::Content;
        self.drawer_request += 1;
        let request = self.drawer_request;
        self.active_drawer = Some(kind);
        self.drawer_open[kind.index()] = true;
        self.drawer_state = LoadState::Loading;
        self.drawer_scroll = 0;
        self.drawer_show = None;
        let root = self
            .code
            .as_ref()
            .and_then(|c| c.changes.get(self.selection))
            .map(|c| c.root.clone())
            .unwrap_or_else(|| self.root.clone());
        let file = self
            .code
            .as_ref()
            .and_then(|c| c.changes.get(self.selection))
            .filter(|c| c.root == root)
            .map(|c| c.path.clone());
        // Reopening an already browsed drawer retains its immutable query context.
        let (root, file) = self
            .drawers
            .get(&kind)
            .filter(|_| preserve_context)
            .map(|v| (v.root.clone(), v.file.clone()))
            .unwrap_or((root, file));
        self.job(move |d| {
            Ok(Data::Drawer(
                request,
                d.scm_drawer(&root, kind, file.as_deref()),
            ))
        })
    }
    pub(crate) fn activate_scm_row(&mut self) -> Result<()> {
        let rows = self.scm_rows();
        let Some(row) = rows.get(self.scm_cursor) else {
            return Ok(());
        };
        match row.3 {
            ScmTarget::Commit => self.prepare_commit()?,
            ScmTarget::Change(index) => {
                self.selection = index;
                self.scm_selection = index;
                self.active_drawer = None;
                let change = self
                    .code
                    .as_ref()
                    .and_then(|c| c.changes.get(index))
                    .context("Git 更改已失效")?
                    .clone();
                self.open_diff(
                    change.root,
                    change.path,
                    if change.staged { "staged" } else { "unstaged" }.into(),
                )?;
            }
            ScmTarget::Drawer(kind) => {
                if self.drawer_open[kind.index()] && self.active_drawer == Some(kind) {
                    self.drawer_open[kind.index()] = false;
                    self.active_drawer = None;
                } else {
                    self.load_drawer(kind)?;
                }
            }
            ScmTarget::History(kind, index) => {
                let view = self.drawers.get(&kind).context("Git 列表尚未加载")?.clone();
                self.drawer_request += 1;
                let request = self.drawer_request;
                self.job(move |d| Ok(Data::Show(request, d.scm_show(&view, index))))?;
            }
            ScmTarget::Section => {}
        }
        Ok(())
    }
    pub(crate) fn prepare_commit(&self) -> Result<()> {
        let root = self
            .code
            .as_ref()
            .and_then(|c| c.changes.get(self.selection))
            .map(|c| c.root.clone())
            .unwrap_or_else(|| self.root.clone());
        self.job(move |d| Ok(Data::CommitReady(d.prepare_commit(&root)?)))
    }
    pub(crate) fn draw_drawer(&self, f: &mut Frame, area: Rect) {
        let Some(kind) = self.active_drawer else {
            return;
        };
        let title = format!(
            "{} · {}",
            kind.title(),
            self.drawers
                .get(&kind)
                .map(|v| v.root.display().to_string())
                .unwrap_or_default()
        );
        let text = if let Some(text) = &self.drawer_show {
            text.clone()
        } else {
            match &self.drawer_state {
                LoadState::Loading => "正在加载 Git 数据…".into(),
                LoadState::Failed(error) => format!("加载失败：{error}\n按 r 重试"),
                LoadState::Ready => self
                    .drawers
                    .get(&kind)
                    .map(|v| {
                        if v.lines.is_empty() {
                            "(none)".into()
                        } else {
                            v.lines.join("\n")
                        }
                    })
                    .unwrap_or_default(),
            }
        };
        f.render_widget(
            Paragraph::new(ratatui::text::Text::from(herdr_work_code::ansi::to_lines(
                &text,
            )))
            .scroll((self.drawer_scroll, 0))
            .block(self.content_block(title)),
            area,
        );
    }
}
