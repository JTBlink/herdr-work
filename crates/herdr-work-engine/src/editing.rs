//! Reuse Sidebar's UTF-8 editor with HerdrWork worktree and path guards.
use crate::{HerdrWork, code::checked_path};
use anyhow::{Result, ensure};
use herdr_work_code::editor::{Editor, SaveOutcome};
use std::path::{Path, PathBuf};

pub struct EditorSession {
    pub editor: Editor,
    pub root: PathBuf,
    identity: String,
    path: PathBuf,
}
impl EditorSession {
    pub fn save(&mut self) -> Result<()> {
        ensure!(
            HerdrWork::worktree_identity(&self.root)? == self.identity,
            "工作目录实例已改变，保留编辑内容，无法保存"
        );
        ensure!(
            self.path.canonicalize()? == self.path && self.path.starts_with(&self.root),
            "文件路径已改变，保留编辑内容，无法保存"
        );
        ensure!(
            self.editor.save(false)? == SaveOutcome::Saved,
            "文件已被外部修改，未覆盖磁盘内容；请保留编辑文本后重新打开文件"
        );
        Ok(())
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
        Ok(EditorSession {
            editor,
            root,
            identity,
            path,
        })
    }
}
