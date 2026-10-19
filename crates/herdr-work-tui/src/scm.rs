//! Sidebar drawer navigation, backed by its extracted shared model and queries.
use crate::{loading::LoadState, state::*};
use anyhow::{Context, Result};
use herdr_work_code::scm::Drawer;
use herdr_work_engine::{CommitDraft, WorkGitAction};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
};

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum ScmTarget {
    Repository(usize),
    Commit(usize),
    Message(usize),
    Section(usize, bool),
    Directory(usize, bool, String),
    Change(usize),
    Drawer(Drawer),
    History(Drawer, usize),
}
#[derive(Default)]
pub(crate) struct GitUi {
    pub active: Option<PathBuf>,
    pub drafts: HashMap<PathBuf, CommitDraft>,
    pub editing: Option<PathBuf>,
    pub tree: bool,
    pub collapsed: HashMap<(PathBuf, bool), BTreeSet<String>>,
    pub repository_collapsed: BTreeSet<PathBuf>,
    pub section_collapsed: BTreeSet<(PathBuf, bool)>,
    pub request: u64,
    pub pending_force: Option<herdr_work_engine::WorkGitPlan>,
    pub unresolved: Vec<(String, String)>,
    pub picker: Option<herdr_work_code::branch_ui::BranchPicker>,
    pub committing: BTreeSet<PathBuf>,
    pub draft_runtime: crate::drafts::DraftRuntime,
}
pub(crate) type ScmRow = (Option<usize>, String, char, ScmTarget);
impl App {
    fn force_delete_idle(&self) -> bool {
        self.prompt.is_none()
            && self.overlay.is_none()
            && self.editor.is_none()
            && self.edit_request.is_none()
            && self.git.editing.is_none()
            && self.git.picker.is_none()
    }
    pub(crate) fn offer_force_delete(&mut self, plan: herdr_work_engine::WorkGitPlan) {
        self.git.pending_force = Some(plan);
        if self.force_delete_idle() && self.input.is_empty() {
            let _ = self.resume_force_delete();
        } else {
            self.status = "分支尚未合并；已保留计划，完成当前输入后从 Git 菜单重开强删确认".into();
        }
    }
    pub(crate) fn resume_force_delete(&mut self) -> Result<()> {
        anyhow::ensure!(self.force_delete_idle(), "请先完成当前输入，再重开强删确认");
        let plan = self
            .git
            .pending_force
            .take()
            .context("没有待确认的未合并分支")?;
        self.git.active = Some(plan.root.clone());
        // Explicitly reopening the confirmation replaces only inactive input;
        // every active input owner was checked above.
        self.input.clear();
        self.prompt = Some(Prompt::WorkGit(plan));
        self.status = "分支尚未合并；强删会丢失提交，请再次明确确认".into();
        Ok(())
    }

