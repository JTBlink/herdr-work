//! Route input into the original Sidebar editor while retaining one terminal owner.
use crate::{
    layout::Panes,
    menus::{Command, Overlay},
    state::*,
};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use herdr_work_code::editor::{EditAction, SaveOutcome};
use ratatui::{Frame, layout::Rect};

impl App {
    pub(crate) fn start_editor(&mut self) -> Result<()> {
        let root = self
            .code
            .as_ref()
            .map(|c| c.root.clone())
            .unwrap_or_else(|| self.root.clone());
        let path = self.selected_path()?;
        let file = path.strip_prefix(&root)?.to_string_lossy().into_owned();
        let request = path.clone();
        self.job(move |d| Ok(Data::Editor(path, d.edit_file(&root, &file))))?;
        self.edit_line = if self.tab == Tab::Search {
            self.search_rows.get(self.selection).map(|r| r.line)
        } else {
            None
        };
        self.edit_request = Some(request);
        self.status = "正在打开编辑器… Esc 取消".into();
        Ok(())
    }
    /// Called by the central two-second Tick. At most one observation is queued.
    pub(crate) fn poll_editor(&mut self) -> Result<()> {
        let Some(session) = &self.editor else {
            return Ok(());
        };
        if self.editor_poll.is_some() || self.editor_write.is_some() {
            return Ok(());
        }
        let session_id = session.id();
        let observation = session.external_request();
        self.editor_request = self.editor_request.wrapping_add(1);
        let request = self.editor_request;
        self.job(move |_| {
            Ok(Data::EditorObserved(
                session_id,
                request,
                observation.observe(),
            ))
        })?;
        self.editor_poll = Some((session_id, request));
        Ok(())
    }
    pub(crate) fn save_editor(&mut self) -> Result<()> {
        self.save_editor_with(false, false)
    }
    pub(crate) fn save_editor_with(&mut self, overwrite: bool, leave: bool) -> Result<()> {
        if self.editor_write.is_some() {
            self.status = "文件保存或重载正在进行…".into();
            return Ok(());
        }
        let mut session = self.editor.as_ref().context("编辑器尚未打开")?.snapshot();
        let session_id = session.id();
        self.editor_request = self.editor_request.wrapping_add(1);
        let request = self.editor_request;
        self.job(move |_| {
            let result = session
                .save_with_overwrite(overwrite)
                .map(|outcome| (session, outcome));
            Ok(Data::EditorSaved(session_id, request, result))
        })?;
        self.editor_write = Some((session_id, request));
        self.editor_leave_after_save = leave;
        self.status = "正在保存文件…".into();
        Ok(())
    }
    pub(crate) fn reload_editor(&mut self) -> Result<()> {
        if self.editor_write.is_some() {
            return Ok(());
        }
        let mut session = self.editor.as_ref().context("编辑器尚未打开")?.snapshot();
        let expected_text = session.editor.text();
        let session_id = session.id();
        self.editor_request = self.editor_request.wrapping_add(1);
        let request = self.editor_request;
        self.job(move |_| {
            let result = session.reload().map(|()| session);
            Ok(Data::EditorReloaded(
                session_id,
                request,
                expected_text,
                result,
            ))
        })?;
        self.editor_write = Some((session_id, request));
        self.status = "正在重新加载文件…".into();
        Ok(())
    }
    fn editor_conflict(&mut self) {
        self.status = "文件已被外部修改，未覆盖磁盘内容".into();
        self.overlay = Some(Overlay::Menu {
            title: "保存冲突 · 磁盘文件已改变",
            rows: vec![
                (Command::Cancel, "继续编辑（取消保存）"),
                (Command::ReloadEditor, "重新加载磁盘内容（放弃修改）"),
                (Command::OverwriteEditor, "明确覆盖磁盘文件"),
            ],
            selected: 0,
        });
    }
    pub(crate) fn editor_observed(
        &mut self,
        session_id: u64,
        request: u64,
        result: Result<herdr_work_engine::EditorExternalObservation>,
    ) {
        if self.editor_poll != Some((session_id, request)) {
            return;
        }
        self.editor_poll = None;
        let Some(session) = self.editor.as_mut().filter(|s| s.id() == session_id) else {
            return;
        };
        match result {
            Ok(observation) => session.apply_external(observation),
            Err(error) => {
                session.editor.external_changed = true;
                self.status = error.to_string();
            }
        }
    }
    pub(crate) fn editor_saved(
        &mut self,
        session_id: u64,
        request: u64,
        result: Result<(herdr_work_engine::EditorSession, SaveOutcome)>,
    ) {
        if self.editor_write != Some((session_id, request)) {
            return;
        }
        self.editor_write = None;
        let Some(session) = self.editor.as_mut().filter(|s| s.id() == session_id) else {
            return;
        };
        match result {
            Ok((saved, SaveOutcome::Saved)) => {
                session.apply_saved(&saved);
                let leave = self.editor_leave_after_save && !session.editor.dirty;
                self.status = if session.editor.dirty {
                    "文件已保存，后续输入仍未保存"
                } else {
                    "文件已保存"
                }
                .into();
                self.editor_leave_after_save = false;
                let _ = self.refresh();
                if leave && let Err(error) = self.finish_editor() {
                    self.status = error.to_string();
                }
            }
            Ok((_, SaveOutcome::Conflict)) => {
                session.editor.external_changed = true;
                self.editor_conflict();
            }
            Err(error) => {
                self.status = error.to_string();
                self.editor_leave_after_save = false;
            }
        }
    }
    pub(crate) fn editor_reloaded(
        &mut self,
        session_id: u64,
        request: u64,
        expected_text: String,
        result: Result<herdr_work_engine::EditorSession>,
    ) {
        if self.editor_write != Some((session_id, request)) {
            return;
        }
        self.editor_write = None;
        let Some(session) = self.editor.as_mut().filter(|s| s.id() == session_id) else {
            return;
        };
        match result {
            Ok(fresh) => {
                if session.apply_reloaded(&expected_text, fresh) {
                    self.status = "已重新加载磁盘文件".into();
                    if self.editor_leave_after_save
                        && let Err(error) = self.finish_editor()
                    {
                        self.status = error.to_string();
                    }
                } else {
                    self.status = "重载期间有新输入，保留编辑内容；请再次选择重载".into();
                }
            }
            Err(error) => self.status = error.to_string(),
        }
        self.editor_leave_after_save = false;
    }
    pub(crate) fn finish_editor(&mut self) -> Result<()> {
        anyhow::ensure!(self.editor_write.is_none(), "请等待文件保存或重载完成");
        self.editor_poll = None;
        if let Some(session) = self.editor.take() {
            let path = session.editor.path().to_path_buf();
            let file = path
                .strip_prefix(&session.root)?
                .to_string_lossy()
                .into_owned();
            self.preview_path = Some(path.clone());
            self.text = "正在读取文件…".into();
            self.job(move |d| Ok(Data::Preview(path, d.file(&session.root, &file))))?;
            self.focus = Focus::Content;
        }
        Ok(())
    }
    pub(crate) fn input_key(&mut self, key: KeyEvent, area: Rect) -> Result<bool> {
        if let Some(picker) = self.git.picker.as_mut() {
            let action = picker.key(key);
            self.picker_action(action)?;
            return Ok(false);
        }
        if self.overlay.is_some() || self.prompt.is_some() {
            return self.key(key.code);
        }
        if self.edit_commit_key(key)? {
            return Ok(false);
        }
        if self.editor.is_some() {
            let layout = self.panes(area);
            let body = self.content_block(String::new()).inner(layout.content);
            let editor = &mut self.editor.as_mut().unwrap().editor;
            let action = editor.on_key(
                key,
                editor.wrap_width(body.width),
                body.height.max(1) as usize,
            );
            let result = match action {
                EditAction::Save => self.save_editor(),
                EditAction::Leave | EditAction::Close => {
                    if self.editor_write.is_some() {
                        self.status = "请等待文件保存或重载完成".into();
                        Ok(())
                    } else if self.editor.as_ref().unwrap().editor.dirty {
                        self.overlay = Some(Overlay::Menu {
                            title: "未保存的文本",
                            rows: vec![
                                (Command::SaveEditor, "保存并返回"),
                                (Command::DiscardEditor, "放弃修改并返回"),
                                (Command::Cancel, "继续编辑"),
                            ],
                            selected: 0,
                        });
                        Ok(())
                    } else {
                        self.finish_editor()
                    }
                }
                EditAction::None => Ok(()),
            };
            if let Err(error) = result {
                self.status = error.to_string();
            }
            return Ok(false);
        }
        if self.edit_request.is_some() {
            if key.code == KeyCode::Esc {
                self.edit_request = None;
                self.status = "已取消打开编辑器".into();
            }
            return Ok(false);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('p' | 'P' | 'f' | 'F'))
        {
            self.query_prompt(matches!(key.code, KeyCode::Char('p' | 'P')));
            return Ok(false);
        }
        if key.code == KeyCode::F(12) {
            self.query_prompt(false);
            return Ok(false);
        }
        self.key(key.code)
    }
    pub(crate) fn draw_editor(&mut self, f: &mut Frame, panes: &Panes) {
        let Some(session) = &self.editor else {
            return;
        };
        if panes.content.width == 0 {
            return;
        }
        let title = format!(
            "编辑 · {}{}{}",
            session.editor.name(),
            if session.editor.dirty {
                " ● 未保存"
            } else {
                ""
            },
            if session.editor.external_changed {
                " · 磁盘已改变"
            } else {
                ""
            }
        );
        let block = self.content_block(title);
        let body = block.inner(panes.content);
        f.render_widget(block, panes.content);
        let footer = Rect::new(
            panes.footer.x,
            panes.footer.y.saturating_add(1),
            panes.footer.width,
            panes.footer.height.saturating_sub(1),
        );
        self.editor.as_mut().unwrap().editor.draw(
            f,
            body,
            footer,
            Some("Ctrl+S 保存 · Esc 返回 · Ctrl+F 查找 · 滚轮滚动"),
        );
    }
}
