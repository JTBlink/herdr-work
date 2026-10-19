//! Resolve the public profile path and migrate an idle legacy profile without replacing new data.
use crate::storage;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

fn variable(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
fn home() -> PathBuf {
    variable("HOME")
        .or_else(|| variable("USERPROFILE"))
        .unwrap_or_else(|| PathBuf::from("."))
}
pub(crate) fn default_root() -> PathBuf {
    variable("HERDR_WORK_HOME")
        .or_else(|| variable("WORKDESK_HOME"))
        .unwrap_or_else(|| home().join(".herdr-work"))
}
pub(crate) fn resolve(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = explicit
        .or_else(|| variable("HERDR_WORK_HOME"))
        .or_else(|| variable("WORKDESK_HOME"))
    {
        return Ok(std::path::absolute(root)?);
    }
    migrate_legacy(&home())
}
pub(crate) fn parent_lock(root: &Path) -> Result<File> {
    let parent = root.parent().context("Profile has no parent directory")?;
    std::fs::create_dir_all(parent)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join(".herdr-work-profile-migration.lock"))?;
    lock.lock_exclusive()?;
    Ok(lock)
}
pub(crate) fn profile_lock(root: &Path, name: &str) -> Result<File> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(name))?;
    lock.try_lock_exclusive()
        .context("数据目录仍被使用，请退出旧界面和旧 Agent 后重试")?;
    Ok(lock)
}
fn inventory(root: &Path) -> Result<(Vec<PathBuf>, String)> {
    fn walk(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            let relative = path.strip_prefix(root)?;
            if ["workdesk.lock", "herdr-work.lock"]
                .iter()
                .any(|name| relative == Path::new(name))
            {
                continue;
            }
            let meta = path.symlink_metadata()?;
            ensure!(
                !meta.file_type().is_symlink(),
                "旧数据包含符号链接，请使用 --home 指定原目录并手动检查"
            );
            if meta.is_dir() {
                walk(root, &path, files)?;
            } else {
                ensure!(meta.is_file(), "旧数据包含非常规文件");
                files.push(relative.to_path_buf());
            }
        }
        Ok(())
    }
    let mut files = vec![];
    walk(root, root, &mut files)?;
    files.sort();
    let mut hashes = vec![];
    for relative in &files {
        let bytes = std::fs::read(root.join(relative))?;
        hashes.push((relative, storage::fingerprint(&bytes)));
        if relative.starts_with("projects/data/tasks")
            && relative.file_name().is_some_and(|name| name == "task.json")
        {
            let task: Value =
                serde_json::from_slice(&bytes).context("旧任务记录损坏，原数据保留")?;
            ensure!(
                task["link"]["pane"].as_str().unwrap_or("").is_empty()
                    || matches!(
                        task["stage"].as_str(),
                        Some("done" | "stopped" | "cancelled" | "failed")
                    ),
                "旧目录仍有关联 Agent 的未结束任务，请先结束旧任务，或使用 --home 指定原目录"
            );
        }
    }
    let digest = storage::fingerprint(&serde_json::to_vec(&hashes)?);
    Ok((files, digest))
}
fn migrate_legacy(home: &Path) -> Result<PathBuf> {
    let target = std::path::absolute(home.join(".herdr-work"))?;
    let _parent = parent_lock(&target)?;
    if target.exists() {
        ensure!(target.is_dir(), "默认数据目录不是文件夹");
        return Ok(target);
    }
    let source = home.join(".workdesk");
    if !source.exists() {
        return Ok(target);
    }
    ensure!(
        source.is_dir() && !source.symlink_metadata()?.file_type().is_symlink(),
        "旧数据目录不是普通文件夹，请显式指定 --home 检查"
    );
    let _legacy = profile_lock(&source, "workdesk.lock")?;
    let _current = if source.join("herdr-work.lock").exists() {
        Some(profile_lock(&source, "herdr-work.lock")?)
    } else {
        None
    };
    let (files, digest) = inventory(&source)?;
    let stage = tempfile::Builder::new()
        .prefix(".herdr-work-migrate-")
        .tempdir_in(home)?;
    for relative in files {
        let dest = stage.path().join(&relative);
        std::fs::create_dir_all(dest.parent().unwrap())?;
        std::fs::copy(source.join(relative), dest)?;
    }
    ensure!(
        inventory(&source)?.1 == digest && inventory(stage.path())?.1 == digest,
        "迁移期间旧数据发生变化，未启用新目录；请结束旧程序后重试"
    );
    storage::save(
        &stage.path().join("profile-migration.json"),
        &json!({"source":source,"digest":digest,"original_retained":true}),
    )?;
    ensure!(!target.exists(), "新数据目录已经存在，拒绝覆盖");
    std::fs::rename(stage.path(), &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_idle_profile_once_and_preserves_original_and_newer_data() {
        let home = tempfile::tempdir().unwrap();
        let old = home.path().join(".workdesk");
        std::fs::create_dir_all(old.join("reviews")).unwrap();
        std::fs::write(old.join("reviews/draft.json"), b"saved draft").unwrap();
        let new = migrate_legacy(home.path()).unwrap();
        assert_eq!(new, home.path().join(".herdr-work"));
        assert_eq!(
            std::fs::read(new.join("reviews/draft.json")).unwrap(),
            b"saved draft"
        );
        assert_eq!(
            std::fs::read(old.join("reviews/draft.json")).unwrap(),
            b"saved draft"
        );
        std::fs::write(new.join("reviews/draft.json"), b"newer draft").unwrap();
        migrate_legacy(home.path()).unwrap();
        assert_eq!(
            std::fs::read(new.join("reviews/draft.json")).unwrap(),
            b"newer draft"
        );
    }
    #[test]
    fn active_legacy_profile_and_linked_tasks_are_not_copied() {
        let home = tempfile::tempdir().unwrap();
        let old = home.path().join(".workdesk");
        std::fs::create_dir(&old).unwrap();
        let lock = profile_lock(&old, "workdesk.lock").unwrap();
        assert!(migrate_legacy(home.path()).is_err());
        drop(lock);
        std::fs::create_dir_all(old.join("projects/data/tasks/1")).unwrap();
        std::fs::write(
            old.join("projects/data/tasks/1/task.json"),
            br#"{"stage":"working","link":{"pane":"agent"}}"#,
        )
        .unwrap();
        assert!(migrate_legacy(home.path()).is_err());
        assert!(!home.path().join(".herdr-work").exists());
    }
    #[cfg(unix)]
    #[test]
    fn symbolic_legacy_entries_do_not_escape_into_new_profile() {
        let home = tempfile::tempdir().unwrap();
        let old = home.path().join(".workdesk");
        std::fs::create_dir(&old).unwrap();
        std::os::unix::fs::symlink(home.path(), old.join("outside")).unwrap();
        assert!(migrate_legacy(home.path()).is_err());
        assert!(!home.path().join(".herdr-work").exists());
    }
}
