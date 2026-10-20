//! OS-backed pane mutation lock moved from Sidebar ensure.rs.
use std::fs::File;

/// Serialize concurrent runs (pane/tab events arrive in bursts; unguarded,
/// one switch opened four panes). The OS releases this lock if a launcher
/// crashes, so no retry timer or stale-lock cleanup is needed.
pub struct LaunchLock {
    _file: File,
    _legacy: Option<File>,
}

impl LaunchLock {
    /// Acquire the shared launcher lock. Discrete user actions should wait;
    /// redundant focus hooks should use a non-blocking attempt and yield.
    pub fn acquire(wait: bool) -> Option<Self> {
        Self::acquire_at(crate::rundir::dir("launch"), wait, true)
    }

    /// Same lock as the source UI, without configuring process-global state.
    pub fn acquire_profile(profile: &std::path::Path, wait: bool) -> Option<Self> {
        Self::acquire_at(profile.join("sidebar/launch"), wait, false)
    }

    fn acquire_at(dir: std::path::PathBuf, wait: bool, compatibility: bool) -> Option<Self> {
        crate::rundir::ensure_private(&dir).ok()?;
        let path = dir.join("launcher.lock");
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .ok()?;
        let acquired = if wait {
            file.lock().is_ok()
        } else {
            file.try_lock().is_ok()
        };
        if !acquired {
            return None;
        }
        // During one upgrade window, also take the pre-runtime-dir lock so
        // already-running old hooks and new hooks cannot launch concurrently.
        let legacy = if let Some(legacy_path) = crate::state::state_path()
            .map(|path| path.with_file_name("launcher.lock"))
            .filter(|legacy| {
                compatibility && crate::state::product_profile().is_none() && *legacy != path
            }) {
            match File::options()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(legacy_path)
            {
                Ok(file) => {
                    let acquired = if wait {
                        file.lock().is_ok()
                    } else {
                        file.try_lock().is_ok()
                    };
                    if !acquired {
                        return None;
                    }
                    Some(file)
                }
                // Compatibility must not make the new private lock unusable
                // forever because a stale old path cannot be opened.
                Err(_) => None,
            }
        } else {
            None
        };
        Some(Self {
            _file: file,
            _legacy: legacy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launcher_and_focus_hooks_share_one_product_layout_lock() {
        let profile = std::env::temp_dir().join(format!(
            "herdr-work-layout-lock-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let lock = LaunchLock::acquire_profile(&profile, false).unwrap();
        assert!(LaunchLock::acquire_profile(&profile, false).is_none());
        drop(lock);
        assert!(LaunchLock::acquire_profile(&profile, false).is_some());
        std::fs::remove_dir_all(profile).unwrap();
    }
}
