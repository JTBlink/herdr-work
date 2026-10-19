//! HerdrWork orchestration. All terminal-free business commands enter here.
mod activation;
mod code;
mod editing;
mod files;
mod host;
mod preferences;
mod profile;
mod review;
mod scm;
mod search;
mod storage;
mod tasks;
mod ui_state;
mod work_git;

pub use activation::{ActivationRequest, InitialView, UiRegistration};
pub use code::{CodeSnapshot, CommitPlan, DiffView, FileRow, GitChange, RepositoryStatus};
pub use editing::{EditorExternalObservation, EditorExternalRequest, EditorSession};
pub use files::FilePlan;
pub use herdr_work_core as model;
pub use preferences::UiPreferences;
pub use review::SendOutcome;
pub use scm::{DrawerView, GitPlan};
pub use search::{QuickIndex, ReplacePlan, ReplaceRequest, SearchRow, SearchView};
pub use storage::fingerprint;
pub use ui_state::{DocumentState, DrawerRoot, DrawerState, WorkspaceUiState};
pub use work_git::{CommitDraft, WorkGitAction, WorkGitOutcome, WorkGitPlan};

use anyhow::Result;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub struct HerdrWork(Arc<Profile>);
struct Profile {
    root: PathBuf,
    backend: PathBuf,
    _lock: File,
}
impl Drop for Profile {
    fn drop(&mut self) {
        // Closing our fd alone can leave a Unix flock alive in a child that
        // forked before exec. Release when the final shared owner exits.
        let _ = fs2::FileExt::unlock(&self._lock);
    }
}

impl HerdrWork {
    pub fn worktree_identity(root: &Path) -> Result<String> {
        let root = root.canonicalize()?;
        let meta = root.metadata()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(format!("{}:{}:{}", root.display(), meta.dev(), meta.ino()))
        }
        #[cfg(not(unix))]
        {
            Ok(format!("{}:{:?}", root.display(), meta.created()?))
        }
    }
    pub fn validate_execution(execution: &herdr_work_core::Execution) -> Result<()> {
        anyhow::ensure!(
            !execution.worktree_identity.is_empty()
                && Self::worktree_identity(&execution.worktree)? == execution.worktree_identity,
            "Worktree instance changed; select the current execution again"
        );
        Ok(())
    }
    pub fn save_context(&self, context: &herdr_work_core::Context) -> Result<()> {
        storage::save(&self.root().join("context.json"), context)
    }
    pub fn load_context(&self) -> Result<herdr_work_core::Context> {
        let path = self.root().join("context.json");
        if !path.exists() {
            return Ok(herdr_work_core::Context::default());
        }
        let mut context: herdr_work_core::Context = storage::load(&path)?;
        if context
            .execution
            .as_ref()
            .is_some_and(|e| Self::validate_execution(e).is_err())
        {
            context.execution = None;
        }
        Ok(context)
    }

    pub fn open(root: &Path, backend: Option<PathBuf>) -> Result<Self> {
        let absolute = std::path::absolute(root)?;
        std::fs::create_dir_all(&absolute)?;
        let root = absolute.canonicalize()?;
        let lock = profile::profile_lock(&root)?;
        let backend = backend.unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap_or_default()
                .with_file_name(if cfg!(windows) {
                    "herdr-work-projects.exe"
                } else {
                    "herdr-work-projects"
                })
        });
        let desk = Self(Arc::new(Profile {
            root,
            backend,
            _lock: lock,
        }));
        desk.recover_reviews()?;
        Ok(desk)
    }
    pub fn root(&self) -> &Path {
        &self.0.root
    }
    pub fn backend(&self) -> &Path {
        &self.0.backend
    }
    pub fn projects_home(&self) -> PathBuf {
        self.root().join("projects")
    }
    pub fn default_root() -> PathBuf {
        profile::default_root()
    }
    pub fn profile_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
        profile::resolve(explicit)
    }
}

type QueuedJob<T> = (u64, Box<dyn FnOnce() -> Result<T> + Send>);

/// Runs jobs away from input/render. Results retain their originating context.
pub struct JobQueue<T: Send + 'static> {
    tx: std::sync::mpsc::SyncSender<QueuedJob<T>>,
    pub results: std::sync::mpsc::Receiver<(u64, Result<T>)>,
}
impl<T: Send + 'static> Default for JobQueue<T> {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<QueuedJob<T>>(32);
        let (out, results) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            while let Ok((epoch, job)) = rx.recv() {
                if out.send((epoch, job())).is_err() {
                    break;
                }
            }
        });
        Self { tx, results }
    }
}
impl<T: Send + 'static> JobQueue<T> {
    /// FIFO shutdown barrier. Only use after the input loop has ended: unlike
    /// submit, this waits for capacity so final persistence cannot be dropped.
    pub fn submit_on_shutdown(
        &self,
        epoch: u64,
        job: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<()> {
        self.tx
            .send((epoch, Box::new(job)))
            .map_err(|_| anyhow::anyhow!("Worker stopped before final persistence"))
    }
    pub fn submit(
        &self,
        epoch: u64,
        job: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<()> {
        self.tx
            .try_send((epoch, Box::new(job)))
            .map_err(|_| anyhow::anyhow!("Worker queue full or stopped"))
    }
}
