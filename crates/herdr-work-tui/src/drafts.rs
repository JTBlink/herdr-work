//! Root-keyed draft persistence: editing is in memory; bounded worker saves coalesce.
use crate::state::*;
use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct DraftRuntime {
    revisions: HashMap<PathBuf, u64>,
    dirty: HashMap<PathBuf, Instant>,
    saving: HashMap<PathBuf, u64>,
    failed: HashSet<PathBuf>,
}
impl App {
    pub(crate) fn mark_git_draft(&mut self, root: PathBuf) {
        let runtime = &mut self.git.draft_runtime;
        *runtime.revisions.entry(root.clone()).or_default() += 1;
        runtime.dirty.insert(root.clone(), Instant::now());
        runtime.failed.remove(&root);
    }
    pub(crate) fn tick_git_drafts(&mut self, now: Instant) -> Result<()> {
        let runtime = &self.git.draft_runtime;
        let Some(root) = runtime
            .dirty
            .iter()
            .find(|(root, at)| {
                !runtime.saving.contains_key(*root)
                    && !runtime.failed.contains(*root)
                    && now.saturating_duration_since(**at) >= Duration::from_millis(300)
            })
            .map(|(root, _)| root.clone())
        else {
            return Ok(());
        };
        let draft = self
            .git
            .drafts
            .get(&root)
            .context("提交草稿已失效")?
            .clone();
        let revision = runtime.revisions[&root];
        let target = root.clone();
        if self
            .job(move |desk| {
                let result = desk.save_git_draft(&target, &draft);
                Ok(Data::GitDraftSaved(target, revision, result))
            })
            .is_err()
        {
            return Ok(());
        } // Keep dirty data and defer when the bounded queue is full.
        self.git.draft_runtime.saving.insert(root, revision);
        Ok(())
    }
    pub(crate) fn git_draft_saved(&mut self, root: PathBuf, revision: u64, result: Result<()>) {
        let runtime = &mut self.git.draft_runtime;
        if runtime.saving.get(&root) != Some(&revision) {
            return;
        }
        runtime.saving.remove(&root);
        match result {
            Ok(()) if runtime.revisions.get(&root) == Some(&revision) => {
                runtime.dirty.remove(&root);
            }
            Ok(()) => {}
            Err(error) => {
                if runtime.revisions.get(&root) == Some(&revision) {
                    runtime.failed.insert(root.clone());
                }
                if self
                    .code
                    .as_ref()
                    .is_some_and(|code| code.repositories.iter().any(|repo| repo.root == root))
                {
                    self.status = format!("草稿保存失败，输入仍在内存中：{error:#}");
                }
            }
        }
    }
    /// Final FIFO save uses its own acknowledgement because run forwards normal
    /// queue results into the event channel, which is no longer being consumed.
    pub(crate) fn flush_git_drafts(&mut self) -> Result<()> {
        let values = self
            .git
            .draft_runtime
            .dirty
            .keys()
            .filter_map(|root| {
                self.git
                    .drafts
                    .get(root)
                    .map(|draft| (root.clone(), draft.clone()))
            })
            .collect::<Vec<_>>();
        if values.is_empty() {
            return Ok(());
        }
        let desk = self.desk.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.jobs.submit_on_shutdown(self.context.epoch, move || {
            let mut errors = Vec::new();
            for (root, draft) in values {
                if let Err(error) = desk.save_git_draft(&root, &draft) {
                    errors.push(format!("{}: {error:#}", root.display()));
                }
            }
            let result = if errors.is_empty() {
                Ok(())
            } else {
                Err(anyhow::anyhow!(errors.join("; ")))
            };
            let _ = tx.send(result);
            Ok(Data::GitDraftFlush)
        })?;
        rx.recv().context("草稿最终保存未收到确认")??;
        self.git.draft_runtime.dirty.clear();
        Ok(())
    }
}
