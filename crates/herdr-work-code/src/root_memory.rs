//! Root persistence moved mechanically from the original Sidebar main.rs.
use std::time::Duration;

/// This process's root and where it is remembered across restarts.
///
/// The key's label is re-read on every save. Herdr names an unlabelled
/// workspace after its live folder, so a workspace created from another
/// project's folder starts with that project's label — and therefore that
/// project's key — until its pane `cd`s elsewhere. With a startup-only label,
/// following that `cd` wrote the new folder over the other project's entry,
/// and every sidebar later opened in that project (or switching views there)
/// came up in the wrong folder.
#[derive(Clone)]
pub struct RootMemory {
    pub root: std::path::PathBuf,
    pub startup_label: String,
    pub spawn_cwd: std::path::PathBuf,
    /// A root change not yet persisted because the live label was unknown.
    pub pending: bool,
    pub last_attempt: Option<std::time::Instant>,
}

/// How often a pending save retries the live-label lookup. The lookup is a
/// socket round trip on the UI thread, so a host that is not answering must
/// not be asked on every loop iteration.
pub const ROOT_SAVE_RETRY: Duration = Duration::from_secs(15);

impl RootMemory {
    pub fn record(&mut self, root: &std::path::Path) {
        if let Some(key) = self.record_with(root, std::time::Instant::now, live_workspace_label) {
            herdr_work_code::state::save_root(&key, &self.root);
        }
        publish_pending(self);
    }

    /// One loop iteration's worth of `record`, minus the disk write. The
    /// retry clock restarts when an attempt that really ran FINISHES — a
    /// lookup that took its whole socket timeout must not leave the next retry
    /// immediately due — and never on iterations the throttle skipped.
    pub fn record_with(
        &mut self,
        root: &std::path::Path,
        clock: impl Fn() -> std::time::Instant,
        live_label: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        let before = self.last_attempt;
        let key = self.save_key(root, clock(), live_label);
        if self.pending && self.last_attempt != before {
            self.last_attempt = Some(clock());
        }
        key
    }

    /// Quitting: one last attempt, ignoring the retry clock. If the label is
    /// still unknown the choice is NOT saved under a guessed key (that is the
    /// overwrite this type exists to prevent); it is reported instead.
    pub fn flush(&mut self) {
        if !self.pending {
            return;
        }
        self.last_attempt = None;
        let root = self.root.clone();
        if let Some(key) = self.save_key(&root, std::time::Instant::now(), live_workspace_label) {
            herdr_work_code::state::save_root(&key, &self.root);
        } else {
            eprintln!(
                "herdr-work-code: could not resolve this workspace; {} was not remembered",
                self.root.display()
            );
        }
    }

    /// The key to persist the current root under, or `None` when there is
    /// nothing to save yet. A failed label lookup DEFERS the save: falling
    /// back to the startup label would write this workspace's folder over the
    /// project whose label it inherited — the very bug this type prevents.
    /// The in-process root still follows immediately; only persistence waits.
    pub fn save_key(
        &mut self,
        root: &std::path::Path,
        now: std::time::Instant,
        live_label: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        if root != self.root {
            self.root = root.to_path_buf();
            self.pending = true;
            self.last_attempt = None;
        }
        if !self.pending
            || self
                .last_attempt
                .is_some_and(|at| now.duration_since(at) < ROOT_SAVE_RETRY)
        {
            return None;
        }
        self.last_attempt = Some(now);
        let label = match live_label() {
            Some(label) => label,
            // A process that never had a label keys roots by folder alone, so
            // there is no other workspace's entry to collide with.
            None if self.startup_label.is_empty() => String::new(),
            None => return None,
        };
        self.pending = false;
        Some(remembered_root_key(&label, &self.spawn_cwd))
    }
}

/// The pending save, mirrored where the apps' own close paths can reach it:
/// hiding the sidebar or Ctrl+Q closes the pane from inside the app, which
/// kills the process before the outer loop's `flush` could run.
#[doc(hidden)]
pub static PENDING_ROOT: std::sync::Mutex<Option<RootMemory>> = std::sync::Mutex::new(None);

pub fn publish_pending(roots: &RootMemory) {
    if let Ok(mut pending) = PENDING_ROOT.lock() {
        *pending = roots.pending.then(|| roots.clone());
    }
}

/// Called by the apps right before they close their own pane.
pub fn flush_pending_root() {
    let pending = PENDING_ROOT.lock().ok().and_then(|mut slot| slot.take());
    if let Some(mut roots) = pending {
        roots.flush();
    }
}

/// The workspace's CURRENT label, or `None` when it cannot be determined
/// (socket error, workspace not listed). Outside herdr there is no label.
pub fn live_workspace_label() -> Option<String> {
    let Ok(ws_id) = std::env::var("HERDR_WORKSPACE_ID") else {
        return Some(String::new());
    };
    let json = herdr_work_code::ipc::call_text("workspace.list", serde_json::json!({})).ok()?;
    let label = herdr_work_code::launch::workspace_label(&json, &ws_id);
    (!label.is_empty()).then_some(label)
}

/// The label of the space this pane lives in, or "" when it can't be
/// resolved — the caller then falls back to the pane's cwd.
pub fn workspace_label() -> String {
    let Ok(ws_id) = std::env::var("HERDR_WORKSPACE_ID") else {
        return String::new();
    };
    herdr_work_code::ipc::call_text("workspace.list", serde_json::json!({}))
        .map(|json| herdr_work_code::launch::workspace_label(&json, &ws_id))
        .unwrap_or_default()
}

/// Roots are project state, not workspace state: one Herdr workspace may host
/// unrelated project tabs, while tab ids change across server restarts.
pub fn remembered_root_key(workspace_label: &str, spawn_cwd: &std::path::Path) -> String {
    let mut cwd = spawn_cwd.display().to_string().replace('\\', "/");
    if cfg!(windows) {
        cwd.make_ascii_lowercase();
    }
    format!("{workspace_label}::{cwd}")
}

/// The directory the tree is built from: the root this tab remembers,
/// else the cwd the pane was spawned with.
///
/// A remembered root that has since been deleted is ignored rather than
/// yielding an empty tree. The guarded legacy lookup migrates v0.10's
/// workspace-keyed entry only when it contains this tab's spawn cwd.
pub fn resolve_root(
    root_key: &str,
    legacy_workspace_label: &str,
    spawn_cwd: &std::path::Path,
) -> std::io::Result<std::path::PathBuf> {
    let root = if let Some(root) = herdr_work_code::state::load_root(root_key)
        && root.is_dir()
    {
        root
    } else if herdr_work_code::state::product_profile().is_none()
        && let Some(root) = herdr_work_code::state::load_root(legacy_workspace_label)
        && root.is_dir()
        && spawn_cwd.starts_with(&root)
    {
        // v0.10 keyed roots by workspace label. Migrate that choice only
        // when it contains this tab's live spawn cwd; sibling project tabs
        // must not all inherit the same old entry.
        root
    } else {
        spawn_cwd.to_path_buf()
    };
    herdr_work_code::state::save_root(root_key, &root);
    Ok(root)
}