    pub(crate) fn load_drawer(&mut self, kind: Drawer) -> Result<()> {
        let preserve_context = self.active_drawer == Some(kind) && self.focus == Focus::Content;
        self.drawer_request += 1;
        let request = self.drawer_request;
        self.active_drawer = Some(kind);
        self.drawer_open[kind.index()] = true;
        self.drawer_state = LoadState::Loading;
        self.drawer_scroll = 0;
        self.drawer_show = None;
        self.drawer_doc = None;
        let root = self.selected_repository();
        let file = self
            .code
            .as_ref()
            .and_then(|c| c.changes.get(self.selection))
            .filter(|c| c.root == root)
            .map(|c| c.path.clone());
        // Reopening an already browsed drawer retains its immutable query context.
        let (root, file) = self
            .drawers
            .get(&kind)
            .filter(|_| preserve_context)
            .map(|v| (v.root.clone(), v.file.clone()))
            .unwrap_or((root, file));
        self.job(move |d| {
            Ok(Data::Drawer(
                request,
                d.scm_drawer(&root, kind, file.as_deref()),
            ))
        })
    }
    pub(crate) fn activate_scm_row(&mut self) -> Result<()> {
        let rows = self.scm_rows();
        let Some(row) = rows.get(self.scm_cursor) else {
            return Ok(());
        };
        match row.3.clone() {
            ScmTarget::Repository(index) => {
                let root = self.repository_root(index)?;
                self.git.active = Some(root.clone());
                if !self.git.repository_collapsed.remove(&root) {
                    self.git.repository_collapsed.insert(root);
                }
            }
            ScmTarget::Message(index) => {
                let root = self.repository_root(index)?;
                self.git.active = Some(root.clone());
                self.ensure_draft(&root)?;
                self.git.editing = Some(root);
            }
            ScmTarget::Commit(index) => {
                self.git.active = Some(self.repository_root(index)?);
                self.prepare_commit()?;
            }
            ScmTarget::Change(index) => {
                self.git.active = self
                    .code
                    .as_ref()
                    .and_then(|c| c.changes.get(index))
                    .map(|c| c.root.clone());
                self.selection = index;
                self.scm_selection = index;
                self.active_drawer = None;
                let change = self
                    .code
                    .as_ref()
                    .and_then(|c| c.changes.get(index))
                    .context("Git 更改已失效")?
                    .clone();
                self.open_diff(
                    change.root,
                    change.path,
                    if change.staged { "staged" } else { "unstaged" }.into(),
                )?;
            }
            ScmTarget::Drawer(kind) => {
                if self.drawer_open[kind.index()] && self.active_drawer == Some(kind) {
                    self.drawer_open[kind.index()] = false;
                    self.active_drawer = None;
                } else {
                    self.load_drawer(kind)?;
                }
            }
            ScmTarget::History(kind, index) => {
                let view = self.drawers.get(&kind).context("Git 列表尚未加载")?.clone();
                self.drawer_request += 1;
                let request = self.drawer_request;
                self.job(move |d| Ok(Data::Show(request, d.scm_show(&view, index))))?;
            }
            ScmTarget::Section(index, staged) => {
                let key = (self.repository_root(index)?, staged);
                self.git.active = Some(key.0.clone());
                if !self.git.section_collapsed.remove(&key) {
                    self.git.section_collapsed.insert(key);
                }
            }
            ScmTarget::Directory(index, staged, path) => {
                let root = self.repository_root(index)?;
                self.git.active = Some(root.clone());
                let collapsed = self.git.collapsed.entry((root, staged)).or_default();
                if !collapsed.remove(&path) {
                    collapsed.insert(path);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn repository_root(&self, index: usize) -> Result<PathBuf> {
        Ok(self
            .code
            .as_ref()
            .and_then(|c| c.repositories.get(index))
            .context("仓库已失效")?
            .root
            .clone())
    }
    pub(crate) fn selected_repository(&self) -> PathBuf {
        if let Some(row) = self.scm_rows().get(self.scm_cursor) {
            let index = match &row.3 {
                ScmTarget::Repository(i)
                | ScmTarget::Commit(i)
                | ScmTarget::Message(i)
                | ScmTarget::Section(i, _)
                | ScmTarget::Directory(i, _, _) => Some(*i),
                ScmTarget::Change(i) => {
                    return self.code.as_ref().unwrap().changes[*i].root.clone();
                }
                _ => None,
            };
            if let Some(i) = index
                && let Ok(root) = self.repository_root(i)
            {
                return root;
            }
        }
        self.git
            .active
            .clone()
            .or_else(|| {
                self.code
                    .as_ref()
                    .and_then(|c| c.repositories.first())
                    .map(|r| r.root.clone())
            })
            .unwrap_or_else(|| self.root.clone())
    }
    pub(crate) fn ensure_draft(&mut self, root: &std::path::Path) -> Result<()> {
        if !self.git.drafts.contains_key(root) {
            let repo = self
                .code
                .as_ref()
                .and_then(|c| c.repositories.iter().find(|r| r.root == root))
                .context("仓库草稿正在加载，请稍候")?;
            anyhow::ensure!(
                repo.draft_error.is_none(),
                "无法读取草稿（保留原文件）：{}",
                repo.draft_error.as_deref().unwrap_or_default()
            );
            self.git.drafts.insert(root.into(), repo.draft.clone());
        }
        Ok(())
    }
    pub(crate) fn edit_commit_key(&mut self, key: crossterm::event::KeyEvent) -> Result<bool> {
        let Some(root) = self.git.editing.clone() else {
            return Ok(false);
        };
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
                self.git.editing = None;
            }
            KeyCode::Enter => {
                self.git.editing = None;
                self.prepare_commit_root(root.clone())?;
            }
            _ => {
                let draft = self.git.drafts.get_mut(&root).context("提交输入已失效")?;
                herdr_work_code::scm_input::edit_message(
                    &mut draft.message,
                    &mut draft.cursor,
                    key,
                );
                self.mark_git_draft(root.clone());
            }
        }
        Ok(true)
    }
    pub(crate) fn persist_commit_prompt(&mut self) -> Result<()> {
        if let Some(Prompt::Commit(plan)) = &self.prompt {
            let draft = self.git.drafts.entry(plan.root.clone()).or_default();
            draft.message = self.input.chars().collect();
            draft.cursor = draft.message.len();
            let root = plan.root.clone();
            self.mark_git_draft(root);
        }
        Ok(())
    }
    pub(crate) fn suggest_commit(&mut self) -> Result<()> {
        let root = self.selected_repository();
        self.ensure_draft(&root)?;
        let baseline = self.git.drafts[&root].clone();
        self.git.request += 1;
        let request = self.git.request;
        self.status = "正在生成提交消息…".into();
        self.job(move |d| {
            let message = d.suggest_commit(&root);
            Ok(Data::Suggested(request, root, baseline, message))
        })
    }
    pub(crate) fn paste_commit(&mut self, text: &str) -> Result<bool> {
        let Some(root) = self.git.editing.clone() else {
            return Ok(false);
        };
        let draft = self.git.drafts.get_mut(&root).context("提交输入已失效")?;
        anyhow::ensure!(
            draft.message.len() + text.chars().count() <= 65536,
            "提交消息超过限制"
        );
        for c in text.chars() {
            draft.message.insert(draft.cursor, c);
            draft.cursor += 1;
        }
        self.mark_git_draft(root);
        Ok(true)
    }
    pub(crate) fn prepare_commit(&mut self) -> Result<()> {
        let root = self.selected_repository();
        self.prepare_commit_root(root)
    }
    fn prepare_commit_root(&mut self, root: PathBuf) -> Result<()> {
        self.git.active = Some(root.clone());
        self.ensure_draft(&root)?;
        anyhow::ensure!(!self.git.committing.contains(&root), "此仓库正在提交");
        self.git.request += 1;
        let request = self.git.request;
        self.job(move |d| Ok(Data::CommitReady(request, d.prepare_commit(&root))))
    }
    pub(crate) fn work_git(&mut self, action: WorkGitAction) -> Result<()> {
        let root = self.selected_repository();
        self.git.active = Some(root.clone());
        self.git.request += 1;
        let request = self.git.request;
        self.job(move |d| {
            Ok(Data::WorkGitReady(
                request,
                d.prepare_work_git(&root, action),
            ))
        })
    }
    pub(crate) fn stage_explorer(&mut self) -> Result<()> {
        let path = self.selected_path()?;
        self.git.request += 1;
        let request = self.git.request;
        self.job(move |d| Ok(Data::WorkGitReady(request, d.prepare_explorer_stage(&path))))
    }
    pub(crate) fn open_branches(&mut self) -> Result<()> {
        let root = self.selected_repository();
        self.git.active = Some(root.clone());
        self.git.request += 1;
        let request = self.git.request;
        self.job(move |d| Ok(Data::BranchPicker(request, d.branch_picker(&root))))
    }
    pub(crate) fn picker_action(
        &mut self,
        action: herdr_work_code::branch_ui::PickerAction,
    ) -> Result<()> {
        use herdr_work_code::branch_ui::PickerAction;
        match action {
            PickerAction::None => return Ok(()),
            PickerAction::Close => {
                self.git.picker = None;
                return Ok(());
            }
            PickerAction::Checkout(branch) => {
                self.git.picker = None;
                self.work_git(WorkGitAction::Checkout {
                    name: branch.name,
                    remote: branch.remote,
                })?;
            }
            PickerAction::Create(name) => {
                self.git.picker = None;
                self.work_git(WorkGitAction::CreateBranch(name))?;
            }
        }
        Ok(())
    }
    pub(crate) fn draw_drawer(&mut self, f: &mut Frame, area: Rect) {
        let Some(kind) = self.active_drawer else {
            return;
        };
        let title = format!(
            "{} · {}",
            kind.title(),
            self.drawers
                .get(&kind)
                .map(|v| v
                    .root
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned())
                .unwrap_or_default()
        );
        if self.drawer_show.is_some() {
            let block = self.content_block(title);
            let body = block.inner(area);
            f.render_widget(block, area);
            if let Some(doc) = &mut self.drawer_doc {
                doc.draw_body(f, body);
                self.drawer_scroll = doc.scroll().min(u16::MAX as usize) as u16;
            }
            return;
        }
        let text = if let Some(text) = &self.drawer_show {
            text.clone()
        } else {
            match &self.drawer_state {
                LoadState::Loading => "正在加载 Git 数据…".into(),
                LoadState::Failed(error) => format!("加载失败：{error}\n按 r 重试"),
                LoadState::Ready => self
                    .drawers
                    .get(&kind)
                    .map(|v| {
                        if v.lines.is_empty() {
                            "(none)".into()
                        } else {
                            v.lines.join("\n")
                        }
                    })
                    .unwrap_or_default(),
            }
        };
        f.render_widget(
            Paragraph::new(ratatui::text::Text::from(herdr_work_code::ansi::to_lines(
                &text,
            )))
            .scroll((self.drawer_scroll, 0))
            .block(self.content_block(title)),
            area,
        );
    }
}
