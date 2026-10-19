//! Stable HerdrWork contracts. This crate owns no terminal, filesystem or host connection.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

macro_rules! id {
    ($name:ident) => {
        #[derive(
            Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub String);
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
id!(TaskId);
id!(ExecutionId);
id!(ReviewId);
id!(BatchId);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTarget {
    pub socket: PathBuf,
    pub session: String,
    pub pane: String,
    pub terminal: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Execution {
    pub id: ExecutionId,
    pub task: TaskId,
    pub attempt: Option<String>,
    pub worktree: PathBuf,
    #[serde(default)]
    pub worktree_identity: String,
    #[serde(default)]
    pub origin_terminal: Option<String>,
    #[serde(default)]
    pub origin_socket: Option<PathBuf>,
    pub target: Option<AgentTarget>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    /// The original Projects stage, not a second task state machine.
    pub stage: String,
    pub executions: Vec<Execution>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Context {
    pub epoch: u64,
    pub execution: Option<Execution>,
}
impl Context {
    pub fn select(&mut self, execution: Execution) {
        self.epoch = self.epoch.wrapping_add(1);
        self.execution = Some(execution);
    }
    pub fn accepts(&self, epoch: u64, worktree: &std::path::Path) -> bool {
        self.epoch == epoch
            && self
                .execution
                .as_ref()
                .is_some_and(|e| e.worktree == worktree)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Old,
    New,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub version: u64,
    pub file: String,
    pub side: Side,
    pub start: u32,
    pub end: u32,
    pub snippet: String,
    pub text: String,
    pub fingerprint: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Delivery {
    Prepared,
    Sending,
    Delivered,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewBatch {
    pub id: BatchId,
    pub target: AgentTarget,
    pub comments: Vec<(u64, u64)>,
    pub text: String,
    pub state: Delivery,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub schema_version: u32,
    pub id: ReviewId,
    pub execution: Execution,
    pub base: Option<String>,
    pub scope: String,
    pub comments: Vec<Comment>,
    pub batches: Vec<ReviewBatch>,
    pub next_comment: u64,
}

/// An anchor frozen from the diff the user actually read.
#[derive(Clone, Debug)]
pub struct CommentAnchor {
    pub file: String,
    pub side: Side,
    pub start: u32,
    pub end: u32,
    pub fingerprint: String,
}
