//! Coalesced passive reads use the existing worker and preserve active interaction.
use crate::state::*;
use anyhow::Result;
use herdr_work_engine::{CodeSnapshot, DiffView, DrawerView};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(crate) struct BackgroundRefresh {
    pub epoch: u64,
    pub generation: u64,
    pub root: PathBuf,
    pub code: Result<CodeSnapshot>,
    pub preview: Option<(u64, PathBuf, Result<Option<herdr_work_code::preview::Doc>>)>,
    pub diff: Option<(DiffView, Result<DiffView>)>,
    pub drawer: Option<(u64, Result<DrawerView>)>,
}
impl App {
    pub(crate) fn tick_refresh(&mut self, now: Instant) -> Result<()> {
        if self
            .background_pending
            .is_some_and(|(epoch, _)| epoch != self.context.epoch)
        {
            self.background_pending = None;
        }
        if self.background_pending.is_some()
            || now.saturating_duration_since(self.last_background) < Duration::from_secs(2)
        {
            return Ok(());
        }
        let epoch = self.context.epoch;
        let generation = self.generation;
        let root = self.root.clone();
        let expanded = self.expanded.clone();
        let hidden = self.preferences.show_hidden;
        let preview = self.preview_path.clone().filter(|_| self.editor.is_none());
        let preview_request = self.preview_request;
        let preview_width = self.preview_width;
        let baseline = self
            .preview_doc
            .as_ref()
            .filter(|doc| !doc.needs_width_reload(preview_width))
            .and_then(|doc| doc.stamp());
        let preview_line = self.preview_target.as_ref().map(|(_, line)| *line);
        let diff = self
            .diff
            .clone()
            .filter(|_| self.prompt.is_none() && self.overlay.is_none());
        let drawer = self
            .active_drawer
            .and_then(|kind| self.drawers.get(&kind))
            .cloned()
            .filter(|_| self.drawer_show.is_none());
        let drawer_request = self.drawer_request;
        self.job(move |desk| {
            let code = desk.code_snapshot_with_hidden(&root, &expanded, generation, hidden);
            let preview = preview.map(|path| {
                let value = path
                    .strip_prefix(&root)
                    .map_err(anyhow::Error::from)
                    .and_then(|file| {
                        desk.preview_file_if_changed(
                            &root,
                            &file.to_string_lossy(),
                            baseline,
                            preview_line,
                            preview_width,
                        )
                    });
                (preview_request, path, value)
            });
            let diff = diff.map(|previous| {
                let fresh = desk.diff(
                    &previous.root,
                    &previous.file,
                    &previous.scope,
                    previous.base.as_deref(),
                );
                (previous, fresh)
            });
            let drawer = drawer.map(|view| {
                (
                    drawer_request,
                    desk.scm_drawer(&view.root, view.kind, view.file.as_deref()),
                )
            });
            Ok(Data::Background(Box::new(BackgroundRefresh {
                epoch,
                generation,
                root,
                code,
                preview,
                diff,
                drawer,
            })))
        })?;
        self.last_background = now;
        self.background_pending = Some((epoch, generation));
        Ok(())
    }
    pub(crate) fn apply_background(&mut self, update: BackgroundRefresh) {
        if self.background_pending != Some((update.epoch, update.generation)) {
            return;
        }
        self.background_pending = None;
        if update.epoch != self.context.epoch
            || update.generation != self.generation
            || update.root != self.root
        {
            return;
        }
        match update.code {
            Ok(code) => {
                if self.code.as_ref().is_none_or(|old| {
                    old.files
                        .iter()
                        .map(|f| &f.path)
                        .ne(code.files.iter().map(|f| &f.path))
                }) {
                    self.invalidate_quick_index();
                }
                self.apply(update.epoch, Ok(Data::Code(update.generation, Ok(code))));
            }
            Err(error) if self.code.is_none() => {
                self.apply(update.epoch, Ok(Data::Code(update.generation, Err(error))))
            }
            Err(_) => {} // Retain the last usable snapshot on transient passive read errors.
        }
        if self.editor.is_none()
            && let Some((request, path, Ok(Some(doc)))) = update.preview
            && request == self.preview_request
            && self.preview_path.as_ref() == Some(&path)
        {
            self.apply_preview(path, doc, true);
        }
        if self.prompt.is_none()
            && self.overlay.is_none()
            && let Some((previous, Ok(fresh))) = update.diff
            && self.diff.as_ref().is_some_and(|current| {
                current.root == previous.root
                    && current.file == previous.file
                    && current.scope == previous.scope
                    && current.base == previous.base
                    && current.fingerprint == previous.fingerprint
            })
        {
            self.diff = Some(fresh);
        }
        if let Some((request, Ok(view))) = update.drawer
            && request == self.drawer_request
            && self.active_drawer == Some(view.kind)
            && self.drawer_show.is_none()
        {
            self.drawers.insert(view.kind, view);
        }
    }
}
