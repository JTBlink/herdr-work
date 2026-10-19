//! Activity and sidebar pointer actions share the geometry used by the renderer.
use crate::{
    layout::{ACTIVITIES, offset, panes},
    state::*,
};
use anyhow::Result;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

impl App {
    pub(crate) fn mouse(&mut self, event: MouseEvent, area: Rect) -> Result<()> {
        if self.prompt.is_some() {
            return Ok(());
        }
        let layout = panes(area, self.focus);
        let point = Position::new(event.column, event.row);
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            if layout.activity.contains(point) {
                let row = event.row.saturating_sub(layout.activity.y) as usize;
                if row % 4 < 3
                    && let Some((tab, _, _)) = ACTIVITIES.get(row / 4)
                {
                    self.switch_tab(*tab);
                }
            } else if layout.sidebar.contains(point) {
                if event.row == layout.sidebar.y {
                    return Ok(());
                }
                self.focus = Focus::Sidebar;
                if self.tab == Tab::Review {
                    self.switch_tab(Tab::Scm);
                }
                let row = event.row.saturating_sub(layout.sidebar.y + 1) as usize;
                let count = match self.tab {
                    Tab::Files => self.code.as_ref().map(|c| c.files.len()).unwrap_or(0),
                    Tab::Tasks => self.tasks.len(),
                    _ => self.code.as_ref().map(|c| c.changes.len()).unwrap_or(0),
                };
                let selected = if self.tab == Tab::Tasks {
                    row / 2
                        + self
                            .task
                            .saturating_sub(layout.sidebar.height.saturating_sub(4) as usize / 2)
                } else if self.tab == Tab::Scm {
                    let rows = self.scm_rows();
                    let position = rows
                        .iter()
                        .position(|r| r.0 == Some(self.selection))
                        .unwrap_or(0);
                    let index = row + offset(position, layout.sidebar.height);
                    let Some(selected) = rows.get(index).and_then(|r| r.0) else {
                        return Ok(());
                    };
                    selected
                } else {
                    row + offset(self.selection, layout.sidebar.height)
                };
                if selected < count && row < layout.sidebar.height.saturating_sub(2) as usize {
                    if self.tab == Tab::Tasks {
                        self.task = selected;
                    } else {
                        self.selection = selected;
                    }
                    self.action(KeyCode::Enter)?;
                }
            } else if layout.content.contains(point) {
                self.focus = Focus::Content;
                if self.tab == Tab::Scm && self.diff.is_some() {
                    self.switch_tab(Tab::Review);
                }
            }
        } else if matches!(
            event.kind,
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
        ) {
            if layout.content.contains(point) {
                self.focus = Focus::Content;
                if self.tab == Tab::Scm && self.diff.is_some() {
                    self.scm_selection = self.selection;
                    self.switch_tab(Tab::Review);
                }
            } else if layout.sidebar.contains(point) {
                self.focus = Focus::Sidebar;
                if self.tab == Tab::Review {
                    self.switch_tab(Tab::Scm);
                }
            } else {
                return Ok(());
            }
            let key = if event.kind == MouseEventKind::ScrollDown {
                KeyCode::Down
            } else {
                KeyCode::Up
            };
            self.action(key)?;
        }
        Ok(())
    }
}
