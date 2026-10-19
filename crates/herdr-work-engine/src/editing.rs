//! Reuse Sidebar's UTF-8 editor with HerdrWork worktree and path guards.
use crate::{HerdrWork, code::checked_path};
use anyhow::{Result, ensure};
use herdr_work_code::editor::{Editor, ExternalObservation, ExternalRequest, SaveOutcome};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub struct EditorSession {
    pub editor: Editor,
    pub root: PathBuf,
    identity: String,
    path: PathBuf,
    id: u64,
}

pub struct EditorExternalRequest {
    root: PathBuf,
    identity: String,
    path: PathBuf,
    request: ExternalRequest,
    session: u64,
}
pub struct EditorExternalObservation {
    session: u64,
    observation: ExternalObservation,
}
fn validate(root: &Path, identity: &str, path: &Path) -> Result<()> {
    ensure!(
        HerdrWork::worktree_identity(root)? == identity,
        "工作目录实例已改变，保留编辑内容，无法读取或保存"
    );
    ensure!(
        path.canonicalize()? == path && path.starts_with(root),
        "文件路径已改变，保留编辑内容，无法读取或保存"
    );
    ensure!(path.is_file(), "只能编辑普通文本文件");
    Ok(())
}
impl EditorExternalRequest {
    pub fn observe(self) -> Result<EditorExternalObservation> {
        validate(&self.root, &self.identity, &self.path)?;
        let observation = self.request.observe(1024 * 1024, 5000);
        validate(&self.root, &self.identity, &self.path)?;
        Ok(EditorExternalObservation {
            session: self.session,
            observation,
        })
    }
}
impl EditorSession {
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn snapshot(&self) -> Self {
        Self {
            editor: self.editor.io_snapshot(),
            root: self.root.clone(),
            identity: self.identity.clone(),
            path: self.path.clone(),
            id: self.id,
        }
    }
    pub fn external_request(&self) -> EditorExternalRequest {
        EditorExternalRequest {
            root: self.root.clone(),
            identity: self.identity.clone(),
            path: self.path.clone(),
            request: self.editor.external_request(),
            session: self.id,
        }
    }
    pub fn apply_external(&mut self, observation: EditorExternalObservation) {
        if observation.session == self.id {
            self.editor.apply_external(observation.observation);
        }
    }
    pub fn save(&mut self) -> Result<()> {
        ensure!(
            self.save_with_overwrite(false)? == SaveOutcome::Saved,
            "文件已被外部修改，未覆盖磁盘内容"
        );
        Ok(())
    }
    pub fn save_with_overwrite(&mut self, overwrite: bool) -> Result<SaveOutcome> {
        validate(&self.root, &self.identity, &self.path)?;
        Ok(self.editor.save(overwrite)?)
    }
    pub fn reload(&mut self) -> Result<()> {
        validate(&self.root, &self.identity, &self.path)?;
        self.editor
            .reload(1024 * 1024, 5000)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        validate(&self.root, &self.identity, &self.path)
    }
    pub fn apply_saved(&mut self, saved: &Self) {
        if self.id == saved.id {
            self.editor.apply_saved(&saved.editor);
        }
    }
    pub fn apply_reloaded(&mut self, expected_text: &str, fresh: Self) -> bool {
        self.id == fresh.id && self.editor.apply_reloaded(expected_text, fresh.editor)
    }
}
impl HerdrWork {
    pub fn edit_file(&self, root: &Path, file: &str) -> Result<EditorSession> {
        let root = root.canonicalize()?;
        let path = checked_path(&root, file)?.canonicalize()?;
        ensure!(
            !path
                .strip_prefix(&root)?
                .components()
                .any(|p| p.as_os_str() == ".git"),
            "不能编辑 Git 内部文件"
        );
        ensure!(path.is_file(), "只能编辑普通文本文件");
        ensure!(
            path.metadata()?.len() <= 1024 * 1024,
            "编辑文件不能超过 1 MiB"
        );
        let identity = Self::worktree_identity(&root)?;
        let editor = Editor::open(&path, 1024 * 1024, 5000).map_err(|e| anyhow::anyhow!("{e}"))?;
        validate(&root, &identity, &path)?;
        static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
        Ok(EditorSession {
            editor,
            root,
            identity,
            path,
            id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
        })
    }
}
