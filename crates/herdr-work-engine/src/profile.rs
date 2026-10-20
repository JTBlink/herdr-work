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

pub(crate) fn sidebar_lock(root: &Path) -> Result<File> {
    sidebar_lock_for(
        root,
        &std::env::var("HERDR_SOCKET_PATH").unwrap_or_default(),
        &std::env::var("HERDR_PANE_ID").unwrap_or_else(|_| "local".into()),
    )
}
fn sidebar_lock_for(root: &Path, socket: &str, pane: &str) -> Result<File> {
    let dir = root.join("sidebar/instances");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!(
        "{}.lock",
        crate::storage::key(&format!("{socket}\0{pane}"))
    ));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock_exclusive()
        .context("此 HerdrWork 侧栏窗格仍被使用")?;
    Ok(file)
}

#[cfg(test)]
mod sidebar_tests {
    #[test]
    fn sidebar_ownership_is_per_session_and_pane_without_locking_business_data() {
        let root = tempfile::tempdir().unwrap();
        let _a = super::sidebar_lock_for(root.path(), "session-a", "pane-1").unwrap();
        assert!(super::sidebar_lock_for(root.path(), "session-a", "pane-1").is_err());
        let _b = super::sidebar_lock_for(root.path(), "session-a", "pane-2").unwrap();
        let _c = super::sidebar_lock_for(root.path(), "session-b", "pane-1").unwrap();
        let _business = super::profile_lock(root.path()).unwrap();
    }
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
