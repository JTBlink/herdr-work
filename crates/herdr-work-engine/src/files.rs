//! File mutations reuse Sidebar actions after worktree and selection checks.
use crate::{HerdrWork, code::checked_path};
use anyhow::{Context, Result, ensure};
use herdr_work_code::actions::{self, MenuAction};
use std::path::{Path, PathBuf};

pub struct FilePlan {
    pub action: MenuAction,
    pub target: PathBuf,
    root: PathBuf,
    instance: String,
    stamp: String,
}
fn stamp(path: &Path) -> Result<String> {
    fn visit(path: &Path, output: &mut Vec<String>) -> Result<()> {
        ensure!(output.len() < 20_000, "目录过大，请缩小文件操作范围");
        let m = path.symlink_metadata()?;
        output.push(format!(
            "{:?}:{:?}:{}:{:?}",
            path,
            m.file_type(),
            m.len(),
            m.modified()?
        ));
        if m.is_dir() {
            let mut children = std::fs::read_dir(path)?
                .map(|r| r.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            children.sort();
            for child in children {
                visit(&child, output)?;
            }
        }
        Ok(())
    }
    let mut entries = vec![];
    visit(path, &mut entries)?;
    Ok(crate::fingerprint(entries.join("\n").as_bytes()))
}
impl HerdrWork {
    pub fn prepare_file_action(
        &self,
        root: &Path,
        target: Option<&Path>,
        action: MenuAction,
    ) -> Result<FilePlan> {
        ensure!(
            matches!(
                action,
                MenuAction::NewFile
                    | MenuAction::NewFolder
                    | MenuAction::Rename
                    | MenuAction::Delete
            ),
            "不是文件修改操作"
        );
        let root = root.canonicalize()?;
        let selected = target.unwrap_or(&root);
        let mut target =
            if let (Some(parent), Some(name)) = (selected.parent(), selected.file_name()) {
                parent.canonicalize()?.join(name)
            } else {
                selected.canonicalize()?
            };
        if matches!(action, MenuAction::NewFile | MenuAction::NewFolder) && !target.is_dir() {
            target = target.parent().context("文件没有父目录")?.into();
        }
        if target != root {
            let relative = target.strip_prefix(&root)?;
            checked_path(&root, &relative.to_string_lossy())?;
            ensure!(
                !relative.components().any(|p| p.as_os_str() == ".git"),
                "不能修改 Git 内部文件"
            );
        }
        ensure!(
            target == root || target.canonicalize()?.starts_with(&root),
            "文件位于工作目录外"
        );
        ensure!(
            !target
                .canonicalize()?
                .strip_prefix(&root)?
                .components()
                .any(|p| p.as_os_str() == ".git"),
            "不能修改 Git 内部文件"
        );
        ensure!(
            target != root || matches!(action, MenuAction::NewFile | MenuAction::NewFolder),
            "不能重命名或删除工作目录根"
        );
        let stamp = if matches!(action, MenuAction::Rename | MenuAction::Delete) {
            stamp(&target)?
        } else {
            Self::worktree_identity(&target)?
        };
        Ok(FilePlan {
            action,
            target,
            instance: Self::worktree_identity(&root)?,
            root,
            stamp,
        })
    }
    pub fn apply_file_action(&self, plan: &FilePlan, input: &str) -> Result<()> {
        ensure!(
            Self::worktree_identity(&plan.root)? == plan.instance,
            "工作目录实例已改变，请重新选择文件"
        );
        ensure!(
            plan.target.canonicalize()?.starts_with(&plan.root),
            "文件路径已改变"
        );
        let current = if matches!(plan.action, MenuAction::Rename | MenuAction::Delete) {
            stamp(&plan.target)?
        } else {
            Self::worktree_identity(&plan.target)?
        };
        ensure!(current == plan.stamp, "文件在确认期间发生变化，请重新选择");
        match plan.action {
            MenuAction::Delete => {
                ensure!(input == "DELETE", "请输入 DELETE 确认删除");
                actions::delete(&plan.target, plan.target.symlink_metadata()?.is_dir())?;
            }
            action => {
                let name = actions::validate_name(input).context("名称不能为空或包含路径分隔符")?;
                ensure!(name != ".git", "不能创建或重命名 Git 内部目录");
                match action {
                    MenuAction::NewFile => {
                        actions::create_file(&plan.target, name)?;
                    }
                    MenuAction::NewFolder => {
                        actions::create_folder(&plan.target, name)?;
                    }
                    MenuAction::Rename => {
                        actions::rename(&plan.target, name)?;
                    }
                    _ => unreachable!(),
                }
            }
        }
        Ok(())
    }
}
