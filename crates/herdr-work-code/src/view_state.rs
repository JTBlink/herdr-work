//! Pure source view-state records; storage locations belong to each host.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TreeState {
    pub expanded: Vec<PathBuf>,
    pub selected: Option<PathBuf>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScmState {
    pub drawers: Vec<String>,
    pub active_root: Option<String>,
    pub selected: Option<String>,
    pub history_target: Option<String>,
    pub scroll: usize,
    pub drafts: std::collections::BTreeMap<String, String>,
    /// Draft roots this pane previously observed and has since emptied.
    /// Kept out of the JSON shape; it only scopes merge-on-write removals.
    #[serde(skip)]
    pub cleared_drafts: std::collections::BTreeSet<String>,
    /// Tree-view folders the user collapsed, as JSON `(repo root, staged,
    /// path)` triples. The tree/list MODE itself is global (`scm_tree_view`).
    pub collapsed_dirs: Vec<String>,
}
