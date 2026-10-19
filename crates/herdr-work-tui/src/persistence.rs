//! Product workspace snapshots: source DTOs, guarded background reads/writes.
use crate::{
    preview::PreviewDocument,
    scm::{ScmRow, ScmTarget},
    state::*,
};
use anyhow::Result;
use herdr_work_code::{
    preview::{Doc, ViewState},
    scm::Drawer,
    view_state::ScmState,
};
use herdr_work_engine::{DocumentState, DrawerRoot, DrawerState, WorkspaceUiState};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(crate) enum Update {
    Loaded(u64, u64, PathBuf, Result<WorkspaceUiState>),
    Saved(u64, PathBuf, WorkspaceUiState, Result<()>),
    Documents(PathBuf, u64, Vec<(DocumentState, Result<Doc>)>),
    Drawers(
        PathBuf,
        u64,
        Vec<(Drawer, Result<herdr_work_engine::DrawerView>)>,
    ),
}
impl App {
    fn relative_ui_path(&self, path: &std::path::Path) -> Option<PathBuf> {
        path.strip_prefix(&self.root)
            .ok()
            .map(std::path::Path::to_path_buf)
    }
    fn scm_key(&self, row: &ScmRow) -> String {
        let repo = |index| {
            self.repository_root(index)
                .ok()
                .and_then(|root| self.relative_ui_path(&root))
                .unwrap_or_default()
        };
        let value = match &row.3 {
            ScmTarget::Repository(i) => serde_json::json!(["repo", repo(*i)]),
            ScmTarget::Commit(i) => serde_json::json!(["commit", repo(*i)]),
            ScmTarget::Message(i) => serde_json::json!(["message", repo(*i)]),
            ScmTarget::Section(i, staged) => serde_json::json!(["section", repo(*i), staged]),
            ScmTarget::Directory(i, staged, path) => {
                serde_json::json!(["directory", repo(*i), staged, path])
            }
            ScmTarget::Change(i) => {
                let change = &self.code.as_ref().unwrap().changes[*i];
                serde_json::json!([
                    "change",
                    self.relative_ui_path(&change.root),
                    change.staged,
                    change.path
                ])
            }
            ScmTarget::Drawer(kind) => serde_json::json!(["drawer", kind.title()]),
            ScmTarget::History(kind, index) => serde_json::json!([
                "history",
                kind.title(),
                self.drawers
                    .get(kind)
                    .and_then(|view| view.lines.get(*index))
            ]),
        };
        value.to_string()
    }
    pub(crate) fn workspace_snapshot(&self) -> Option<WorkspaceUiState> {
        if self.interaction.workspace.identity.is_empty() {
            return None;
        }
        if self.interaction.restore.is_some() {
            return Some(self.interaction.workspace.clone());
        }
        let mut state = WorkspaceUiState {
            identity: self.interaction.workspace.identity.clone(),
            revision: self.interaction.workspace.revision,
            activity: match self.tab {
                Tab::Files => "files",
                Tab::Search => "search",
                Tab::Scm => "git",
                Tab::Tasks => "tasks",
                Tab::Review => "review",
            }
            .into(),
            ..Default::default()
        };
        state.tree.expanded = self
            .expanded
            .iter()
            .filter_map(|path| self.relative_ui_path(path))
            .collect();
        let selected = if self.tab == Tab::Files {
            self.code
                .as_ref()
                .and_then(|code| code.files.get(self.selection))
                .map(|row| &row.path)
        } else {
            self.interaction.file_selection.as_ref()
        };
        state.tree.selected = selected.and_then(|path| self.relative_ui_path(path));
        state.scm = ScmState {
            active_root: self
                .relative_ui_path(&self.selected_repository())
                .map(|path| path.to_string_lossy().into_owned()),
            selected: self
                .scm_rows()
                .get(self.scm_cursor)
                .map(|row| self.scm_key(row)),
            drawers: Drawer::ALL
                .into_iter()
                .filter(|kind| self.drawer_open[kind.index()])
                .map(|kind| kind.title().into())
                .collect(),
            scroll: self.drawer_scroll as usize,
            collapsed_dirs: self
                .git
                .collapsed
                .iter()
                .filter_map(|((root, staged), paths)| {
                    self.relative_ui_path(root).map(|root| {
                        paths
                            .iter()
                            .map(move |path| serde_json::json!([root, staged, path]).to_string())
                    })
                })
                .flatten()
                .collect(),
            ..Default::default()
        };
        state.scm_tree = self.git.tree;
        state.active_drawer = self.active_drawer.map(|kind| kind.title().to_string());
        state.drawers = self
            .drawers
            .iter()
            .filter(|(kind, _)| self.drawer_open[kind.index()])
            .filter_map(|(kind, view)| {
                if !self
                    .code
                    .as_ref()?
                    .repositories
                    .iter()
                    .any(|repo| repo.root == view.root)
                {
                    return None;
                }
                let root = self
                    .relative_ui_path(&view.root)
                    .map(DrawerRoot::Workspace)
                    .or_else(|| {
                        self.root
                            .starts_with(&view.root)
                            .then_some(DrawerRoot::Enclosing)
                    })?;
                Some(DrawerState {
                    kind: kind.title().into(),
                    root,
                    instance: view.instance.clone(),
                    file: view.file.clone(),
                })
            })
            .collect();
        state.drawers.sort_by(|a, b| a.kind.cmp(&b.kind));
        state.repository_collapsed = self
            .git
            .repository_collapsed
            .iter()
            .filter_map(|root| self.relative_ui_path(root))
            .collect();
        state.section_collapsed = self
            .git
            .section_collapsed
            .iter()
            .filter_map(|(root, staged)| self.relative_ui_path(root).map(|root| (root, *staged)))
            .collect();
        if let Some(path) = &self.preview_path
            && let Some(path) = self.relative_ui_path(path)
        {
            state.documents.push(DocumentState {
                path,
                view: self
                    .preview_doc
                    .as_ref()
                    .map(Doc::view_state)
                    .unwrap_or(ViewState {
                        line: self.preview_target.as_ref().map_or(1, |(_, line)| *line),
                        wrap: true,
                    }),
                pinned: self.preview_pinned,
                active: true,
            });
        }
        state
            .documents
            .extend(self.preview_documents.iter().filter_map(|document| {
                self.relative_ui_path(&document.path)
                    .map(|path| DocumentState {
                        path,
                        view: document.doc.view_state(),
                        pinned: true,
                        active: false,
                    })
            }));
        if state.documents.is_empty() && self.interaction.restoring_documents {
            state.documents = self.interaction.workspace.documents.clone();
        }
        state.documents.truncate(64);
        state.scm.collapsed_dirs.sort();
        Some(state)
    }
    /// Final lifecycle save happens after the event loop, before the profile is released.
    pub(crate) fn persist_workspace_on_exit(&mut self) -> Result<()> {
        if let Some(state) = self.next_workspace_snapshot() {
            self.desk.save_workspace_ui_state(&self.root, &state)?;
        }
        Ok(())
    }
    fn next_workspace_snapshot(&mut self) -> Option<WorkspaceUiState> {
        let mut state = self.workspace_snapshot()?;
        self.interaction.save_request = self
            .interaction
            .save_request
            .max(self.interaction.workspace.revision)
            .saturating_add(1);
        state.revision = self.interaction.save_request;
        self.interaction.workspace.revision = state.revision;
        Some(state)
    }
    pub(crate) fn queue_workspace_save(&mut self) -> Result<()> {
        let Some(state) = self.next_workspace_snapshot() else {
            return Ok(());
        };
        let root = self.root.clone();
        let request = self.interaction.save_request;
        self.job(move |desk| {
            let result = desk.save_workspace_ui_state(&root, &state);
            Ok(Data::Persistence(Box::new(Update::Saved(
                request, root, state, result,
            ))))
        })?;
        self.interaction.save_pending = Some(request);
        Ok(())
    }
    pub(crate) fn tick_persistence(&mut self, now: Instant) -> Result<()> {
        if now.saturating_duration_since(self.interaction.last_save) < Duration::from_millis(500)
            || self.interaction.save_pending.is_some()
            || self.interaction.restore.is_some()
            || self.interaction.restoring_documents
        {
            return Ok(());
        }
        if self.workspace_snapshot().as_ref() != self.interaction.last_saved.as_ref() {
            self.queue_workspace_save()?;
        }
        self.interaction.last_save = now;
        Ok(())
    }
    pub(crate) fn load_workspace_after_change(&mut self) -> Result<()> {
        self.interaction.workspace = Default::default();
        self.interaction.restore = None;
        self.interaction.restoring_documents = false;
        self.interaction.file_selection = None;
        let root = self.root.clone();
        self.interaction.load_request = self.interaction.load_request.wrapping_add(1);
        let request = self.interaction.load_request;
        let revision = self.interaction.user_revision;
        self.job(move |desk| {
            Ok(Data::Persistence(Box::new(Update::Loaded(
                request,
                revision,
                root.clone(),
                desk.workspace_ui_state(&root),
            ))))
        })
    }
    pub(crate) fn restore_workspace_after_code(&mut self) -> Result<()> {
        let Some(state) = self.interaction.restore.take() else {
            return Ok(());
        };
        let roots = self
            .code
            .as_ref()
            .unwrap()
            .repositories
            .iter()
            .map(|repo| repo.root.clone())
            .collect::<std::collections::BTreeSet<_>>();
        self.git.active = state
            .scm
            .active_root
            .as_ref()
            .map(|path| self.root.join(path))
            .filter(|root| roots.contains(root));
        self.git.tree = state.scm_tree;
        self.git.repository_collapsed = state
            .repository_collapsed
            .iter()
            .map(|root| self.root.join(root))
            .filter(|root| roots.contains(root))
            .collect();
        self.git.section_collapsed = state
            .section_collapsed
            .iter()
            .map(|(root, staged)| (self.root.join(root), *staged))
            .filter(|(root, _)| roots.contains(root))
            .collect();
        self.git.collapsed.clear();
        for value in &state.scm.collapsed_dirs {
            if let Ok((root, staged, path)) = serde_json::from_str::<(PathBuf, bool, String)>(value)
            {
                let root = self.root.join(root);
                if roots.contains(&root) {
                    self.git
                        .collapsed
                        .entry((root, staged))
                        .or_default()
                        .insert(path);
                }
            }
        }
        for kind in Drawer::ALL {
            self.drawer_open[kind.index()] =
                state.scm.drawers.iter().any(|title| title == kind.title());
        }
        if self.tab == Tab::Files
            && let Some(selected) = &self.interaction.file_selection
        {
            self.selection = self
                .code
                .as_ref()
                .unwrap()
                .files
                .iter()
                .position(|row| &row.path == selected)
                .unwrap_or(0);
        }
        self.interaction.pending_scm_key = state.scm.selected.clone();
        self.restore_scm_selection();
        self.active_drawer = state
            .active_drawer
            .as_ref()
            .and_then(|title| Drawer::ALL.into_iter().find(|kind| kind.title() == title))
            .filter(|kind| state.drawers.iter().any(|saved| saved.kind == kind.title()));
        self.drawer_scroll = state.scm.scroll.min(u16::MAX as usize) as u16;
        if !state.drawers.is_empty() {
            let root = self.root.clone();
            let request = self.drawer_request;
            let drawers = state.drawers.clone();
            if self.active_drawer.is_some() {
                self.drawer_state = crate::loading::LoadState::Loading;
            }
            self.job(move |desk| {
                let views = drawers
                    .into_iter()
                    .filter_map(|saved| {
                        let kind = Drawer::ALL
                            .into_iter()
                            .find(|kind| kind.title() == saved.kind)?;
                        Some((kind, desk.restore_ui_drawer(&root, &saved)))
                    })
                    .collect();
                Ok(Data::Persistence(Box::new(Update::Drawers(
                    root, request, views,
                ))))
            })?;
        }
        if !state.documents.is_empty() && self.preview_path.is_none() && self.editor.is_none() {
            let root = self.root.clone();
            let request = self.preview_request;
            let width = self.preview_width;
            self.job(move |desk| {
                let documents = state
                    .documents
                    .into_iter()
                    .map(|document| {
                        let loaded = desk.preview_file(
                            &root,
                            &document.path.to_string_lossy(),
                            Some(document.view.line),
                            width,
                        );
                        (document, loaded)
                    })
                    .collect();
                Ok(Data::Persistence(Box::new(Update::Documents(
                    root, request, documents,
                ))))
            })?;
            self.interaction.restoring_documents = true;
        }
        Ok(())
    }
    fn restore_scm_selection(&mut self) {
        if let Some(selected) = &self.interaction.pending_scm_key
            && let Some(index) = self
                .scm_rows()
                .iter()
                .position(|row| &self.scm_key(row) == selected)
        {
            self.scm_cursor = index;
            self.interaction.pending_scm_key = None;
        }
    }
    pub(crate) fn apply_persistence(&mut self, update: Update) {
        match update {
            Update::Saved(request, root, state, result) => {
                if self.interaction.save_pending != Some(request) {
                    return;
                }
                self.interaction.save_pending = None;
                if root == self.root {
                    match result {
                        Ok(()) => self.interaction.last_saved = Some(state),
                        Err(error) => self.status = error.to_string(),
                    }
                }
            }
            Update::Loaded(request, revision, root, result)
                if root == self.root && request == self.interaction.load_request =>
            {
                match result {
                    Ok(state) => {
                        if revision != self.interaction.user_revision {
                            self.interaction.workspace.identity = state.identity;
                            self.interaction.workspace.revision = state.revision;
                            return;
                        }
                        self.expanded = state
                            .tree
                            .expanded
                            .iter()
                            .map(|path| root.join(path))
                            .collect();
                        self.interaction.file_selection =
                            state.tree.selected.as_ref().map(|path| root.join(path));
                        self.tab = match state.activity.as_str() {
                            "git" => Tab::Scm,
                            "search" => Tab::Search,
                            "tasks" => Tab::Tasks,
                            "review" => Tab::Review,
                            _ => Tab::Files,
                        };
                        self.interaction.restore = Some(state.clone());
                        self.interaction.workspace = state;
                        let _ = self.refresh();
                    }
                    Err(error) => self.status = error.to_string(),
                }
            }
            Update::Drawers(root, request, views)
                if root == self.root && request == self.drawer_request =>
            {
                for (kind, result) in views {
                    match result {
                        Ok(view) => {
                            self.drawers.insert(view.kind, view);
                            if self.active_drawer == Some(kind) {
                                self.drawer_state = crate::loading::LoadState::Ready;
                            }
                        }
                        Err(error) if self.active_drawer == Some(kind) => {
                            self.drawer_state =
                                crate::loading::LoadState::Failed(format!("{error:#}"))
                        }
                        _ => {}
                    }
                }
                self.restore_scm_selection();
            }
            Update::Documents(root, request, documents) if root == self.root => {
                self.interaction.restoring_documents = false;
                if request != self.preview_request
                    || self.preview_path.is_some()
                    || self.editor.is_some()
                    || self.prompt.is_some()
                    || self.overlay.is_some()
                {
                    return;
                }
                for (state, document) in documents {
                    let Ok(mut doc) = document else {
                        continue;
                    };
                    doc.restore_view_state(state.view);
                    let path = root.join(state.path);
                    if state.active && self.preview_path.is_none() {
                        self.preview_path = Some(path);
                        self.preview_pinned = state.pinned;
                        self.text = doc.plain_text();
                        self.preview_doc = Some(doc);
                    } else if state.pinned {
                        self.preview_documents.push(PreviewDocument {
                            path,
                            doc,
                            target: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}
