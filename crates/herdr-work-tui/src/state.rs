//! One terminal owner, with background jobs and upstream-backed views.
use crate::loading::LoadState;
use anyhow::{Context as _, Result};
use crossterm::event::Event;
use herdr_work_core::{Context, Execution, ExecutionId, Review, Task, TaskId};
use herdr_work_engine::{CodeSnapshot, DiffView, HerdrWork, JobQueue};
use std::path::PathBuf;

pub(crate) enum Data {
    Preferences(crate::preferences::Update),
    Persistence(Box<crate::persistence::Update>),
    Background(Box<crate::refresh::BackgroundRefresh>),
    Tasks(Result<Vec<Task>>),
    ActivationReported(Result<()>),
    ActivationRead(u64, Result<Option<herdr_work_engine::ActivationRequest>>),
    Code(u64, Result<CodeSnapshot>),
    Diff(DiffView, Review),
    CommitReady(u64, Result<herdr_work_engine::CommitPlan>),
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
    GitReady(u64, Result<herdr_work_engine::GitPlan>),
    WorkGitReady(u64, Result<herdr_work_engine::WorkGitPlan>),
    WorkGitDone(Result<herdr_work_engine::WorkGitOutcome>),
    CommitDone(PathBuf, String, Result<String>),
    BranchPicker(u64, Result<herdr_work_code::branch_ui::BranchPicker>),
    Reconcile(u64, Result<Vec<(String, String)>>),
    Suggested(u64, PathBuf, herdr_work_engine::CommitDraft, Result<String>),
    GitDraftSaved(PathBuf, u64, Result<()>),
    GitDraftFlush,
}
pub(crate) enum Message {
    Input(Event),
    Result(u64, Box<Result<Data>>),
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
#[derive(Clone)]
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
    WorkGit(herdr_work_engine::WorkGitPlan),
    Resolve(String),
}
pub(crate) struct App {
    pub(crate) background_pending: Option<(u64, u64)>,
    pub(crate) last_background: std::time::Instant,
    pub(crate) activation: Option<crate::activation::ActivationRuntime>,
    pub(crate) desk: HerdrWork,
    pub(crate) interaction: crate::interaction::Interaction,
    pub(crate) context: Context,
    pub(crate) tasks: Vec<Task>,
    pub(crate) task: usize,
    pub(crate) tab: Tab,
    pub(crate) selection: usize,
    pub(crate) scm_selection: usize,
    pub(crate) focus: Focus,
    pub(crate) preview_doc: Option<herdr_work_code::preview::Doc>,
    pub(crate) preview_request: u64,
    pub(crate) preview_loading: Option<(u64, u64)>,
    pub(crate) preview_pending: Option<(u64, PathBuf, Option<usize>)>,
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
    pub(crate) git: crate::scm::GitUi,
    pub(crate) hover: crate::hover::HoverState,
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
    pub(crate) drawer_doc: Option<herdr_work_code::preview::Doc>,
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
        let workspace = desk.workspace_ui_state(&root)?;
        let expanded = workspace
            .tree
            .expanded
            .iter()
            .map(|path| root.join(path))
            .collect();
        let file_selection = workspace.tree.selected.as_ref().map(|path| root.join(path));
        let tab = match workspace.activity.as_str() {
            "git" => Tab::Scm,
            "search" => Tab::Search,
            "tasks" => Tab::Tasks,
            "review" => Tab::Review,
            _ => Tab::Files,
        };
        Ok(Self {
            background_pending: None,
            last_background: std::time::Instant::now(),
            desk,
            interaction: crate::interaction::Interaction {
                preference_base: preferences.clone(),
                file_selection,
                restore: (!workspace.activity.is_empty()).then(|| workspace.clone()),
                workspace,
                ..Default::default()
            },
            activation: None,

            context,
            tasks: vec![],
            task: 0,
            tab,
            selection: 0,
            scm_selection: 0,
            focus: Focus::Sidebar,
            preview_doc: None,
            preview_request: 0,
            preview_loading: None,
            preview_pending: None,
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
            expanded,
            generation: 0,
            prompt: None,
            input: String::new(),
            jobs: JobQueue::default(),
            preferences,
            overlay: None,
            editor: None,
            edit_request: None,
            git: Default::default(),
            hover: Default::default(),
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
            drawer_doc: None,
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
        let decorate = self.preferences.git_decorations;
        self.job(move |d| {
            Ok(Data::Code(
                generation,
                d.code_snapshot_with_options(&root, &expanded, generation, show_hidden, decorate),
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
        // Guarded editor sessions and commit drafts can outlive unrelated binding epochs.
        if epoch != self.context.epoch
            && !matches!(
                &data,
                Ok(Data::Tasks(_)
                    | Data::Preferences(..)
                    | Data::Persistence(..)
                    | Data::ActivationRead(..)
                    | Data::CommitDone(..)
                    | Data::GitDraftSaved(..)
                    | Data::GitDraftFlush
                    | Data::EditorObserved(..)
                    | Data::EditorSaved(..)
                    | Data::EditorReloaded(..))
            )
        {
            return;
        }
        match data {
            Ok(Data::Preferences(update)) => self.apply_preferences(update),
            Ok(Data::Persistence(update)) => self.apply_persistence(*update),
            Ok(Data::Background(update)) => self.apply_background(*update),
            Err(e) => self.status = format!("{e:#}"),
            Ok(Data::ActivationReported(Err(error))) => {
                self.status = format!("自有宿主入口注册失败：{error:#}")
            }
            Ok(Data::ActivationReported(Ok(()))) => {}
            Ok(Data::ActivationRead(request, result)) => self.activation_read(request, result),
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
            Ok(Data::GitDraftSaved(root, revision, result)) => {
                self.git_draft_saved(root, revision, result)
            }
            Ok(Data::GitDraftFlush) => {}
            Ok(data) if epoch != self.context.epoch && !matches!(&data, Data::CommitDone(..)) => {}
            Ok(Data::Code(generation, _)) if generation != self.generation => {}
            Ok(Data::Code(_, Err(error))) => {
                self.code_state = LoadState::Failed(format!("{error:#}"));
            }
            Ok(Data::Code(_, Ok(code))) => {
                let previous_scm = self.scm_rows().get(self.scm_cursor).map(|r| r.3.clone());
                let previous_root = self.selected_repository();
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
                for repo in &code.repositories {
                    if repo.draft_error.is_none() {
                        self.git
                            .drafts
                            .entry(repo.root.clone())
                            .or_insert_with(|| repo.draft.clone());
                    }
                }
                self.code = Some(code);
                if was_change {
                    self.scm_cursor = self
                        .scm_rows()
                        .iter()
                        .position(|r| r.0 == Some(self.selection))
                        .unwrap_or(0);
                } else if let Some(previous) = previous_scm {
                    use crate::scm::ScmTarget;
                    self.scm_cursor = self
                        .scm_rows()
                        .iter()
                        .position(|r| match (&previous, &r.3) {
                            (ScmTarget::Repository(_), ScmTarget::Repository(i))
                            | (ScmTarget::Message(_), ScmTarget::Message(i))
                            | (ScmTarget::Commit(_), ScmTarget::Commit(i)) => self
                                .repository_root(*i)
                                .is_ok_and(|root| root == previous_root),
                            (ScmTarget::Section(_, old), ScmTarget::Section(i, new)) => {
                                old == new
                                    && self
                                        .repository_root(*i)
                                        .is_ok_and(|root| root == previous_root)
                            }
                            (
                                ScmTarget::Directory(_, staged, path),
                                ScmTarget::Directory(i, new_staged, new_path),
                            ) => {
                                staged == new_staged
                                    && path == new_path
                                    && self
                                        .repository_root(*i)
                                        .is_ok_and(|root| root == previous_root)
                            }
                            _ => previous == r.3,
                        })
                        .unwrap_or(self.scm_cursor.min(self.scm_rows().len().saturating_sub(1)));
                }
                if let Err(error) = self.restore_workspace_after_code() {
                    self.status = error.to_string();
                }
            }
            Ok(Data::CommitReady(request, result))
                if request == self.git.request
                    && self.prompt.is_none()
                    && self.overlay.is_none() =>
            {
                match result {
                    Ok(plan) => {
                        let _ = self.ensure_draft(&plan.root);
                        self.input = self
                            .git
                            .drafts
                            .get(&plan.root)
                            .map(|d| d.message.iter().collect())
                            .unwrap_or_default();
                        self.prompt = Some(Prompt::Commit(plan));
                    }
                    Err(error) => self.status = format!("提交未准备：{error:#}（草稿已保留）"),
                }
            }
            Ok(Data::CommitReady(..)) => {}
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
            Ok(Data::Preview(request, path, result)) => {
                if self.preview_loading == Some((epoch, request)) {
                    self.preview_loading = None;
                }
                if request == self.preview_request && self.preview_path.as_ref() == Some(&path) {
                    match result {
                        Ok(doc) => self.apply_preview(path, doc, false),
                        Err(error) => {
                            self.preview_doc = None;
                            self.text = format!("无法预览文件：{error:#}");
                        }
                    }
                }
                if let Err(error) = self.dispatch_preview() {
                    self.status = error.to_string();
                }
            }

            Ok(Data::WorkGitReady(request, result))
                if request == self.git.request
                    && self.prompt.is_none()
                    && self.overlay.is_none() =>
            {
                match result {
                    Ok(plan) => {
                        self.git.active = Some(plan.root.clone());
                        self.input.clear();
                        self.prompt = Some(Prompt::WorkGit(plan));
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
            Ok(Data::WorkGitReady(..)) => {}
            Ok(Data::BranchPicker(request, result))
                if request == self.git.request
                    && self.prompt.is_none()
                    && self.overlay.is_none() =>
            {
                match result {
                    Ok(picker) => self.git.picker = Some(picker),
                    Err(error) => self.status = error.to_string(),
                }
            }
            Ok(Data::BranchPicker(..)) => {}
            Ok(Data::WorkGitDone(result)) => match result {
                Ok(herdr_work_engine::WorkGitOutcome::Finished(text)) => {
                    self.apply(epoch, Ok(Data::Business(text)))
                }
                Ok(herdr_work_engine::WorkGitOutcome::DeleteNotMerged(plan)) => {
                    self.offer_force_delete(plan);
                }
                Err(error) => {
                    self.status = format!("{error:#}；结果未知时请从 Git 菜单选择核对恢复");
                    let _ = self.refresh();
                }
            },
            Ok(Data::CommitDone(root, message, result)) => {
                self.git.committing.remove(&root);
                match result {
                    Ok(text) => {
                        if self
                            .git
                            .drafts
                            .get(&root)
                            .is_some_and(|d| d.message.iter().collect::<String>() == message)
                        {
                            self.git.drafts.insert(root.clone(), Default::default());
                            self.mark_git_draft(root.clone());
                        }
                        self.apply(epoch, Ok(Data::Business(text)));
                    }
                    Err(error) if epoch == self.context.epoch => {
                        self.status = format!("提交失败（草稿已保留）：{error:#}")
                    }
                    Err(_) => {}
                }
            }
            Ok(Data::Suggested(request, root, baseline, result)) if request == self.git.request => {
                match result {
                    Ok(message) if self.git.drafts.get(&root).is_some_and(|d| *d == baseline) => {
                        let draft = herdr_work_engine::CommitDraft {
                            message: message.chars().collect(),
                            cursor: message.chars().count(),
                        };
                        self.git.drafts.insert(root.clone(), draft);
                        self.mark_git_draft(root);
                        self.status = "提交消息建议已生成".into();
                    }
                    Ok(_) => self.status = "提交消息已编辑，保留当前草稿".into(),
                    Err(error) => self.status = error.to_string(),
                }
            }
            Ok(Data::Suggested(..)) => {}
            Ok(Data::Reconcile(request, result))
                if request == self.git.request
                    && self.prompt.is_none()
                    && self.overlay.is_none() =>
            {
                match result {
                    Ok(operations) => {
                        self.git.unresolved = operations;
                        self.overlay = Some(crate::menus::Overlay::Recovery(0));
                    }
                    Err(error) => self.status = error.to_string(),
                }
            }
            Ok(Data::Reconcile(..)) => {}
            Ok(Data::GitReady(request, result))
                if request == self.git.request
                    && self.prompt.is_none()
                    && self.overlay.is_none() =>
            {
                match result {
                    Ok(plan) => {
                        self.input.clear();
                        self.prompt = Some(Prompt::Git(plan));
                    }
                    Err(error) => self.status = error.to_string(),
                }
            }
            Ok(Data::GitReady(..)) => {}
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
                        if self.tab == Tab::Search {
                            self.selection = 0;
                        }
                        if view.truncated && self.tab == Tab::Search {
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
                let text = result.unwrap_or_else(|e| format!("无法显示 Git 对象：{e}"));
                self.drawer_doc = Some(herdr_work_code::preview::git_show(
                    "Git Changes".into(),
                    String::new(),
                    &text,
                ));
                self.drawer_show = Some(text);
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
        self.queue_workspace_save()?;
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
        self.load_workspace_after_change()?;
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
        self.queue_workspace_save()?;
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
        self.load_workspace_after_change()?;
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
