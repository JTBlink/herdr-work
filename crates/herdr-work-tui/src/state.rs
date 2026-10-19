//! One terminal owner, with background jobs and upstream-backed views.
use crate::loading::LoadState;
use anyhow::{Context as _, Result};
use crossterm::event::Event;
use herdr_work_core::{Context, Execution, ExecutionId, Review, Task, TaskId};
use herdr_work_engine::{CodeSnapshot, DiffView, HerdrWork, JobQueue};
use std::path::PathBuf;

pub(crate) enum Data {
    Tasks(Result<Vec<Task>>),
    Code(u64, Result<CodeSnapshot>),
    Diff(DiffView, Review),
    CommitReady(herdr_work_engine::CommitPlan),
    Delivery(Review, String),
    Preview(u64, PathBuf, Result<herdr_work_code::preview::Doc>),
    Review(Review),
    Bound(Execution),
    Business(String),
    Editor(PathBuf, Result<herdr_work_engine::EditorSession>),
    EditorObserved(
        u64,
        u64,
        Result<herdr_work_engine::EditorExternalObservation>,
    ),
    EditorSaved(
        u64,
        u64,
        Result<(
            herdr_work_engine::EditorSession,
            herdr_work_code::editor::SaveOutcome,
        )>,
    ),
    EditorReloaded(u64, u64, String, Result<herdr_work_engine::EditorSession>),
    Drawer(u64, Result<herdr_work_engine::DrawerView>),
    Show(u64, Result<String>),
    QuickIndex(u64, Result<herdr_work_engine::QuickIndex>),
    Search(u64, Result<herdr_work_engine::SearchView>),
    FileReady(herdr_work_engine::FilePlan),
    Folder(Option<PathBuf>),
    GitReady(herdr_work_engine::GitPlan),
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
    Search,
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Focus {
    Sidebar,
    Content,
}
pub(crate) enum Prompt {
    Comment {
        review: herdr_work_core::ReviewId,
        anchor: herdr_work_core::CommentAnchor,
    },
    Commit(herdr_work_engine::CommitPlan),
    NewTask,
    Stop(TaskId),
    Search(bool),
    SearchInclude,
    SearchExclude,
    File(herdr_work_engine::FilePlan),
    Folder,
    Git(herdr_work_engine::GitPlan),
}
pub(crate) struct App {
    pub(crate) desk: HerdrWork,
    pub(crate) context: Context,
    pub(crate) tasks: Vec<Task>,
    pub(crate) task: usize,
    pub(crate) tab: Tab,
    pub(crate) selection: usize,
    pub(crate) scm_selection: usize,
    pub(crate) focus: Focus,
    pub(crate) preview_doc: Option<herdr_work_code::preview::Doc>,
    pub(crate) preview_request: u64,
    pub(crate) preview_width: u16,
    pub(crate) preview_pinned: bool,
    pub(crate) preview_documents: Vec<crate::preview::PreviewDocument>,
    pub(crate) preview_last_click: Option<(PathBuf, std::time::Instant)>,
    pub(crate) preview_copier: Option<herdr_work_code::preview::ReleaseCopier>,
    pub(crate) preview_scroll: u16,
    pub(crate) preview_path: Option<PathBuf>,
    pub(crate) root: PathBuf,
    pub(crate) code: Option<CodeSnapshot>,
    pub(crate) diff: Option<DiffView>,
    pub(crate) review: Option<Review>,
    pub(crate) text: String,
    pub(crate) status: String,
    pub(crate) tasks_state: LoadState,
    pub(crate) code_state: LoadState,
    pub(crate) expanded: Vec<PathBuf>,
    pub(crate) generation: u64,
    pub(crate) prompt: Option<Prompt>,
    pub(crate) input: String,
    pub(crate) jobs: JobQueue<Data>,
    pub(crate) preferences: herdr_work_engine::UiPreferences,
    pub(crate) overlay: Option<crate::menus::Overlay>,
    pub(crate) editor: Option<herdr_work_engine::EditorSession>,
    pub(crate) edit_request: Option<PathBuf>,
    pub(crate) editor_request: u64,
    pub(crate) editor_poll: Option<(u64, u64)>,
    pub(crate) editor_write: Option<(u64, u64)>,
    pub(crate) editor_leave_after_save: bool,
    pub(crate) scm_cursor: usize,
    pub(crate) active_drawer: Option<herdr_work_code::scm::Drawer>,
    pub(crate) drawer_open: [bool; 8],
    pub(crate) drawers:
        std::collections::HashMap<herdr_work_code::scm::Drawer, herdr_work_engine::DrawerView>,
    pub(crate) drawer_request: u64,
    pub(crate) drawer_state: LoadState,
    pub(crate) drawer_scroll: u16,
    pub(crate) drawer_show: Option<String>,
    pub(crate) search_runtime: crate::search::SearchRuntime,
    pub(crate) preview_target: Option<(PathBuf, usize)>,
    pub(crate) edit_line: Option<usize>,
    pub(crate) search_query: String,
    pub(crate) search_quick: bool,
    pub(crate) search_options: herdr_work_code::search::SearchOptions,
    pub(crate) search_include: String,
    pub(crate) search_exclude: String,
    pub(crate) search_rows: Vec<herdr_work_engine::SearchRow>,
    pub(crate) search_state: LoadState,
    pub(crate) search_request: u64,
    pub(crate) menu_path: Option<PathBuf>,
    pub(crate) menu_reference: Option<(herdr_work_engine::DrawerView, usize)>,
}

impl App {
    pub(crate) fn new(desk: HerdrWork, root: PathBuf, context: Context) -> Result<Self> {
        let root = root.canonicalize()?;
        let preferences = desk.ui_preferences()?;
        Ok(Self {
            desk,
            context,
            tasks: vec![],
            task: 0,
            tab: Tab::Files,
            selection: 0,
            scm_selection: 0,
            focus: Focus::Sidebar,
            preview_doc: None,
            preview_request: 0,
            preview_width: 80,
            preview_pinned: false,
            preview_documents: vec![],
            preview_last_click: None,
            preview_copier: None,
            preview_scroll: 0,
            preview_path: None,
            root,
            code: None,
            diff: None,
            review: None,
            text: String::new(),
            status: String::new(),
            tasks_state: LoadState::Loading,
            code_state: LoadState::Loading,
            expanded: vec![],
            generation: 0,
            prompt: None,
            input: String::new(),
            jobs: JobQueue::default(),
            preferences,
            overlay: None,
            editor: None,
            edit_request: None,
            editor_request: 0,
            editor_poll: None,
            editor_write: None,
            editor_leave_after_save: false,
            scm_cursor: 0,
            active_drawer: None,
            drawer_open: [false; 8],
            drawers: Default::default(),
            drawer_request: 0,
            drawer_state: LoadState::Ready,
            drawer_scroll: 0,
            drawer_show: None,
            search_runtime: Default::default(),
            preview_target: None,
            edit_line: None,
            search_query: String::new(),
            search_quick: false,
            search_options: Default::default(),
            search_include: String::new(),
            search_exclude: String::new(),
            search_rows: vec![],
            search_state: LoadState::Ready,
            search_request: 0,
            menu_path: None,
            menu_reference: None,
        })
    }

