//! Route input into the original Sidebar editor while retaining one terminal owner.
use crate::{
    layout::Panes,
    menus::{Command, Overlay},
    state::*,
};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use herdr_work_code::editor::EditAction;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders},
};

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
    pub(crate) fn save_editor(&mut self) -> Result<()> {
        self.editor.as_mut().context("编辑器尚未打开")?.save()?;
        self.status = "文件已保存".into();
        self.refresh()
    }
    pub(crate) fn finish_editor(&mut self) -> Result<()> {
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
        if self.overlay.is_some() || self.prompt.is_some() {
            return self.key(key.code);
        }
        if self.editor.is_some() {
            let layout = self.panes(area);
            let editor = &mut self.editor.as_mut().unwrap().editor;
            let action = editor.on_key(
                key,
                editor.wrap_width(layout.content.width.saturating_sub(2)),
                layout.content.height.saturating_sub(2).max(1) as usize,
            );
            let result = match action {
                EditAction::Save => self.save_editor(),
                EditAction::Leave | EditAction::Close => {
                    if self.editor.as_ref().unwrap().editor.dirty {
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
        let Some(session) = &mut self.editor else {
            return;
        };
        if panes.content.width == 0 {
            return;
        }
        let title = format!(
            "编辑 · {}{}",
            session.editor.name(),
            if session.editor.dirty {
                " ● 未保存"
            } else {
                ""
            }
        );
        let block = Block::default().borders(Borders::ALL).title(title);
        let body = block.inner(panes.content);
        f.render_widget(block, panes.content);
        let footer = Rect::new(
            panes.footer.x,
            panes.footer.y,
            panes.footer.width,
            panes.footer.height.min(2),
        );
        session.editor.draw(
            f,
            body,
            footer,
            Some(&format!(
                "{} · Ctrl+S 保存 · Esc 返回 · Ctrl+F 查找",
                self.status
            )),
        );
    }
}
