//! HerdrWork file, Git, search, rendering and editor implementation.
//! Independent plugin lifecycle modules are excluded from the default build.
//! `reference-ui` exists only for explicit source UI validation.

pub mod actions;
pub mod ansi;
pub mod branch_ui;
pub mod diffview;
pub mod editor;
#[cfg(any(feature = "reference-ui", test))]
pub mod ensure;
#[cfg(any(feature = "reference-ui", test))]
pub mod fontsetup;
pub mod git;
pub mod gitdeco;
pub mod icons;
#[cfg(any(feature = "reference-ui", test))]
pub mod ipc;
#[cfg(any(feature = "reference-ui", test))]
pub mod launch;
#[cfg(any(feature = "reference-ui", test))]
pub mod rundir;
pub mod scm;
pub mod search;
#[cfg(any(feature = "reference-ui", test))]
pub mod snooze;
#[cfg(any(feature = "reference-ui", test))]
pub mod state;
#[cfg(any(feature = "reference-ui", test))]
pub mod suggest;
pub mod syntax;
#[cfg(any(feature = "reference-ui", test))]
pub mod takeover;
pub mod tree;
pub mod ui;
#[cfg(any(feature = "reference-ui", test))]
pub mod updates;
#[cfg(any(feature = "reference-ui", test))]
pub mod viewer;
pub mod wrap;

pub mod appearance;

pub mod file_view;
