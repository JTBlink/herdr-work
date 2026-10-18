//! One terminal owner, with background jobs and upstream-backed views.
use anyhow::{Context as _, Result};
use crossterm::event::Event;
use std::path::PathBuf;
use workdesk_core::{Context, Execution, ExecutionId, Review, Task, TaskId};
use workdesk_engine::{CodeSnapshot, DiffView, JobQueue, Workdesk};

pub(crate) enum Data {
    Tasks(Vec<Task>),
    Code(CodeSnapshot),
    Diff(DiffView, Review),
    CommitReady(workdesk_engine::CommitPlan),
    Delivery(Review, String),
    Text(String),
    Review(Review),
    Bound(Execution),
    Business(String),
}
pub(crate) enum Message {
    Input(Event),
    Result(u64, Result<Data>),
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Tab {
    Tasks,
    Files,
    Scm,
    Review,
}
pub(crate) enum Prompt {
    Comment {
        review: workdesk_core::ReviewId,
        anchor: workdesk_core::CommentAnchor,
    },
    Commit(workdesk_engine::CommitPlan),
    NewTask,
    Stop(TaskId),
}
pub(crate) struct App {
    pub(crate) desk: Workdesk,
    pub(crate) context: Context,
    pub(crate) tasks: Vec<Task>,
    pub(crate) task: usize,
    pub(crate) tab: Tab,
    pub(crate) selection: usize,
    pub(crate) root: PathBuf,
    pub(crate) code: Option<CodeSnapshot>,
    pub(crate) diff: Option<DiffView>,
    pub(crate) review: Option<Review>,
    pub(crate) text: String,
    pub(crate) status: String,
    pub(crate) expanded: Vec<PathBuf>,
    pub(crate) generation: u64,
    pub(crate) prompt: Option<Prompt>,
    pub(crate) input: String,
    pub(crate) jobs: JobQueue<Data>,
}

impl App {
    pub(crate) fn job(
        &self,
        job: impl FnOnce(Workdesk) -> Result<Data> + Send + 'static,
    ) -> Result<()> {
        let desk = self.desk.clone();
        self.jobs.submit(self.context.epoch, move || job(desk))
    }
    pub(crate) fn load_tasks(&self) -> Result<()> {
        self.job(|d| Ok(Data::Tasks(d.tasks()?)))
    }
    pub(crate) fn refresh(&mut self) -> Result<()> {
        self.generation += 1;
        let generation = self.generation;
        let root = self.root.clone();
        let expanded = self.expanded.clone();
        self.job(move |d| Ok(Data::Code(d.code_snapshot(&root, &expanded, generation)?)))
    }
    pub(crate) fn execution(&self) -> Execution {
        let root = self.diff.as_ref().map(|d| &d.root).unwrap_or(&self.root);
        self.context
            .execution
            .clone()
            .filter(|e| e.worktree == *root)
            .unwrap_or_else(|| Execution {
                id: ExecutionId(format!("browse:{}", root.display())),
                task: TaskId("browse".into()),
                attempt: None,
                worktree: root.clone(),
                worktree_identity: Workdesk::worktree_identity(root).unwrap_or_default(),
                origin_terminal: None,
                origin_socket: None,
                target: None,
            })
    }
    pub(crate) fn apply(&mut self, epoch: u64, data: Result<Data>) {
        match data {
            Err(e) => self.status = format!("{e:#}"),
            Ok(Data::Business(text)) => {
                self.status = text;
                let _ = self.refresh();
                let _ = self.load_tasks();
            }
            Ok(Data::Tasks(tasks)) => {
                let selected = self.tasks.get(self.task).map(|t| t.id.clone());
                self.task = selected
                    .and_then(|id| tasks.iter().position(|t| t.id == id))
                    .unwrap_or(self.task.min(tasks.len().saturating_sub(1)));
                self.tasks = tasks;
            }
            Ok(_) if epoch != self.context.epoch => {}
            Ok(Data::Code(code)) if code.generation == self.generation => {
                self.code = Some(code);
            }
            Ok(Data::Code(_)) => {}
            Ok(Data::CommitReady(plan)) => {
                self.prompt = Some(Prompt::Commit(plan));
                self.input.clear();
            }
            Ok(Data::Delivery(review, status)) => {
                self.status = status;
                self.review = Some(review);
            }
            Ok(Data::Diff(diff, review)) => {
                self.status = format!("{} saved comment(s)", review.comments.len());
                self.review = Some(review);
                self.diff = Some(diff);
                self.tab = Tab::Review;
                self.selection = 0;
            }
            Ok(Data::Text(text)) => self.text = text,
            Ok(Data::Review(review)) => {
                self.status = format!("{} saved comment(s)", review.comments.len());
                self.review = Some(review);
            }
            Ok(Data::Bound(execution)) => {
                self.context.select(execution);
                self.status = "Agent binding verified".into();
                let _ = self.desk.save_context(&self.context);
                if let Some(diff) = &self.diff {
                    let _ =
                        self.open_diff(diff.root.clone(), diff.file.clone(), diff.scope.clone());
                }
            }
        }
    }
    pub(crate) fn choose_execution(&mut self) -> Result<()> {
        let task = self.tasks.get(self.task).context("No task selected")?;
        let e = task
            .executions
            .first()
            .cloned()
            .context("Task has no execution; delegate it first")?;
        self.root = e.worktree.clone();
        self.context.select(e);
        self.desk.save_context(&self.context)?;
        self.diff = None;
        self.review = None;
        self.selection = 0;
        self.refresh()
    }
    pub(crate) fn open_diff(&self, root: PathBuf, path: String, scope: String) -> Result<()> {
        let mut execution = self.execution();
        if execution.worktree != root {
            execution = Execution {
                id: ExecutionId(format!("browse:{}", root.display())),
                task: TaskId("browse".into()),
                attempt: None,
                worktree_identity: Workdesk::worktree_identity(&root)?,
                worktree: root.clone(),
                origin_terminal: None,
                origin_socket: None,
                target: None,
            };
        }
        self.job(move |d| {
            let diff = d.diff(&root, &path, &scope, None)?;
            let execution = d.bound_execution(&execution)?;
            let review = d.open_review(&execution, &scope, diff.base.clone())?;
            Ok(Data::Diff(diff, review))
        })
    }
}
