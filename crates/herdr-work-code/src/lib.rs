//! HerdrWork file, Git, search, rendering and editor implementation.
//! Independent plugin lifecycle modules are excluded from the default build.
//! `reference-ui` exists only for explicit source UI validation.

extern crate self as herdr_work_code;

pub mod actions;
pub mod activity;
pub mod ansi;
pub mod branch_ui;
pub mod browser;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod context_menu;
pub mod diffview;
pub mod dock;
pub mod editor;
#[cfg(any(feature = "reference-ui", test))]
pub mod ensure;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod fontsetup;
pub mod git;
pub mod git_graph;
pub mod gitdeco;
pub mod history;
pub mod history_view;
pub mod icons;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod ipc;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod launch;
pub mod notice;
pub mod preview;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod rundir;
pub mod scm;
pub mod scm_input;
pub mod scm_tree;
pub mod scm_view;
pub mod scm_widgets;
pub mod search;
pub mod search_widgets;
pub mod settings_view;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod snooze;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod state;
pub mod suggest;
pub mod syntax;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod takeover;
pub mod tree;
pub mod ui;
#[cfg(any(feature = "reference-ui", test))]
pub mod updates;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod viewer;
pub mod wrap;

pub mod appearance;

pub mod file_view;

pub mod view_state;
pub mod vscode;

#[cfg(feature = "sidebar-ui")]
pub mod explorer_app;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod pane_close;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod pane_lock;
#[cfg(feature = "sidebar-ui")]
pub mod scm_app;
#[cfg(feature = "sidebar-ui")]
mod sidebar_chrome;
#[cfg(feature = "sidebar-ui")]
pub mod sidebar_component;

#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub mod root_memory;
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub use root_memory::flush_pending_root;
