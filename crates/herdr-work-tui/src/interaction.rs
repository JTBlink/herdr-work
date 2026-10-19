//! Product navigation and interaction snapshots; source algorithms stay shared.
use crate::state::*;
use anyhow::Result;
use crossterm::event::KeyCode;
use herdr_work_code::tree::{Navigation, navigate};
use ratatui::layout::Rect;

pub(crate) struct Interaction {
    pub area: Rect,
}
impl Default for Interaction {
    fn default() -> Self {
        Self {
            area: Rect::new(0, 0, 80, 24),
        }
    }
}
impl App {
    pub(crate) fn navigate_key(&mut self, key: KeyCode) -> Result<bool> {
        if self.focus != Focus::Sidebar {
            return Ok(false);
        }
        if self.tab == Tab::Files && matches!(key, KeyCode::Left | KeyCode::Right) {
            let Some(code) = &self.code else {
                return Ok(true);
            };
            let Some(row) = code.files.get(self.selection) else {
                return Ok(true);
            };
            match navigate(
                &code.files,
                self.selection,
                self.expanded.contains(&row.path),
                key == KeyCode::Right,
            ) {
                Navigation::Select(index) => self.selection = index,
                Navigation::Expand => {
                    self.expanded.push(row.path.clone());
                    self.refresh()?;
                }
                Navigation::Collapse => {
                    self.expanded.retain(|path| path != &row.path);
                    self.refresh()?;
                }
                Navigation::None => {}
            }
            return Ok(true);
        }
        if self.tab == Tab::Scm && matches!(key, KeyCode::Left | KeyCode::Right) {
            use crate::scm::ScmTarget;
            let rows = self.scm_rows();
            if let Some(row) = rows.get(self.scm_cursor) {
                match &row.3 {
                    ScmTarget::Directory(index, staged, path) => {
                        let root = self.repository_root(*index)?;
                        let collapsed = self.git.collapsed.entry((root, *staged)).or_default();
                        if key == KeyCode::Right {
                            collapsed.remove(path);
                        } else {
                            collapsed.insert(path.clone());
                        }
                    }
                    _ if key == KeyCode::Left && self.git.tree => {
                        let depths = rows
                            .iter()
                            .map(|row| match &row.3 {
                                ScmTarget::Directory(_, _, path) => {
                                    (path.split('/').count().saturating_sub(1), true)
                                }
                                ScmTarget::Change(index) => (
                                    self.code.as_ref().unwrap().changes[*index]
                                        .path
                                        .split('/')
                                        .count()
                                        .saturating_sub(1),
                                    false,
                                ),
                                _ => (usize::MAX, false),
                            })
                            .collect::<Vec<_>>();
                        if let Some(parent) =
                            herdr_work_code::scm_tree::parent_directory(&depths, self.scm_cursor)
                        {
                            self.scm_cursor = parent;
                        }
                    }
                    _ => {}
                }
            }
            return Ok(true);
        }
        if !matches!(
            key,
            KeyCode::Home | KeyCode::End | KeyCode::PageUp | KeyCode::PageDown
        ) {
            return Ok(false);
        }
        let sidebar = self.panes(self.interaction.area).sidebar;
        let height = if self.tab == Tab::Search {
            self.search_regions(sidebar).1.height
        } else {
            self.sidebar_block(String::new()).inner(sidebar).height
        };
        let count = match self.tab {
            Tab::Files => self.code.as_ref().map_or(0, |code| code.files.len()),
            Tab::Search => self.search_rows.len(),
            Tab::Tasks => self.tasks.len(),
            Tab::Scm => self.scm_rows().len(),
            Tab::Review => return Ok(false),
        };
        let page = usize::from(if self.tab == Tab::Tasks {
            height / 2
        } else {
            height
        })
        .max(1);
        let selected = if self.tab == Tab::Tasks {
            &mut self.task
        } else if self.tab == Tab::Scm {
            &mut self.scm_cursor
        } else {
            &mut self.selection
        };
        *selected = match key {
            KeyCode::Home => 0,
            KeyCode::End => count.saturating_sub(1),
            KeyCode::PageUp => selected.saturating_sub(page),
            _ => selected.saturating_add(page).min(count.saturating_sub(1)),
        };
        if self.tab == Tab::Scm {
            self.git.active = Some(self.selected_repository());
            if let Some(index) = self.scm_rows().get(self.scm_cursor).and_then(|row| row.0) {
                self.selection = index;
                self.scm_selection = index;
            }
        }
        Ok(true)
    }
}
