//! Original Sidebar SCM queries, bounded by the same process runner as status/diff.
use crate::{HerdrWork, code::bounded_git};
use anyhow::{Context, Result};
use herdr_work_code::{
    git::Git,
    scm::{Drawer, DrawerRef, drawer_git_spec, parse_drawer_ref},
};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct GitPlan {
    pub root: PathBuf,
    args: Vec<String>,
    instance: String,
    repository: String,
    head: Option<String>,
    index: u64,
    content: String,
    check: Vec<String>,
    reference: String,
    target: Option<(PathBuf, String)>,
}
use crate::work_git::working_stamp;

#[derive(Clone)]
pub struct DrawerView {
    pub root: PathBuf,
    pub kind: Drawer,
    pub file: Option<String>,
    pub lines: Vec<String>,
}
impl GitPlan {
    pub fn arguments(&self) -> &[String] {
        &self.args
    }
}
impl HerdrWork {
    pub fn prepare_scm_action(
        &self,
        view: &DrawerView,
        index: usize,
        action: herdr_work_code::scm::MenuAction,
    ) -> Result<GitPlan> {
        bounded_git(|| {
            use herdr_work_code::scm::{self, MenuEntry};
            let line = view.lines.get(index).context("未选择 Git 对象")?;
            let reference = parse_drawer_ref(view.kind, line);
            anyhow::ensure!(
                scm::reference_menu(&reference)
                    .iter()
                    .any(|e| matches!(e,MenuEntry::Action(a,_) if *a==action)),
                "此对象不支持此操作"
            );
            let operation =
                scm::reference_operation(&reference, action).context("不是 Git 修改操作")?;
            let git = Git::discover(&view.root).map_err(anyhow::Error::msg)?;
            let spec = drawer_git_spec(&reference).context("未选择 Git 引用")?;
            let (check, target) = match &reference {
                DrawerRef::Remote { name, .. } => (
                    vec![
                        "config".into(),
                        "--get".into(),
                        format!("remote.{name}.url"),
                    ],
                    None,
                ),
                DrawerRef::Worktree(path) => (
                    vec!["worktree".into(), "list".into(), "--porcelain".into()],
                    Some((
                        PathBuf::from(path),
                        Self::worktree_identity(Path::new(path))?,
                    )),
                ),
                _ => (
                    vec![
                        "rev-parse".into(),
                        "--verify".into(),
                        "--end-of-options".into(),
                        format!("{spec}^{{commit}}"),
                    ],
                    None,
                ),
            };
            let current = git
                .raw(&check.iter().map(String::as_str).collect::<Vec<_>>())
                .map_err(anyhow::Error::msg)?;
            Ok(GitPlan {
                root: git.root().into(),
                args: operation.args,
                instance: Self::worktree_identity(git.root())?,
                repository: crate::work_git::repository_identity(&git)?,
                content: working_stamp(&git)?,
                check,
                reference: crate::fingerprint(current.as_bytes()),
                target,
            })
        })
    }
    pub fn execute_git_plan(&self, plan: &GitPlan) -> Result<String> {
        bounded_git(|| {
            anyhow::ensure!(
                Self::worktree_identity(&plan.root)? == plan.instance,
                "工作目录实例已改变"
            );
            let git = Git::discover(&plan.root).map_err(anyhow::Error::msg)?;
            let _lock = crate::code::write_lock(&git)?;
            anyhow::ensure!(
                crate::work_git::repository_identity(&git)? == plan.repository,
                "仓库实例已改变"
            );
            anyhow::ensure!(
                    && working_stamp(&git)? == plan.content,
                "Git 状态已改变，请重新确认操作"
            );
            anyhow::ensure!(
                crate::fingerprint(
                    git.raw(&plan.check.iter().map(String::as_str).collect::<Vec<_>>())
                        .map_err(anyhow::Error::msg)?
                        .as_bytes()
                ) == plan.reference,
                "Git 引用已改变，请重新选择"
            );
            if let Some((path, identity)) = &plan.target {
                anyhow::ensure!(
                    Self::worktree_identity(path)? == *identity,
                    "目标工作目录已改变"
                );
            }
            let record = self.root().join("operations").join(format!(
                "git-{}.json",
                crate::fingerprint(&serde_json::to_vec(plan)?)
            ));
            if record.exists() {
                let old: serde_json::Value = crate::storage::load(&record)?;
                anyhow::ensure!(old["state"] == "finished", "上次操作结果未知，请先核对恢复");
            }
            crate::storage::save(&record, &serde_json::json!({"state":"started","plan":plan}))?;
            let result = git
                .raw(&plan.args.iter().map(String::as_str).collect::<Vec<_>>())
                .map_err(anyhow::Error::msg);
            crate::storage::save(
                &record,
            )?;
            result
        })
    }
    pub fn scm_drawer(&self, root: &Path, kind: Drawer, file: Option<&str>) -> Result<DrawerView> {
        bounded_git(|| {
            let git = Git::discover(root).map_err(anyhow::Error::msg)?;
            let lines = kind.load(&git, file, 100).map_err(anyhow::Error::msg)?;
            Ok(DrawerView {
                root: git.root().into(),
                kind,
                file: file.map(str::to_string),
                lines,
            })
        })
    }
    pub fn scm_show(&self, view: &DrawerView, index: usize) -> Result<String> {
        bounded_git(|| {
            let line = view.lines.get(index).context("未选择 Git 对象")?;
            let reference = parse_drawer_ref(view.kind, line);
            let git = Git::discover(&view.root).map_err(anyhow::Error::msg)?;
            let spec = drawer_git_spec(&reference).context("此行没有 Git 对象")?;
            if matches!(reference, DrawerRef::Remote { .. } | DrawerRef::Worktree(_)) {
                return Ok(line.clone());
            }
            let args = herdr_work_code::scm::show_args(
                &spec,
                if view.kind == Drawer::FileHistory {
                    view.file.as_deref()
                } else {
                    None
                },
            );
            git.raw(&args.iter().map(String::as_str).collect::<Vec<_>>())
                .map_err(anyhow::Error::msg)
        })
    }
}
