//! Confirmed worktree operations. Algorithms belong to the imported Sidebar Git model.
use crate::{
    HerdrWork,
    code::{bounded_git, checked_path, write_lock},
    storage,
};
use anyhow::{Context, Result, ensure};
use herdr_work_code::git::{Branch, DeleteRefused, Git};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
pub enum WorkGitAction {
    StageAll,
    UnstageAll,
    StageUnder(String),
    UnstageUnder(String),
    Discard(String),
    DiscardAll,
    Stash,
    Sync,
    Checkout { name: String, remote: bool },
    CreateBranch(String),
    DeleteBranch(String),
    ForceDeleteBranch(String),
}
#[derive(Clone, Serialize)]
pub struct WorkGitPlan {
    pub root: PathBuf,
    action: WorkGitAction,
    instance: String,
    repository: String,
    head: Option<String>,
    index: u64,
    content: String,
    references: String,
    targets: Vec<String>,
}
pub enum WorkGitOutcome {
    Finished(String),
    DeleteNotMerged(WorkGitPlan),
}
impl WorkGitPlan {
    pub fn action(&self) -> &WorkGitAction {
        &self.action
    }
    pub fn description(&self) -> String {
        format!(
            "{}\n{:?} · 状态快照 {} 个路径\n{}",
            self.root.display(),
            self.action,
            self.targets.len(),
            self.targets
                .iter()
                .map(|path| format!("{path:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
pub(crate) fn repository_identity(git: &Git) -> Result<String> {
    let mut identities = vec![];
    for option in ["--absolute-git-dir", "--git-common-dir"] {
        let path = git
            .raw(&["rev-parse", option])
            .map_err(anyhow::Error::msg)?;
        let path = PathBuf::from(path.trim());
        identities.push(HerdrWork::worktree_identity(&if path.is_absolute() {
            path
        } else {
            git.root().join(path)
        })?);
    }
    Ok(identities.join("|"))
}
/// Include status, tracked patches, untracked bytes and symlink targets. A status-only
/// fingerprint cannot detect editing an untracked file while a confirmation is open.
pub(crate) fn working_stamp(git: &Git) -> Result<String> {
    use sha2::Digest;
    let mut hash = sha2::Sha256::new();
    let status = git.status().map_err(anyhow::Error::msg)?;
    hash.update(format!("{status:?}"));
    hash.update(git.raw(&["diff", "--binary"]).map_err(anyhow::Error::msg)?);
    let mut remaining = 256 * 1024 * 1024u64;
    fn visit(
        path: &Path,
        hash: &mut sha2::Sha256,
        remaining: &mut u64,
        depth: usize,
    ) -> Result<()> {
        use sha2::Digest;
        ensure!(depth < 64, "Git 确认目录层级超过限制");
        let meta = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                hash.update(b"absent");
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };
        if meta.file_type().is_symlink() {
            hash.update(b"link");
            hash.update(std::fs::read_link(path)?.as_os_str().as_encoded_bytes());
        } else if meta.is_dir() {
            hash.update(b"dir");
            if path.join(".git").exists() {
                hash.update(HerdrWork::worktree_identity(&path.join(".git"))?);
            }
            let mut entries = std::fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                if entry.file_name() != ".git" {
                    hash.update(entry.file_name().as_encoded_bytes());
                    visit(&entry.path(), hash, remaining, depth + 1)?;
                }
            }
        } else {
            ensure!(meta.is_file(), "Git 确认不支持特殊文件");
            ensure!(meta.len() <= *remaining, "Git 确认内容超过 256 MiB 限制");
            *remaining -= meta.len();
            hash.update(b"file");
            hash.update(meta.len().to_le_bytes());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                hash.update(meta.permissions().mode().to_le_bytes());
            }
            let mut file = std::fs::File::open(path)?;
            std::io::copy(&mut file, hash)?;
        }
        Ok(())
    }
    let mut paths = std::collections::BTreeSet::new();
    for entry in status.staged.iter().chain(&status.unstaged) {
        paths.insert(entry.path.trim_end_matches('/').to_string());
        paths.extend(entry.orig.iter().cloned());
    }
    for path in paths {
        hash.update(path.as_bytes());
        visit(
            &checked_path(git.root(), &path)?,
            &mut hash,
            &mut remaining,
            0,
        )?;
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn references(git: &Git) -> Result<String> {
    Ok(crate::fingerprint(
        format!(
            "{}{}",
            git.raw(&[
                "for-each-ref",
                "--format=%(refname) %(objectname) %(upstream)"
            ])
            .map_err(anyhow::Error::msg)?,
            git.raw(&["config", "--local", "--list"])
                .map_err(anyhow::Error::msg)?
        )
        .as_bytes(),
    ))
}
impl HerdrWork {
    pub fn prepare_explorer_stage(&self, target: &Path) -> Result<WorkGitPlan> {
        bounded_git(|| {
            let repo = Git::owner_of(target).map_err(anyhow::Error::msg)?;
            let target = target
                .parent()
                .context("目标缺少父目录")?
                .canonicalize()?
                .join(target.file_name().context("目标缺少名称")?);
            let path = target
                .strip_prefix(repo.root())?
                .to_string_lossy()
                .into_owned();
            if path.is_empty() {
                self.prepare_work_git(repo.root(), WorkGitAction::StageAll)
            } else {
                self.prepare_work_git(repo.root(), WorkGitAction::StageUnder(path))
            }
        })
    }
    pub fn prepare_work_git(&self, root: &Path, action: WorkGitAction) -> Result<WorkGitPlan> {
        bounded_git(|| {
            let repo = Git::discover(root).map_err(anyhow::Error::msg)?;
            ensure!(
                repo.root().canonicalize()? == root.canonicalize()?,
                "请选择明确的仓库根目录"
            );
            match &action {
                WorkGitAction::StageUnder(path)
                | WorkGitAction::UnstageUnder(path)
                | WorkGitAction::Discard(path) => {
                    checked_path(repo.root(), path)?;
                }
                WorkGitAction::Sync => ensure!(
                    repo.status().map_err(anyhow::Error::msg)?.has_upstream,
                    "当前分支没有 upstream；请先设置 tracking 分支"
                ),
                WorkGitAction::Checkout { name, remote } => ensure!(
                    repo.branch_choices()
                        .map_err(anyhow::Error::msg)?
                        .iter()
                        .any(|b| &b.name == name && b.remote == *remote),
                    "分支已改变，请重新选择"
                ),
                WorkGitAction::ForceDeleteBranch(_) => {
                    anyhow::bail!("强制删除只能由未合并拒绝后的第二次确认生成")
                }
                _ => {}
            }
            Ok(WorkGitPlan {
                root: repo.root().into(),
                action,
                instance: Self::worktree_identity(repo.root())?,
                repository: repository_identity(&repo)?,
                head: herdr_reviewr::git::head_oid(repo.root()),
                index: herdr_reviewr::git::staged_fingerprint(repo.root())?,
                content: working_stamp(&repo)?,
                references: references(&repo)?,
                targets: repo
                    .status()
                    .map_err(anyhow::Error::msg)?
                    .unstaged
                    .iter()
                    .chain(&repo.status().map_err(anyhow::Error::msg)?.staged)
                    .map(|e| e.path.clone())
                    .collect(),
            })
        })
    }
    pub fn execute_work_git(&self, plan: &WorkGitPlan) -> Result<WorkGitOutcome> {
        bounded_git(|| {
            ensure!(
                Self::worktree_identity(&plan.root)? == plan.instance,
                "工作目录实例已改变"
            );
            let repo = Git::discover(&plan.root).map_err(anyhow::Error::msg)?;
            let _lock = write_lock(&repo)?;
            ensure!(
                repo.root() == plan.root && repository_identity(&repo)? == plan.repository,
                "仓库实例已改变"
            );
            ensure!(
                herdr_reviewr::git::head_oid(repo.root()) == plan.head
                    && herdr_reviewr::git::staged_fingerprint(repo.root())? == plan.index
                    && working_stamp(&repo)? == plan.content
                    && references(&repo)? == plan.references,
                "Git 状态已改变，请重新确认操作"
            );
            let record = self.root().join("operations").join(format!(
                "git-{}.json",
                crate::fingerprint(&serde_json::to_vec(plan)?)
            ));
            if record.exists() {
                let old: serde_json::Value = storage::load(&record)?;
                ensure!(old["state"] == "finished", "上次操作结果未知，请先核对恢复");
            }
            storage::save(
                &record,
                &serde_json::json!({"state":"started", "plan":plan}),
            )?;
            let result: Result<String> = (|| {
                match &plan.action {
                    WorkGitAction::StageAll => {
                        let done = repo.stage_under(repo.root()).map_err(anyhow::Error::msg)?;
                        return Ok(format!(
                            "暂存 {} 个路径，跳过 {} 个嵌套仓库路径",
                            done.count, done.skipped_nested
                        ));
                    }
                    WorkGitAction::UnstageAll => repo.unstage_all(),
                    WorkGitAction::StageUnder(path) => {
                        repo.stage_under(&repo.root().join(path))
                            .map_err(anyhow::Error::msg)?;
                        Ok(())
                    }
                    WorkGitAction::UnstageUnder(path) => repo.unstage_under(path).map(drop),
                    WorkGitAction::Discard(path) => {
                        let entry = repo
                            .status()
                            .map_err(anyhow::Error::msg)?
                            .unstaged
                            .into_iter()
                            .find(|e| &e.path == path)
                            .context("未暂存更改已失效")?;
                        repo.discard(&entry)
                    }
                    WorkGitAction::DiscardAll => repo
                        .discard_entries(&repo.status().map_err(anyhow::Error::msg)?.unstaged)
                        .map(drop),
                    WorkGitAction::Stash => repo
                        .raw(&["stash", "push", "--include-untracked"])
                        .map(drop),
                    WorkGitAction::Sync => return repo.sync().map_err(anyhow::Error::msg),
                    WorkGitAction::Checkout { name, remote } => repo.checkout_branch(&Branch {
                        name: name.clone(),
                        remote: *remote,
                        current: false,
                    }),
                    WorkGitAction::CreateBranch(name) => repo.create_branch(name),
                    WorkGitAction::DeleteBranch(name) => match repo.delete_branch(name) {
                        Ok(()) => Ok(()),
                        Err(DeleteRefused::NotMerged) => return Ok("DELETE_NOT_MERGED".into()),
                        Err(DeleteRefused::Failed(error)) => Err(error),
                    },
                    WorkGitAction::ForceDeleteBranch(name) => repo.force_delete_branch(name),
                }
                .map_err(anyhow::Error::msg)?;
                Ok("Git 操作完成".into())
            })();
            storage::save(
                &record,
                &serde_json::json!({"state":if result.is_ok() {"finished"} else {"unknown"}, "plan":plan, "head":herdr_reviewr::git::head_oid(repo.root()), "error":result.as_ref().err().map(|e|e.to_string())}),
            )?;
            match result? {
                text if text == "DELETE_NOT_MERGED" => {
                    let WorkGitAction::DeleteBranch(name) = &plan.action else {
                        unreachable!()
                    };
                    let mut force = self
                        .prepare_work_git(&plan.root, WorkGitAction::DeleteBranch(name.clone()))?;
                    force.action = WorkGitAction::ForceDeleteBranch(name.clone());
                    Ok(WorkGitOutcome::DeleteNotMerged(force))
                }
                text => Ok(WorkGitOutcome::Finished(text)),
            }
        })
    }
    pub fn branch_picker(&self, root: &Path) -> Result<herdr_work_code::branch_ui::BranchPicker> {
        bounded_git(|| {
            herdr_work_code::branch_ui::BranchPicker::open(
                Git::discover(root).map_err(anyhow::Error::msg)?,
            )
            .map_err(anyhow::Error::msg)
        })
    }
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CommitDraft {
    pub message: Vec<char>,
    pub cursor: usize,
}
impl HerdrWork {
    pub fn git_draft(&self, root: &Path) -> Result<CommitDraft> {
        let path = self
            .root()
            .join("git-drafts")
            .join(format!("{}.json", storage::key(&root.to_string_lossy())));
        let mut draft: CommitDraft = if path.exists() {
            storage::load(&path)?
        } else {
            CommitDraft::default()
        };
        ensure!(draft.message.len() <= 65536, "提交草稿超过限制；保留原文件");
        draft.cursor = draft.cursor.min(draft.message.len());
        Ok(draft)
    }
    pub fn save_git_draft(&self, root: &Path, draft: &CommitDraft) -> Result<()> {
        ensure!(
            draft.message.len() <= 65536 && draft.cursor <= draft.message.len(),
            "提交消息超过限制或光标无效"
        );
        storage::save(
            &self
                .root()
                .join("git-drafts")
                .join(format!("{}.json", storage::key(&root.to_string_lossy()))),
            draft,
        )
    }
}

impl HerdrWork {
    /// Same explicit resolution records as the CLI, enriched with a fresh observation.
    pub fn git_reconciliation(&self) -> Result<serde_json::Value> {
        bounded_git(|| {
            let mut value = self.reconcile_tasks()?;
            if let Some(operations) = value["operations"].as_array_mut() {
                for operation in operations {
                    let root = operation["record"]["plan"]["root"]
                        .as_str()
                        .map(PathBuf::from);
                    if let Some(root) = root {
                        let observed = (|| -> Result<serde_json::Value> {
                            let repo = Git::discover(&root).map_err(anyhow::Error::msg)?;
                            Ok(
                                serde_json::json!({"root":repo.root(), "instance":Self::worktree_identity(repo.root())?, "head":herdr_reviewr::git::head_oid(repo.root()), "status":format!("{:?}", repo.status().map_err(anyhow::Error::msg)?)}),
                            )
                        })();
                        operation["record"]["observed"] =
                            observed.unwrap_or_else(|e| serde_json::json!({"error":e.to_string()}));
                    }
                }
            }
            Ok(value)
        })
    }
}

impl HerdrWork {
    pub fn prepare_branch_checkout(&self, root: &Path, name: &str) -> Result<WorkGitPlan> {
        bounded_git(|| {
            let repo = Git::discover(root).map_err(anyhow::Error::msg)?;
            let branch = repo
                .branch_choices()
                .map_err(anyhow::Error::msg)?
                .into_iter()
                .find(|b| b.name == name)
                .context("分支已改变，请重新选择")?;
            self.prepare_work_git(
                root,
                WorkGitAction::Checkout {
                    name: branch.name,
                    remote: branch.remote,
                },
            )
        })
    }
}

impl HerdrWork {
    pub fn suggest_commit(&self, root: &Path) -> Result<String> {
        let (diff, files) =
            bounded_git(|| Git::discover(root).and_then(|repo| repo.diff_for_message()))
                .map_err(anyhow::Error::msg)?;
        Ok(herdr_work_code::suggest::generate(&diff, &files))
    }
}

impl HerdrWork {
    pub fn unresolved_git_operations(&self) -> Result<Vec<(String, String)>> {
        let value = self.git_reconciliation()?;
        Ok(value["operations"].as_array().into_iter().flatten().filter(|o|o["record"]["state"] != "finished").map(|o| {
            let r = &o["record"];
            let action = r["plan"].get("action").unwrap_or(&r["plan"]["args"]);
            let details = format!("工作目录：{}\n操作：{}\n准备时 HEAD：{}\n当前 HEAD：{}\n当前状态：{}\n错误：{}", r["plan"]["root"].as_str().unwrap_or("记录未包含工作目录，请手动核对"), action, r["plan"]["head"], r["observed"]["head"], r["observed"]["status"].as_str().unwrap_or("需手动核对"), r["error"].as_str().unwrap_or("未知结果"));
            (o["operation"].as_str().unwrap_or_default().to_string(), details)
        }).collect())
    }
}
