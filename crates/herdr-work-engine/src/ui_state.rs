//! Product-only workspace snapshots reuse source DTOs, never source plugin storage.
use crate::{HerdrWork, code::checked_path, storage};
use anyhow::{Result, ensure};
use herdr_work_code::{
    preview::ViewState,
    view_state::{ScmState, TreeState},
};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentState {
    pub path: PathBuf,
    pub view: ViewState,
    pub pinned: bool,
    pub active: bool,
}
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceUiState {
    pub identity: String,
    pub revision: u64,
    pub activity: String,
    pub tree: TreeState,
    pub scm: ScmState,
    pub scm_tree: bool,
    pub active_drawer: Option<String>,
    pub repository_collapsed: Vec<PathBuf>,
    pub section_collapsed: Vec<(PathBuf, bool)>,
    pub documents: Vec<DocumentState>,
}
fn relative(root: &Path, path: &Path, allow_root: bool) -> Result<PathBuf> {
    if allow_root && path.as_os_str().is_empty() {
        return Ok(root.to_path_buf());
    }
    ensure!(
        path.components()
            .all(|part| matches!(part, Component::Normal(name) if name != ".git")),
        "无效的界面状态路径"
    );
    checked_path(root, &path.to_string_lossy())
}
impl WorkspaceUiState {
    fn validate(&self, root: &Path) -> Result<()> {
        for path in self
            .tree
            .expanded
            .iter()
            .chain(self.tree.selected.iter())
            .chain(self.documents.iter().map(|document| &document.path))
        {
            relative(root, path, false)?;
        }
        for path in self
            .repository_collapsed
            .iter()
            .chain(self.section_collapsed.iter().map(|entry| &entry.0))
        {
            relative(root, path, true)?;
        }
        if let Some(path) = &self.scm.active_root {
            relative(root, Path::new(path), true)?;
        }
        if let Some(path) = &self.scm.history_target {
            let repository = self
                .scm
                .active_root
                .as_ref()
                .map(|path| root.join(path))
                .unwrap_or_else(|| root.to_path_buf());
            relative(&repository, Path::new(path), false)?;
        }
        ensure!(self.documents.len() <= 64, "界面状态最多恢复64个文档");
        Ok(())
    }
}
impl HerdrWork {
    pub fn workspace_ui_state(&self, root: &Path) -> Result<WorkspaceUiState> {
        let root = root.canonicalize()?;
        let identity = Self::worktree_identity(&root)?;
        let path = self
            .root()
            .join("ui-workspaces")
            .join(format!("{}.json", storage::key(&root.to_string_lossy())));
        if !path.exists() {
            return Ok(WorkspaceUiState {
                identity,
                ..Default::default()
            });
        }
        ensure!(path.metadata()?.len() <= 1024 * 1024, "界面状态文件过大");
        let mut saved: WorkspaceUiState = storage::load(&path)?;
        if saved.identity != identity {
            return Ok(WorkspaceUiState {
                identity,
                ..Default::default()
            });
        }
        saved.validate(&root)?;
        saved.tree.expanded.retain(|path| root.join(path).is_dir());
        saved.tree.selected = saved.tree.selected.filter(|path| root.join(path).exists());
        saved
            .documents
            .retain(|document| root.join(&document.path).is_file());
        Ok(saved)
    }
    pub fn save_workspace_ui_state(&self, root: &Path, state: &WorkspaceUiState) -> Result<()> {
        let root = root.canonicalize()?;
        ensure!(
            Self::worktree_identity(&root)? == state.identity,
            "工作目录实例已改变，未保存旧界面状态"
        );
        state.validate(&root)?;
        let path = self
            .root()
            .join("ui-workspaces")
            .join(format!("{}.json", storage::key(&root.to_string_lossy())));
        use fs2::FileExt;
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))?;
        lock.lock_exclusive()?;
        if let Ok(current) = storage::load::<WorkspaceUiState>(&path)
            && current.identity == state.identity
            && current.revision > state.revision
        {
            return Ok(());
        }
        storage::save(&path, state)
    }
}
