//! Activity and sidebar pointer actions share the geometry used by the renderer.
use crate::{
    layout::{ACTIVITIES, offset, settings_button},
    state::*,
};
use anyhow::Result;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

impl App {
    pub(crate) fn mouse(&mut self, event: MouseEvent, area: Rect) -> Result<()> {
        if self.overlay.is_some() {
            return self.overlay_mouse(event, area);
        }
        if self.prompt.is_some() && !self.search_prompt() {
            return Ok(());
        }
        if self.preview_mouse(&event, area) {
            return Ok(());
        }
        let layout = self.panes(area);
        let point = Position::new(event.column, event.row);
        let editor_body = self.content_block(String::new()).inner(layout.content);
        if let Some(session) = &mut self.editor {
            if layout.content.contains(point) {
                let delta = match event.kind {
                    MouseEventKind::ScrollDown => 3,
                    MouseEventKind::ScrollUp => -3,
                    _ => 0,
                };
                if delta != 0 {
                    session.editor.scroll_by(
                        delta,
                        session.editor.wrap_width(editor_body.width),
                        editor_body.height as usize,
                    );
                } else {
                    session.editor.on_mouse(&event, editor_body);
                }
            }
            return Ok(());
        }
        if self.search_prompt()
            && !layout.sidebar.contains(point)
            && matches!(event.kind, MouseEventKind::Down(_))
        {
            self.prompt = None;
            self.input.clear();
        }
        let right = event.kind == MouseEventKind::Down(MouseButton::Right);
        if event.kind == MouseEventKind::Down(MouseButton::Left) || right {
            if settings_button(layout.activity).contains(point) {
                self.open_settings();
                return Ok(());
            }
            if layout.activity.contains(point) {
                let row = event.row.saturating_sub(layout.activity.y) as usize;
                if row % 3 < 3
                    && let Some((tab, _, _)) = ACTIVITIES.get(row / 3)
                {
                    self.switch_tab(*tab);
                }
            } else if layout.sidebar.contains(point) {
                if event.row == layout.sidebar.y {
                    self.open_menu(false);
                    return Ok(());
                }
                self.focus = Focus::Sidebar;
                if self.tab == Tab::Review {
                    self.switch_tab(Tab::Scm);
                }
                let mut row = event.row.saturating_sub(layout.sidebar.y + 1) as usize;
                if self.tab == Tab::Search {
                    if row < 4 {
                        match row {
                            0 => self.query_prompt(self.search_quick),
                            1 if !self.search_quick => {
                                self.prompt = None;
                                let x = event.column.saturating_sub(layout.sidebar.x + 1);
                                match x {
                                    0..=5 => {
                                        self.search_options.match_case =
                                            !self.search_options.match_case
                                    }
                                    7..=11 => {
                                        self.search_options.whole_word =
                                            !self.search_options.whole_word
                                    }
                                    13..=18 => {
                                        self.search_options.regex = !self.search_options.regex
                                    }
                                    _ => return Ok(()),
                                }
                                self.search()?;
                            }
                            2 | 3 if !self.search_quick => {
                                self.prompt = Some(if row == 2 {
                                    Prompt::SearchInclude
                                } else {
                                    Prompt::SearchExclude
                                });
                                self.input = if row == 2 {
                                    self.search_include.clone()
                                } else {
                                    self.search_exclude.clone()
                                };
                            }
                            _ => {}
                        }
                        return Ok(());
                    }
                    self.prompt = None;
                    self.input.clear();
                    row -= 4;
                }
                if self.tab == Tab::Scm {
                    let rows = self.scm_rows();
                    let index = row + offset(self.scm_cursor, layout.sidebar.height);
                    if row < layout.sidebar.height.saturating_sub(2) as usize
                        && let Some(entry) = rows.get(index)
                    {
                        self.scm_cursor = index;
                        if let Some(selected) = entry.0 {
                            self.selection = selected;
                            self.scm_selection = selected;
                        }
                        if right {
                            self.open_menu(true);
                        } else {
                            self.activate_scm_row()?;
                        }
                    }
                    return Ok(());
                }
                let count = match self.tab {
                    Tab::Files => self.code.as_ref().map(|c| c.files.len()).unwrap_or(0),
                    Tab::Tasks => self.tasks.len(),
                    Tab::Search => self.search_rows.len(),
                    _ => self.code.as_ref().map(|c| c.changes.len()).unwrap_or(0),
                };
                let selected = if self.tab == Tab::Tasks {
                    row / 2
                        + self
                            .task
                            .saturating_sub(layout.sidebar.height.saturating_sub(4) as usize / 2)
                } else {
                    row + offset(
                        self.selection,
                        if self.tab == Tab::Search {
                            layout.sidebar.height.saturating_sub(6)
                        } else {
                            layout.sidebar.height
                        },
                    )
                };
                if selected < count && row < layout.sidebar.height.saturating_sub(2) as usize {
                    if self.tab == Tab::Tasks {
                        self.task = selected;
                    } else {
                        self.selection = selected;
                    }
                    if right {
                        self.open_menu(true);
                    } else {
                        self.action(KeyCode::Enter)?;
                        if matches!(self.tab, Tab::Files | Tab::Search) {
                            self.note_preview_click();
                        }
                    }
                }
            } else if layout.content.contains(point) {
                self.focus = Focus::Content;
                if self.tab == Tab::Scm && self.diff.is_some() && self.active_drawer.is_none() {
                    self.switch_tab(Tab::Review);
                }
            }
        } else if matches!(
            event.kind,
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
        ) {
            if layout.content.contains(point) {
                self.focus = Focus::Content;
                if self.tab == Tab::Scm && self.diff.is_some() && self.active_drawer.is_none() {
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
