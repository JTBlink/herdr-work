//! Resolve HerdrWork configuration and enforce exclusive profile ownership.
use anyhow::{Context, Result};
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

fn variable(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
pub(crate) fn default_root() -> PathBuf {
    variable("HERDR_WORK_HOME").unwrap_or_else(|| {
        variable("HOME")
            .or_else(|| variable("USERPROFILE"))
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".herdr-work")
    })
}
pub(crate) fn resolve(explicit: Option<PathBuf>) -> Result<PathBuf> {
    Ok(std::path::absolute(explicit.unwrap_or_else(default_root))?)
}
pub(crate) fn profile_lock(root: &Path) -> Result<File> {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("herdr-work.lock"))?;
    lock.try_lock_exclusive()
        .context("HerdrWork 数据目录仍被使用，请退出已有实例后重试")?;
    Ok(lock)
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn final_profile_owner_releases_lock_with_inherited_descriptor() {
        let root = tempfile::tempdir().unwrap();
        let desk = crate::HerdrWork::open(root.path(), None).unwrap();
        // A dup shares the open file description, as a concurrent fork does
        // before exec. Closing only the parent's fd leaves its flock alive.
        let _inherited = desk.0._lock.try_clone().unwrap();
        let last_owner = desk.clone();
        drop(desk);
        assert!(crate::HerdrWork::open(root.path(), None).is_err());
        drop(last_owner);
        assert!(crate::HerdrWork::open(root.path(), None).is_ok());
    }
}
