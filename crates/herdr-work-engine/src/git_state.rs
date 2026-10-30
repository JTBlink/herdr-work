//! Git identity and index observations through the shared Code client.
use anyhow::{Context, Result};
use herdr_work_code::git::Git;
use std::{
    hash::{Hash, Hasher},
    path::Path,
};

pub(crate) const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
fn query(root: &Path, args: &[&str]) -> Result<String> {
    Git::discover(root)
        .and_then(|git| git.raw(args))
        .map_err(anyhow::Error::msg)
}
pub(crate) fn head_oid(root: &Path) -> Option<String> {
    query(root, &["rev-parse", "--verify", "-q", "HEAD"])
        .ok()
        .map(|head| head.trim().to_owned())
        .filter(|head| !head.is_empty())
}
pub(crate) fn staged_fingerprint(root: &Path) -> Result<u64> {
    let staged = query(root, &["ls-files", "--stage", "-z"])?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    staged.hash(&mut hash);
    Ok(hash.finish())
}
pub(crate) fn diff_base(head: Option<String>) -> String {
    head.unwrap_or_else(|| EMPTY_TREE.to_owned())
}
pub(crate) fn comparison_base(root: &Path, explicit: Option<&str>) -> Result<String> {
    let candidates = if let Some(base) = explicit {
        vec![base.to_owned()]
    } else {
        let mut candidates = Vec::new();
        if let Ok(remote) = query(
            root,
            &["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"],
        ) {
            candidates.push(remote.trim().to_owned());
        }
        candidates.extend(
            [
                "refs/remotes/origin/main",
                "refs/remotes/origin/master",
                "refs/heads/main",
                "refs/heads/master",
            ]
            .map(str::to_owned),
        );
        candidates
    };
    for candidate in candidates {
        let Ok(oid) = query(
            root,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{candidate}^{{commit}}"),
            ],
        ) else {
            continue;
        };
        if let Ok(base) = query(root, &["merge-base", "HEAD", oid.trim()]) {
            return Ok(base.trim().to_owned());
        }
    }
    None.context("No common comparison base; pass --base explicitly")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    fn git(root: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    fn repository() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        git(temp.path(), &["init", "-q", "--initial-branch=main"]);
        git(temp.path(), &["config", "user.name", "Fixture"]);
        git(
            temp.path(),
            &["config", "user.email", "fixture@example.invalid"],
        );
        temp
    }
    #[test]
    fn index_fingerprint_ignores_unstaged_edits_and_changes_when_staged() {
        let temp = repository();
        let root = temp.path();
        assert!(head_oid(root).is_none());
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        git(root, &["add", "."]);
        let first = staged_fingerprint(root).unwrap();
        std::fs::write(root.join("a.txt"), "two\n").unwrap();
        assert_eq!(first, staged_fingerprint(root).unwrap());
        git(root, &["add", "."]);
        assert_ne!(first, staged_fingerprint(root).unwrap());
    }
    #[test]
    fn comparison_uses_the_common_ancestor_and_rejects_invalid_explicit_refs() {
        let temp = repository();
        let root = temp.path();
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "seed"]);
        let seed = head_oid(root).unwrap();
        git(root, &["checkout", "-qb", "topic"]);
        std::fs::write(root.join("a.txt"), "two\n").unwrap();
        git(root, &["commit", "-am", "topic", "-q"]);
        assert_eq!(comparison_base(root, None).unwrap(), seed);
        assert_eq!(comparison_base(root, Some("main")).unwrap(), seed);
        assert!(comparison_base(root, Some("--help")).is_err());
    }
}
