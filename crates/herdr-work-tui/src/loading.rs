//! Independent read states keep task backend failures separate from Git browsing.
#[derive(Default)]
pub(crate) enum LoadState {
    #[default]
    Loading,
    Ready,
    Failed(String),
}
