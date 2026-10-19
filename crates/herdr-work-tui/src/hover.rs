//! Sidebar hover uses source geometry for both visible controls and hit testing.
use crate::{
    menus::Command,
    scm::{ScmRow, ScmTarget},
    state::*,
};
use anyhow::Result;
use herdr_work_code::{
    scm_view as source,
    ui::{self, TitleAction},
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    text::Line,
    widgets::{ListItem, Paragraph},
};
use std::{collections::HashMap, time::Instant};

#[derive(Default)]
pub(crate) struct HoverState {
    pub point: Option<Position>,
    last_mouse: Option<Instant>,
    title_shown: bool,
}
pub(crate) struct ScmHoverModel {
    dirs: Vec<herdr_work_code::scm_tree::DirEntry>,
    depths: HashMap<usize, usize>,
    count: usize,
}
pub(crate) type ScmHoverModels = HashMap<(usize, bool), ScmHoverModel>;
impl App {
    pub(crate) fn track_sidebar_mouse(&mut self, point: Position, area: Rect) {
        if self.panes(area).sidebar.contains(point) {
            self.hover.point = Some(point);
            self.hover.last_mouse = Some(Instant::now());
            self.hover.title_shown = true;
        } else {
            self.hover.point = None;
        }
    }
    pub(crate) fn tick_hover(&mut self) -> bool {
        if self.hover.title_shown && !ui::title_actions_visible(self.hover.last_mouse) {
            self.hover.title_shown = false;
            self.hover.point = None;
            return true;
        }
        false
    }
    pub(crate) fn sidebar_hovered(&self, body: Rect, index: usize, selected: usize) -> bool {
        self.hover.point.is_some_and(|p| {
            body.contains(p)
                && p.y.saturating_sub(body.y) as usize
                    + crate::layout::offset(selected, body.height)
                    == index
        })
    }
    fn title_actions(&self) -> Vec<TitleAction> {
        match self.tab {
            Tab::Files => vec![
                TitleAction::NewFile,
                TitleAction::NewFolder,
                TitleAction::Refresh,
                TitleAction::CollapseAll,
            ],
            Tab::Scm | Tab::Review => vec![
                if self.git.tree {
                    TitleAction::ViewAsList
                } else {
                    TitleAction::ViewAsTree
                },
                TitleAction::Refresh,
                TitleAction::CollapseAll,
            ],
            _ => vec![],
        }
    }
    pub(crate) fn sidebar_title_actions(
        &self,
        area: Rect,
    ) -> (Vec<ratatui::text::Span<'static>>, Vec<(Rect, TitleAction)>) {
        if !ui::title_actions_visible(self.hover.last_mouse) {
            return (vec![], vec![]);
        }
        let actions = self.title_actions();
        let width = ui::title_actions_width(self.icon_theme(), &actions);
        let body_width = area.width.saturating_sub(1);
        // Keep the overflow menu and a readable title; hidden controls have no zones.
        let minimum_title = if self.tab == Tab::Files { 8 } else { 14 };
        if actions.is_empty() || body_width < width + 3 + minimum_title + 1 {
            return (vec![], vec![]);
        }
        let x = area.x + u16::from(self.preferences.dock_right) + body_width - width - 3;
        ui::title_action_spans(
            self.icon_theme(),
            &actions,
            x,
            area.y,
            self.hover.point.map(|p| (p.x, p.y)),
        )
    }
    pub(crate) fn draw_sidebar_actions(&self, frame: &mut Frame, area: Rect) {
        let (spans, zones) = self.sidebar_title_actions(area);
        if let (Some(first), Some(last)) = (zones.first(), zones.last()) {
            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(first.0.x, area.y, last.0.right() - first.0.x, 1),
            );
        }
    }
    pub(crate) fn sidebar_title_action(&mut self, action: TitleAction) -> Result<()> {
        match action {
            TitleAction::NewFile | TitleAction::NewFolder => {
                self.menu_path = self.selected_path().ok();
                self.menu_command(Command::FileAction(if action == TitleAction::NewFile {
                    herdr_work_code::actions::MenuAction::NewFile
                } else {
                    herdr_work_code::actions::MenuAction::NewFolder
                }))?;
            }
            TitleAction::Refresh => self.action(crossterm::event::KeyCode::Char('r'))?,
            TitleAction::ViewAsTree | TitleAction::ViewAsList => {
                self.action(crossterm::event::KeyCode::Char('t'))?
            }
            TitleAction::CollapseAll if self.tab == Tab::Files => {
                self.menu_command(Command::Collapse)?
            }
            TitleAction::CollapseAll => {
                if let Some(code) = &self.code {
                    let multi = code.repositories.len() > 1;
                    for repo in &code.repositories {
                        if multi {
                            self.git.repository_collapsed.insert(repo.root.clone());
                        }
                        self.git.section_collapsed.insert((repo.root.clone(), true));
                        self.git
                            .section_collapsed
                            .insert((repo.root.clone(), false));
                    }
                }
                self.drawer_open = [false; 8];
                self.active_drawer = None;
                self.scm_cursor = 0;
            }
        }
        Ok(())
    }
    pub(crate) fn scm_hover_models(&self) -> ScmHoverModels {
        let mut models = HashMap::new();
        if let Some(code) = &self.code {
            for (repo, root) in code.repositories.iter().enumerate() {
                for staged in [false, true] {
                    let entries = code
                        .changes
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.root == root.root && c.staged == staged)
                        .collect::<Vec<_>>();
                    let files = entries
                        .iter()
                        .map(|(_, c)| herdr_work_code::git::FileEntry {
                            path: c.path.clone(),
                            orig: c.original.clone(),
                            letter: c.status,
                        })
                        .collect::<Vec<_>>();
                    let collapsed = self
                        .git
                        .collapsed
                        .get(&(root.root.clone(), staged))
                        .cloned()
                        .unwrap_or_default();
                    let (dirs, depths, _) =
                        herdr_work_code::scm_tree::build_tree_model(&files, &collapsed);
                    models.insert(
                        (repo, staged),
                        ScmHoverModel {
                            dirs,
                            depths: entries
                                .iter()
                                .zip(depths)
                                .map(|((i, _), depth)| (*i, depth))
                                .collect(),
                            count: entries.len(),
                        },
                    );
                }
            }
        }
        models
    }
    pub(crate) fn scm_hover_item(
        &self,
        row: &ScmRow,
        width: u16,
        hovered: bool,
        x: Option<u16>,
        models: &ScmHoverModels,
    ) -> ListItem<'static> {
        match &row.3 {
            ScmTarget::Change(index) => {
                if let Some(code) = &self.code
                    && let Some(change) = code.changes.get(*index)
                {
                    let entry = herdr_work_code::git::FileEntry {
                        path: change.path.clone(),
                        orig: change.original.clone(),
                        letter: change.status,
                    };
                    let action =
                        x.and_then(|x| source::file_hover_action_at(x, width, change.staged));
                    if self.git.tree {
                        let depth = code
                            .repositories
                            .iter()
                            .position(|r| r.root == change.root)
                            .and_then(|i| models.get(&(i, change.staged)))
                            .and_then(|m| m.depths.get(index))
                            .copied()
                            .unwrap_or(0);
                        return source::tree_file_item(
                            &entry,
                            depth,
                            width as usize,
                            self.icon_theme(),
                            change.staged,
                            hovered,
                            action,
                        );
                    }
                    return source::file_item(
                        &entry,
                        width as usize,
                        self.icon_theme(),
                        change.staged,
                        hovered,
                        action,
                    );
                }
            }
            ScmTarget::Directory(repo, staged, path) if hovered => {
                if let Some(dir) = models
                    .get(&(*repo, *staged))
                    .and_then(|m| m.dirs.iter().find(|d| &d.path == path))
                {
                    let action =
                        x.and_then(|x| source::dir_hover_action_at(x, width, *staged, dir.count));
                    return source::tree_dir_item(
                        dir,
                        width as usize,
                        self.icon_theme(),
                        *staged,
                        true,
                        action,
                    );
                }
            }
            ScmTarget::Message(index) => {
                if let Some(repo) = self
                    .code
                    .as_ref()
                    .and_then(|code| code.repositories.get(*index))
                {
                    let draft = self.git.drafts.get(&repo.root);
                    return herdr_work_code::scm_widgets::message_box_item(
                        draft.map_or(&[], |draft| draft.message.as_slice()),
                        draft.map_or(0, |draft| draft.cursor),
                        &repo.branch,
                        self.git.editing.as_ref() == Some(&repo.root),
                        self.icon_theme(),
                        width as usize,
                    );
                }
            }
            ScmTarget::Commit(index) => {
                let root = self.repository_root(*index).ok();
                let active = root
                    .as_ref()
                    .is_some_and(|root| self.selected_repository() == *root);
                return herdr_work_code::scm_widgets::commit_button_item(
                    &row.1,
                    active,
                    self.scm_cursor
                        == self
                            .scm_rows()
                            .iter()
                            .position(|r| r.3 == row.3)
                            .unwrap_or(usize::MAX)
                        && self.focus == Focus::Sidebar,
                    width as usize,
                );
            }
            ScmTarget::Repository(index) => {
                if let Some(repo) = self
                    .code
                    .as_ref()
                    .and_then(|code| code.repositories.get(*index))
                {
                    let status = self.sidebar_repo_status(*index);
                    let name = repo.root.file_name().unwrap_or_default().to_string_lossy();
                    let name = if repo.write_error.is_some() {
                        format!("⚠只读 {name}")
                    } else {
                        name.into_owned()
                    };
                    let header = herdr_work_code::scm_widgets::RepositoryHeader {
                        name: &name,
                        status: &status,
                        collapsed: self.git.repository_collapsed.contains(&repo.root),
                    };
                    let zone = herdr_work_code::scm_widgets::repo_header_branch_zone(
                        header,
                        self.icon_theme(),
                        width,
                    );
                    return herdr_work_code::scm_widgets::repo_header_item(
                        header,
                        self.selected_repository() == repo.root,
                        self.icon_theme(),
                        width as usize,
                        hovered && x.is_some_and(|x| ui::within(x, zone)),
                    );
                }
            }
            ScmTarget::Section(repo, true) if hovered => {
                if let Some(model) = models.get(&(*repo, true)) {
                    let collapsed = self
                        .repository_root(*repo)
                        .is_ok_and(|root| self.git.section_collapsed.contains(&(root, true)));
                    return source::section_item(
                        "Staged",
                        collapsed,
                        Some(model.count),
                        width as usize,
                        Some('−'),
                    );
                }
            }
            ScmTarget::Section(repo, false) if hovered => {
                if let Some(model) = models.get(&(*repo, false)) {
                    let collapsed = self
                        .repository_root(*repo)
                        .is_ok_and(|root| self.git.section_collapsed.contains(&(root, false)));
                    return source::changes_header_item(
                        collapsed,
                        model.count,
                        width as usize,
                        true,
                        x.and_then(|x| source::changes_header_action_at(x, width, model.count)),
                    );
                }
            }
            _ => {}
        }
        ListItem::new(row.1.clone())
    }
    pub(crate) fn scm_hover_action(&mut self, index: usize, x: u16, width: u16) -> Result<bool> {
        use source::{ChangesHeaderAction, FileHoverAction};
        let rows = self.scm_rows();
        let Some(row) = rows.get(index) else {
            return Ok(false);
        };
        self.scm_cursor = index;
        self.git.active = Some(self.selected_repository());
        if let Some(selected) = row.0 {
            self.selection = selected;
            self.scm_selection = selected;
        }
        let models = self.scm_hover_models();
        match &row.3 {
            ScmTarget::Change(i) => {
                let change = &self.code.as_ref().unwrap().changes[*i];
                match source::file_hover_action_at(x, width, change.staged) {
                    Some(FileHoverAction::Open) => self.activate_scm_row()?,
                    Some(FileHoverAction::Stage) => self.change_stage(true)?,
                    Some(FileHoverAction::Unstage) => self.change_stage(false)?,
                    Some(FileHoverAction::Discard) => {
                        self.action(crossterm::event::KeyCode::Char('d'))?
                    }
                    None => return Ok(false),
                }
            }
            ScmTarget::Directory(repo, staged, path) => {
                let action = models
                    .get(&(*repo, *staged))
                    .and_then(|m| m.dirs.iter().find(|d| &d.path == path))
                    .and_then(|dir| source::dir_hover_action_at(x, width, *staged, dir.count));
                if action.is_none() {
                    return Ok(false);
                }
                self.change_stage(!staged)?;
            }
            ScmTarget::Repository(_) => {
                let (sync, commit) = source::repo_header_action_zones(width);
                if ui::within(x, commit) {
                    self.prepare_commit()?;
                } else if ui::within(x, sync) {
                    self.work_git(herdr_work_engine::WorkGitAction::Sync)?;
                } else if let ScmTarget::Repository(index) = row.3 {
                    let status = self.sidebar_repo_status(index);
                    let header = herdr_work_code::scm_widgets::RepositoryHeader {
                        name: "",
                        status: &status,
                        collapsed: false,
                    };
                    let zone = herdr_work_code::scm_widgets::repo_header_branch_zone(
                        header,
                        self.icon_theme(),
                        width,
                    );
                    if ui::within(x, zone) {
                        self.open_branches()?;
                    } else {
                        return Ok(false);
                    }
                } else {
                    return Ok(false);
                }
            }
            ScmTarget::Section(repo, true) => {
                if !models
                    .get(&(*repo, true))
                    .and_then(|m| source::section_action_zone("Staged", m.count, width))
                    .is_some_and(|zone| ui::within(x, zone))
                {
                    return Ok(false);
                }
                self.change_stage(false)?;
            }
            ScmTarget::Section(repo, false) => {
                let action = models
                    .get(&(*repo, false))
                    .and_then(|m| source::changes_header_action_at(x, width, m.count));
                let action = match action {
                    Some(ChangesHeaderAction::Stage) => herdr_work_engine::WorkGitAction::StageAll,
                    Some(ChangesHeaderAction::Discard) => {
                        herdr_work_engine::WorkGitAction::DiscardAll
                    }
                    Some(ChangesHeaderAction::Stash) => herdr_work_engine::WorkGitAction::Stash,
                    None => return Ok(false),
                };
                self.work_git(action)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