    pub(crate) fn job(
        &self,
        job: impl FnOnce(HerdrWork) -> Result<Data> + Send + 'static,
    ) -> Result<()> {
        let desk = self.desk.clone();
        self.jobs.submit(self.context.epoch, move || job(desk))
    }
    pub(crate) fn load_tasks(&mut self) -> Result<()> {
        self.job(|d| Ok(Data::Tasks(d.tasks())))?;
        self.tasks_state = LoadState::Loading;
        Ok(())
    }
    pub(crate) fn refresh(&mut self) -> Result<()> {
        self.generation += 1;
        let generation = self.generation;
        let root = self.root.clone();
        let expanded = self.expanded.clone();
        let show_hidden = self.preferences.show_hidden;
        self.job(move |d| {
            Ok(Data::Code(
                generation,
                d.code_snapshot_with_hidden(&root, &expanded, generation, show_hidden),
            ))
        })?;
        self.code_state = LoadState::Loading;
        Ok(())
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
                worktree_identity: HerdrWork::worktree_identity(root).unwrap_or_default(),
                origin_terminal: None,
                origin_socket: None,
                target: None,
            })
    }
    pub(crate) fn apply(&mut self, epoch: u64, data: Result<Data>) {
        // Editor results belong to their guarded session, which can outlive an
        // unrelated Agent binding epoch. Their handlers reject closed/new sessions.
        if epoch != self.context.epoch
            && !matches!(
                &data,
                Ok(Data::Tasks(_)
                    | Data::EditorObserved(..)
                    | Data::EditorSaved(..)
                    | Data::EditorReloaded(..))
            )
        {
            return;
        }
        match data {
            Err(e) => self.status = format!("{e:#}"),
            Ok(Data::Business(text)) => {
                self.status = text;
                let _ = self.refresh();
                let _ = self.load_tasks();
                if let Some(kind) = self.active_drawer {
                    let _ = self.load_drawer(kind);
                }
            }
            Ok(Data::Tasks(Err(error))) => {
                self.tasks_state = LoadState::Failed(format!("{error:#}"));
            }
            Ok(Data::Tasks(Ok(tasks))) => {
                self.tasks_state = LoadState::Ready;
                let selected = self.tasks.get(self.task).map(|t| t.id.clone());
                self.task = selected
                    .and_then(|id| tasks.iter().position(|t| t.id == id))
                    .unwrap_or(self.task.min(tasks.len().saturating_sub(1)));
                self.tasks = tasks;
            }
            Ok(Data::EditorObserved(session, request, result)) => {
                self.editor_observed(session, request, result)
            }
            Ok(Data::EditorSaved(session, request, result)) => {
                self.editor_saved(session, request, result)
            }
            Ok(Data::EditorReloaded(session, request, text, result)) => {
                self.editor_reloaded(session, request, text, result)
            }
            Ok(_) if epoch != self.context.epoch => {}
            Ok(Data::Code(generation, _)) if generation != self.generation => {}
            Ok(Data::Code(_, Err(error))) => {
                self.code_state = LoadState::Failed(format!("{error:#}"));
            }
            Ok(Data::Code(_, Ok(code))) => {
                let was_change = self
                    .scm_rows()
                    .get(self.scm_cursor)
                    .is_some_and(|r| r.0.is_some());
                self.code_state = LoadState::Ready;
                let old = self.code.as_ref();
                let position = match self.tab {
                    Tab::Files => old
                        .and_then(|c| c.files.get(self.selection))
                        .and_then(|row| code.files.iter().position(|r| r.path == row.path)),
                    Tab::Scm => old
                        .and_then(|c| c.changes.get(self.selection))
                        .and_then(|row| {
                            code.changes.iter().position(|r| {
                                r.root == row.root && r.path == row.path && r.staged == row.staged
                            })
                        }),
                    _ => Some(self.selection),
                };
                let count = match self.tab {
                    Tab::Files => code.files.len(),
                    Tab::Scm => code.changes.len(),
                    _ => usize::MAX,
                };
                self.selection = position
                    .unwrap_or(self.selection)
                    .min(count.saturating_sub(1));
                self.code = Some(code);
                if was_change {
                    self.scm_cursor = self
                        .scm_rows()
                        .iter()
                        .position(|r| r.0 == Some(self.selection))
                        .unwrap_or(0);
                }
            }
            Ok(Data::CommitReady(plan)) => {
                self.prompt = Some(Prompt::Commit(plan));
                self.input.clear();
            }
            Ok(Data::Delivery(review, status)) => {
                self.status = status;
                self.review = Some(review);
            }
            Ok(Data::Diff(diff, review)) => {
                self.active_drawer = None;
                self.status = format!("{} saved comment(s)", review.comments.len());
                self.review = Some(review);
                self.diff = Some(diff);
                if self.tab == Tab::Scm {
                    self.focus = Focus::Sidebar;
                    self.selection = self.scm_selection;
                } else {
                    self.tab = Tab::Review;
                    self.focus = Focus::Content;
                    self.selection = 0;
                }
            }
            Ok(Data::Preview(request, path, result))
                if request == self.preview_request && self.preview_path.as_ref() == Some(&path) =>
            {
                match result {
                    Ok(doc) => self.apply_preview(path, doc, false),
                    Err(error) => {
                        self.preview_doc = None;
                        self.text = format!("无法预览文件：{error:#}");
                    }
                }
            }
            Ok(Data::Preview(..)) => {}
            Ok(Data::GitReady(plan)) => {
                self.input.clear();
                self.prompt = Some(Prompt::Git(plan));
            }
            Ok(Data::FileReady(plan)) => {
                self.input = if plan.action == herdr_work_code::actions::MenuAction::Rename {
                    plan.target
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                } else {
                    String::new()
                };
                self.prompt = Some(Prompt::File(plan));
            }
            Ok(Data::Folder(Some(path))) => {
                if let Err(error) = self.change_folder(path) {
                    self.status = error.to_string();
                }
            }
            Ok(Data::Folder(None)) => {
                self.status = "已取消打开文件夹".into();
            }
            Ok(Data::QuickIndex(request, result)) => {
                self.search_finished(request);
                self.receive_quick_index(result);
            }
            Ok(Data::Search(request, result)) => {
                self.search_finished(request);
                if request != self.search_request {
                    return;
                }
                match result {
                    Ok(view) => {
                        self.search_rows = view.rows;
                        self.search_state = LoadState::Ready;
                        self.selection = 0;
                        if view.truncated {
                            self.status = "结果已截断，请缩小搜索范围".into();
                        }
                    }
                    Err(error) => {
                        self.search_state = LoadState::Failed(error.to_string());
                        self.search_rows.clear();
                    }
                }
            }
            Ok(Data::Drawer(request, result)) if request == self.drawer_request => match result {
                Ok(view) => {
                    self.drawer_state = LoadState::Ready;
                    self.drawers.insert(view.kind, view);
                }
                Err(error) => self.drawer_state = LoadState::Failed(error.to_string()),
            },
            Ok(Data::Show(request, result)) if request == self.drawer_request => {
                self.drawer_show =
                    Some(result.unwrap_or_else(|e| format!("无法显示 Git 对象：{e}")));
            }
            Ok(Data::Drawer(..) | Data::Show(..)) => {}
            Ok(Data::Editor(path, result)) if self.edit_request.as_ref() == Some(&path) => {
                self.edit_request = None;
                match result {
                    Ok(mut editor) if self.overlay.is_none() && self.prompt.is_none() => {
                        if let Some(line) = self.edit_line.take() {
                            editor.editor.goto_line(line);
                        }
                        self.preview_path = Some(editor.editor.path().into());
                        self.editor = Some(editor);
                        self.focus = Focus::Content;
                        self.status = "文本编辑：Ctrl+S 保存 · Esc 返回".into();
                    }
                    Err(error) => self.status = error.to_string(),
                    _ => {}
                }
            }
            Ok(Data::Editor(..)) => {}
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
        self.clear_previews();
        self.search_runtime = Default::default();
        self.preview_target = None;
        self.search_request += 1;
        self.search_rows.clear();
        self.active_drawer = None;
        self.drawers.clear();
        self.drawer_open = [false; 8];
        self.drawer_request += 1;
        self.review = None;
        self.code = None;
        self.text.clear();
        self.preview_path = None;
        self.preview_scroll = 0;
        self.expanded.clear();
        self.selection = 0;
        self.tab = Tab::Files;
        self.focus = Focus::Sidebar;
        self.refresh()
    }
    pub(crate) fn change_folder(&mut self, root: PathBuf) -> Result<()> {
        let root = root.canonicalize()?;
        anyhow::ensure!(root.is_dir(), "请选择目录");
        anyhow::ensure!(self.editor.is_none(), "请先结束文本编辑");
        let context = Context {
            epoch: self.context.epoch.wrapping_add(1),
            execution: None,
        };
        self.desk.save_context(&context)?;
        self.context = context;
        self.root = root;
        self.code = None;
        self.diff = None;
        self.review = None;
        self.preview_path = None;
        self.edit_request = None;
        self.text.clear();
        self.expanded.clear();
        self.clear_previews();
        self.search_runtime = Default::default();
        self.preview_target = None;
        self.search_request += 1;
        self.search_rows.clear();
        self.drawer_request += 1;
        self.drawers.clear();
        self.active_drawer = None;
        self.drawer_open = [false; 8];
        self.overlay = None;
        self.switch_tab(Tab::Files);
        self.refresh()
    }
    pub(crate) fn open_diff(&self, root: PathBuf, path: String, scope: String) -> Result<()> {
        let mut execution = self.execution();
        if execution.worktree != root {
            execution = Execution {
                id: ExecutionId(format!("browse:{}", root.display())),
                task: TaskId("browse".into()),
                attempt: None,
                worktree_identity: HerdrWork::worktree_identity(&root)?,
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

#[cfg(test)]
mod tests;
