//! Activity and sidebar pointer actions share the geometry used by the renderer.
use crate::{
    layout::{ACTIVITIES, activity_button, offset, settings_button, task_offset},
    state::*,
};
use anyhow::Result;
use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

impl App {
    pub(crate) fn mouse(&mut self, event: MouseEvent, area: Rect) -> Result<()> {
        self.interaction.area = area;
        self.interaction.pointer = Some(Position::new(event.column, event.row));
        if matches!(
            event.kind,
            MouseEventKind::Down(_) | MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
        ) {
            self.interaction.user_revision = self.interaction.user_revision.wrapping_add(1);
            self.interaction.restore = None;
            self.interaction.pending_scm_key = None;
        }
        if let Some(picker) = self.git.picker.as_mut() {
            let action = picker.mouse(event);
            return self.picker_action(action);
        }
        if self.overlay.is_some() {
            return self.overlay_mouse(event, area);
        }
        if self.prompt.is_some() && !self.search_prompt() {
            return Ok(());
        }
        if event.kind == MouseEventKind::Down(MouseButton::Left)
            && self
                .preview_close_button(self.panes(area).content)
                .is_some_and(|rect| rect.contains((event.column, event.row).into()))
        {
            self.close_document()?;
            return Ok(());
        }
        if self.footer_mouse(&event)? {
            return Ok(());
        }
        let old_hover = self.hover.point;
        let title_zones = self.sidebar_title_actions(self.panes(area).sidebar).1;
        self.track_sidebar_mouse(Position::new(event.column, event.row), area);
        if event.kind == MouseEventKind::Down(MouseButton::Left)
            && let Some((_, action)) = title_zones
                .iter()
                .find(|(rect, _)| rect.contains(Position::new(event.column, event.row)))
        {
            return self.sidebar_title_action(*action);
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
                if let Some((_, (tab, _, _))) = ACTIVITIES
                    .iter()
                    .enumerate()
                    .find(|(index, _)| activity_button(layout.activity, *index).contains(point))
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
                let mut body = self.sidebar_block(String::new()).inner(layout.sidebar);
                if !body.contains(point) {
                    return Ok(());
                }
                let mut row = event.row.saturating_sub(body.y) as usize;
                if self.tab == Tab::Search {
                    let search = self.search_layout(layout.sidebar);
                    if search.header.contains(point) {
                        if !self.search_quick && search.details.contains(point) {
                            self.search_details = !self.search_filters_visible();
                            self.prompt = None;
                            self.input.clear();
                        } else if let Some(index) =
                            search.options.iter().position(|rect| rect.contains(point))
                        {
                            match index {
                                0 => {
                                    self.search_options.match_case = !self.search_options.match_case
                                }
                                1 => {
                                    self.search_options.whole_word = !self.search_options.whole_word
                                }
                                _ => self.search_options.regex = !self.search_options.regex,
                            }
                            self.search()?;
                        } else if self.search_filters_visible()
                            && (search.include.contains(point) || search.exclude.contains(point))
                        {
                            let include = search.include.contains(point);
                            self.prompt = Some(if include {
                                Prompt::SearchInclude
                            } else {
                                Prompt::SearchExclude
                            });
                            self.input = if include {
                                self.search_include.clone()
                            } else {
                                self.search_exclude.clone()
                            };
                        } else if search.query_box.contains(point) {
                            self.query_prompt(self.search_quick);
                        }
                        return Ok(());
                    }
                    self.prompt = None;
                    self.input.clear();
                    body = search.body;
                    row = event.row.saturating_sub(body.y) as usize;
                }
                if self.tab == Tab::Scm {
                    let layout = self.scm_layout(body);
                    if let Some((index, line)) = layout.hit(body, point)
                        && let Some(entry) = layout.rows.get(index)
                    {
                        self.git.editing = None;
                        self.scm_cursor = index;
                        self.git.active = Some(self.selected_repository());
                        if let Some(selected) = entry.0 {
                            self.selection = selected;
                            self.scm_selection = selected;
                        }
                        if !right
                            && line == 1
                            && matches!(entry.3, crate::scm::ScmTarget::Message(_))
                            && event.column >= body.right().saturating_sub(4)
                            && event.column < body.right().saturating_sub(1)
                        {
                            let root = self.selected_repository();
                            self.ensure_draft(&root)?;
                            self.suggest_commit()?;
                            return Ok(());
                        }
                        if right {
                            self.open_menu(true);
                        } else if !((old_hover.is_some_and(|p| body.contains(p) && p.y == point.y)
                            || matches!(entry.3, crate::scm::ScmTarget::Repository(_)))
                            && self.scm_hover_action(index, event.column - body.x, body.width)?)
                        {
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
                    if row >= (body.height / 2) as usize * 2 {
                        return Ok(());
                    }
                    row / 2 + task_offset(self.task, body.height)
                } else {
                    row + offset(self.selection, body.height)
                };
                if selected < count && row < body.height as usize {
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
