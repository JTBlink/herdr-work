use crate::{HerdrWork, storage};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use herdr_work_code::{git::Git, tree::Tree};
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileRow {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub directory: bool,
    pub symlink: bool,
    pub status: Option<char>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitChange {
    pub root: PathBuf,
    pub instance: String,
    pub path: String,
    pub original: Option<String>,
    pub status: char,
    pub staged: bool,
    pub expected_index: u64,
    pub expected_content: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeSnapshot {
    pub root: PathBuf,
    pub branch: String,
    pub files: Vec<FileRow>,
    pub changes: Vec<GitChange>,
    pub generation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffLine {
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub marker: char,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffView {
    pub root: PathBuf,
    pub file: String,
    pub scope: String,
    pub base: Option<String>,
    pub fingerprint: String,
    pub rows: Vec<DiffLine>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitPlan {
    pub root: PathBuf,
    pub instance: String,
    pub head: Option<String>,
    pub staged: u64,
}

fn git(root: &Path) -> Result<Git> {
    Git::discover(root).map_err(anyhow::Error::msg)
}
pub(crate) fn checked_path(root: &Path, file: &str) -> Result<PathBuf> {
    let p = Path::new(file);
    ensure!(
        !p.as_os_str().is_empty() && p.components().all(|c| matches!(c, Component::Normal(_))),
        "Expected a repository-relative file path"
    );
    let full = root.join(p);
    if full.exists() {
        ensure!(
            full.canonicalize()?.starts_with(root.canonicalize()?),
            "File escapes worktree"
        );
    }
    Ok(full)
}
pub(crate) fn write_lock(git: &Git) -> Result<std::fs::File> {
    let common = git
        .raw(&["rev-parse", "--git-common-dir"])
        .map_err(anyhow::Error::msg)?;
    let common = PathBuf::from(common.trim());
    let common = if common.is_absolute() {
        common
    } else {
        git.root().join(common)
    };
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(common.join("herdr-work-write.lock"))?;
    lock.try_lock_exclusive()
        .context("Another HerdrWork Git operation is running")?;
    Ok(lock)
}
pub(crate) fn bounded_git<T>(job: impl FnOnce() -> T) -> T {
    herdr_work_code::git::herdr_work_bounded(
        |command, timeout| {
            herdr_reviewr::proc::herdr_work_run(command, timeout).map_err(|e| format!("{e:#}"))
        },
        job,
    )
}
fn content_stamp(root: &Path, file: &str) -> Result<String> {
    let path = checked_path(root, file)?;
    if !path.exists() {
        return Ok("absent".into());
    }
    if path.is_dir() {
        let nested = git(&path)?;
        return Ok(storage::fingerprint(
            nested
                .raw(&["diff", "--binary", "HEAD"])
                .map_err(anyhow::Error::msg)?
                .as_bytes(),
        ));
    }
    use sha2::Digest;
    let mut stream = std::fs::File::open(path)?;
    let mut hash = sha2::Sha256::new();
    std::io::copy(&mut stream, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}
impl HerdrWork {
    pub fn code_snapshot(
        &self,
        root: &Path,
        expanded: &[PathBuf],
        generation: u64,
    ) -> Result<CodeSnapshot> {
        self.code_snapshot_with_hidden(root, expanded, generation, true)
    }
    pub fn code_snapshot_with_hidden(
        &self,
        root: &Path,
        expanded: &[PathBuf],
        generation: u64,
        show_hidden: bool,
    ) -> Result<CodeSnapshot> {
        bounded_git(|| {
            let root = root.canonicalize()?;
            let mut tree = Tree::new(root.clone());
            tree.show_hidden = show_hidden;
            tree.set_expanded(expanded.iter().cloned());
            let mut files: Vec<FileRow> = tree
                .rows()
                .into_iter()
                .map(|r| FileRow {
                    path: r.path,
                    name: r.name,
                    depth: r.depth,
                    directory: r.is_dir,
                    symlink: r.is_symlink,
                    status: None,
                })
                .collect();
            let mut changes = vec![];
            let mut decorations = vec![];
            let mut branch = String::new();
            for repo in Git::discover_all(&root) {
                let status = repo.status().map_err(anyhow::Error::msg)?;
                if branch.is_empty() {
                    branch = status.branch.clone();
                }
                decorations.push(herdr_work_code::gitdeco::RepoStatus {
                    root: repo.root().into(),
                    status: status.clone(),
                    ignored: repo.ignored().unwrap_or_default(),
                });
                let instance = Self::worktree_identity(repo.root())?;
                let expected_index = herdr_reviewr::git::staged_fingerprint(repo.root())?;
                for (staged, entries) in [(true, status.staged), (false, status.unstaged)] {
                    for e in entries {
                        let expected_content = content_stamp(repo.root(), &e.path)?;
                        changes.push(GitChange {
                            root: repo.root().into(),
                            instance: instance.clone(),
                            path: e.path,
                            original: e.orig,
                            status: e.letter,
                            staged,
                            expected_index,
                            expected_content,
                        });
                    }
                }
            }
            let decorations = herdr_work_code::gitdeco::Decorations::build(&decorations);
            for row in &mut files {
                row.status = decorations.letter(&row.path, row.directory);
            }
            Ok(CodeSnapshot {
                root,
                branch,
                files,
                changes,
                generation,
            })
        })
    }
    pub fn diff(
        &self,
        root: &Path,
        file: &str,
        scope: &str,
        base: Option<&str>,
    ) -> Result<DiffView> {
        bounded_git(|| {
            let root = git(root)?.root().canonicalize()?;
            let full = checked_path(&root, file)?;
            if full.exists() {
                ensure!(
                    full.metadata()?.len() <= 4 * 1024 * 1024,
                    "File exceeds the 4 MiB diff limit"
                );
            }
            let resolved_base = if scope == "branch" {
                let resolution = herdr_reviewr::git::resolve_base(&root, base)?;
                let winner = resolution
                    .status
                    .winner
                    .context("No branch comparison base; pass --base explicitly")?;
                Some(
                    herdr_reviewr::git::merge_base(&root, winner.oid())
                        .context("No common ancestor for comparison base")?,
                )
            } else {
                base.map(str::to_owned)
            };
            let base = resolved_base.as_deref();
            let repo = git(&root)?;
            let status = repo.status().map_err(anyhow::Error::msg)?;
            let original = status
                .staged
                .iter()
                .chain(&status.unstaged)
                .find(|e| e.path == file)
                .and_then(|e| e.orig.as_deref());
            let source = original.unwrap_or(file);
            let head = herdr_reviewr::git::diff_base(herdr_reviewr::git::head_oid(&root));
            let show = |rev: &str, path: &str| -> Result<String> {
                let listed = if rev.is_empty() {
                    repo.raw(&[
                        "--literal-pathspecs",
                        "ls-files",
                        "--stage",
                        "-z",
                        "--",
                        path,
                    ])
                } else {
                    repo.raw(&[
                        "--literal-pathspecs",
                        "ls-tree",
                        "--name-only",
                        "-z",
                        rev,
                        "--",
                        path,
                    ])
                }
                .map_err(anyhow::Error::msg)?;
                if listed.is_empty() {
                    return Ok(String::new());
                }
                let content = repo
                    .raw(&["show", &format!("{rev}:{path}")])
                    .map_err(anyhow::Error::msg)?;
                ensure!(
                    !content.contains('\0'),
                    "Binary diff has no commentable lines"
                );
                Ok(content)
            };
            let (old, new) = match scope {
                "staged" => (show(&head, source)?, show("", file)?),
                "unstaged" => (
                    show("", file)?,
                    if full.exists() {
                        std::fs::read_to_string(&full).context("Binary or unreadable file")?
                    } else {
                        String::new()
                    },
                ),
                "working" | "branch" => {
                    let old_rev = base.unwrap_or(&head);
                    if status
                        .unstaged
                        .iter()
                        .any(|e| e.path == file && e.letter == 'U')
                    {
                        (
                            String::new(),
                            std::fs::read_to_string(&full).context("Binary or unreadable file")?,
                        )
                    } else {
                        match herdr_reviewr::git::diff_sides(&root, old_rev, None, file, original)?
                        {
                            herdr_reviewr::git::DiffSides::Text { old, new } => (old, new),
                            herdr_reviewr::git::DiffSides::Binary => {
                                anyhow::bail!("Binary diff has no commentable lines")
                            }
                        }
                    }
                }
                _ => anyhow::bail!("Unknown diff scope: {scope}"),
            };
            let signature =
                storage::fingerprint(&serde_json::to_vec(&(scope, base, source, &old, &new))?);
            let highlighter = herdr_reviewr::highlight::Highlighter::new(
                herdr_reviewr::theme::resolve(None).syntax,
            );
            let diff = herdr_reviewr::diff::FileDiff::build(
                file.into(),
                original.map(str::to_owned),
                &old,
                &new,
                &highlighter,
            );
            let rows = diff
                .rows
                .iter()
                .map(|r| DiffLine {
                    old: r.old_no(),
                    new: r.new_no(),
                    marker: r.marker(),
                    text: r.text(),
                })
                .collect();
            Ok(DiffView {
                root,
                file: file.into(),
                scope: scope.into(),
                base: base.map(str::to_owned),
                fingerprint: signature,
                rows,
            })
        })
    }
    pub fn preview_file(
        &self,
        root: &Path,
        file: &str,
        line: Option<usize>,
        width: u16,
    ) -> Result<herdr_work_code::preview::Doc> {
        let root = root.canonicalize()?;
        let path = checked_path(&root, file)?;
        Ok(herdr_work_code::preview::file(&path, line, width))
    }
    pub fn preview_file_if_changed(
        &self,
        root: &Path,
        file: &str,
        baseline: Option<(std::time::SystemTime, u64)>,
        line: Option<usize>,
        width: u16,
    ) -> Result<Option<herdr_work_code::preview::Doc>> {
        let root = root.canonicalize()?;
        let path = checked_path(&root, file)?;
        let stamp = path
            .metadata()
            .ok()
            .and_then(|meta| Some((meta.modified().ok()?, meta.len())));
        if baseline.is_some() && baseline == stamp {
            return Ok(None);
        }
        Ok(Some(herdr_work_code::preview::file(&path, line, width)))
    }
    pub fn file(&self, root: &Path, file: &str) -> Result<String> {
        let root = root.canonicalize()?;
        let path = checked_path(&root, file)?;
        ensure!(
            path.metadata()?.len() <= 4 * 1024 * 1024,
            "File exceeds the 4 MiB preview limit"
        );
        std::fs::read_to_string(path).context("Binary or unreadable file")
    }
    pub fn stage(&self, change: &GitChange, staged: bool) -> Result<()> {
        bounded_git(|| {
            ensure!(
                Self::worktree_identity(&change.root)? == change.instance,
                "Worktree instance changed; refresh SCM"
            );
            let repo = git(&change.root)?;
            let _lock = write_lock(&repo)?;
            ensure!(
                herdr_reviewr::git::staged_fingerprint(repo.root())? == change.expected_index
                    && content_stamp(repo.root(), &change.path)? == change.expected_content,
                "Index or working content changed; refresh before staging"
            );
            checked_path(repo.root(), &change.path)?;
            let status = repo.status().map_err(anyhow::Error::msg)?;
            let entries = if staged {
                status.unstaged
            } else {
                status.staged
            };
            let entry = entries
                .iter()
                .find(|e| e.path == change.path && e.orig == change.original)
                .context("SCM entry changed; refresh before retrying")?;
            if staged {
                repo.stage(entry)
            } else {
                repo.unstage(entry)
            }
            .map_err(anyhow::Error::msg)
        })
    }
    pub fn prepare_commit(&self, root: &Path) -> Result<CommitPlan> {
        bounded_git(|| {
            let repo = git(root)?;
            ensure!(
                !repo.status().map_err(anyhow::Error::msg)?.staged.is_empty(),
                "No staged changes"
            );
            Ok(CommitPlan {
                root: repo.root().canonicalize()?,
                instance: Self::worktree_identity(repo.root())?,
                head: herdr_reviewr::git::head_oid(repo.root()),
                staged: herdr_reviewr::git::staged_fingerprint(repo.root())?,
            })
        })
    }
    pub fn commit(&self, plan: &CommitPlan, message: &str) -> Result<String> {
        bounded_git(|| {
            ensure!(!message.trim().is_empty(), "Commit message is empty");
            ensure!(
                Self::worktree_identity(&plan.root)? == plan.instance,
                "Worktree instance changed; prepare commit again"
            );
            let repo = git(&plan.root)?;
            let _lock = write_lock(&repo)?;
            ensure!(
                herdr_reviewr::git::head_oid(repo.root()) == plan.head
                    && herdr_reviewr::git::staged_fingerprint(repo.root())? == plan.staged,
                "HEAD or staged content changed; review again"
            );
            let operation = self.root().join("operations").join(format!(
                "commit-{}.json",
                storage::key(&plan.root.to_string_lossy())
            ));
            if operation.exists() {
                let old: serde_json::Value = storage::load(&operation)?;
                ensure!(
                    old["state"] == "finished",
                    "Previous commit needs explicit reconciliation"
                );
            }
            storage::save(
                &operation,
                &serde_json::json!({"state":"started","plan":plan}),
            )?;
            let result = repo.commit(message).map_err(anyhow::Error::msg);
            let head = herdr_reviewr::git::head_oid(repo.root());
            storage::save(
                &operation,
                &serde_json::json!({"state":if result.is_ok() || head == plan.head {"finished"} else {"unknown"},"head":head,"ok":result.is_ok()}),
            )?;
            result
        })
    }
}
