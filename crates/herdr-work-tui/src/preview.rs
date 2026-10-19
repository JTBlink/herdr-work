//! Product document state around the shared source viewer; no pane lifecycle.
use crate::state::*;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use herdr_work_code::preview::{Doc, ReleaseCopier, ReleaseCopy};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(crate) struct PreviewDocument {
    pub(crate) path: PathBuf,
    pub(crate) doc: Doc,
    pub(crate) target: Option<usize>,
}

impl App {
    fn readonly_doc(&self) -> Option<&Doc> {
        if self.tab == Tab::Scm {
            self.drawer_doc.as_ref()
        } else {
            self.preview_doc.as_ref()
        }
    }
    fn readonly_doc_mut(&mut self) -> Option<&mut Doc> {
        if self.tab == Tab::Scm {
            self.drawer_doc.as_mut()
        } else {
            self.preview_doc.as_mut()
        }
    }
    pub(crate) fn preview_body(&self, area: Rect) -> Rect {
        self.content_block(String::new()).inner(area)
    }
    pub(crate) fn open_preview(&mut self, path: PathBuf, line: Option<usize>) -> Result<()> {
        self.edit_request = None;
        self.preview_request = self.preview_request.wrapping_add(1);
        self.preview_pending = None;
        if self.preview_path.as_ref() != Some(&path) {
            if let (Some(old), Some(doc)) = (self.preview_path.take(), self.preview_doc.take())
                && self.preview_pinned
            {
                self.preview_documents.push(PreviewDocument {
                    target: self
                        .preview_target
                        .as_ref()
                        .filter(|(p, _)| p == &old)
                        .map(|(_, line)| *line),
                    path: old,
                    doc,
                });
            }
            self.preview_pinned = false;
            if let Some(index) = self
                .preview_documents
                .iter()
                .position(|entry| entry.path == path)
            {
                let mut saved = self.preview_documents.remove(index);
                let target = line.or(saved.target);
                self.preview_target = target.map(|line| (path.clone(), line));
                if let Some(line) = target {
                    saved.doc.goto_line(line);
                }
                self.preview_scroll = saved.doc.scroll().min(u16::MAX as usize) as u16;
                self.text = saved.doc.plain_text();
                self.preview_doc = Some(saved.doc);
                self.preview_path = Some(path);
                self.preview_pinned = true;
                return Ok(());
            }
        }
        self.preview_path = Some(path.clone());
        self.preview_target = line.map(|line| (path.clone(), line));
        self.preview_scroll = line.unwrap_or(1).saturating_sub(1).min(u16::MAX as usize) as u16;
        self.text = "正在读取文件…".into();
        self.preview_pending = Some((self.preview_request, path, line));
        self.dispatch_preview()
    }
    pub(crate) fn dispatch_preview(&mut self) -> Result<()> {
        if self
            .preview_loading
            .is_some_and(|(epoch, _)| epoch == self.context.epoch)
        {
            return Ok(());
        }
        let Some((request, path, line)) = self.preview_pending.clone() else {
            return Ok(());
        };
        let root = self.root.clone();
        let file = path.strip_prefix(&root)?.to_string_lossy().into_owned();
        let width = self.preview_width;
        self.job(move |desk| {
            Ok(Data::Preview(
                request,
                path,
                desk.preview_file(&root, &file, line, width),
            ))
        })?;
        self.preview_pending = None;
        self.preview_loading = Some((self.context.epoch, request));
        Ok(())
    }
    pub(crate) fn apply_preview(&mut self, path: PathBuf, mut doc: Doc, passive: bool) {
        if self.preview_path.as_ref() != Some(&path) {
            return;
        }
        if passive {
            if let Some(current) = &mut self.preview_doc {
                current.refresh(doc);
            } else {
                self.preview_doc = Some(doc);
            }
        } else {
            if let Some((_, line)) = self.preview_target.as_ref().filter(|(p, _)| p == &path) {
                doc.goto_line(*line);
            }
            self.preview_doc = Some(doc);
        }
        if let Some(doc) = &self.preview_doc {
            self.text = doc.plain_text();
        }
    }
    pub(crate) fn preview_title(&self) -> String {
        self.preview_path
            .as_ref()
            .map(|path| {
                format!(
                    "[{}{}]",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    if self.preview_pinned {
                        ""
                    } else {
                        " · preview"
                    }
                )
            })
            .into_iter()
            .chain(
                self.preview_documents
                    .iter()
                    .map(|entry| entry.doc.name().to_string()),
            )
            .collect::<Vec<_>>()
            .join("  ")
    }
    pub(crate) fn draw_preview(&mut self, frame: &mut Frame, area: Rect) {
        let title = self.preview_title();
        let block = self.content_block(if title.is_empty() {
            "Preview".into()
        } else {
            title
        });
        let body = block.inner(area);
        frame.render_widget(block, area);
        self.preview_width = body.width;
        if let Some(doc) = &mut self.preview_doc {
            doc.draw_body(frame, body);
            self.preview_scroll = doc.scroll().min(u16::MAX as usize) as u16;
        } else {
            frame.render_widget(
                Paragraph::new(if self.preview_path.is_some() {
                    self.text.as_str()
                } else {
                    "选择文件，按 Enter 或点击预览。\ne 编辑 · p 固定 · [/] 切换文档 · Esc 关闭预览"
                }),
                body,
            );
        }
    }
    pub(crate) fn preview_key(&mut self, key: KeyEvent, area: Rect) -> bool {
        if self.editor.is_some()
            || self.focus != Focus::Content
            || !matches!(self.tab, Tab::Files | Tab::Search | Tab::Scm)
        {
            return false;
        }
        let body = self.preview_body(self.panes(area).content);
        let shortcut = key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::SUPER);
        let scm = self.tab == Tab::Scm;
        let Some(doc) = self.readonly_doc_mut() else {
            return false;
        };
        doc.prepare(body);
        let page = usize::from(body.height).saturating_sub(1).max(1);
        match key.code {
            KeyCode::Char('a') if shortcut => doc.select_all_text(),
            KeyCode::Char('c') if shortcut => {
                self.copy_preview(false);
            }
            KeyCode::Char('w') => doc.toggle_wrap(),
            KeyCode::Down | KeyCode::Char('j') => doc.set_scroll(
                doc.scroll()
                    .saturating_add(1)
                    .min(doc.row_count().saturating_sub(1)),
            ),
            KeyCode::Up | KeyCode::Char('k') => doc.set_scroll(doc.scroll().saturating_sub(1)),
            KeyCode::PageDown | KeyCode::Char(' ') => doc.set_scroll(
                doc.scroll()
                    .saturating_add(page)
                    .min(doc.row_count().saturating_sub(1)),
            ),
            KeyCode::PageUp => doc.set_scroll(doc.scroll().saturating_sub(page)),
            KeyCode::Home => doc.set_scroll(0),
            KeyCode::End => {
                doc.set_scroll(doc.row_count().saturating_sub(usize::from(body.height)))
            }
            KeyCode::Char('p') if !scm => self.preview_pinned = true,
            KeyCode::Char('[') if !scm => self.switch_preview(false),
            KeyCode::Char(']') if !scm => self.switch_preview(true),
            KeyCode::Esc if scm => {
                self.drawer_doc = None;
                self.drawer_show = None;
            }
            KeyCode::Esc => self.close_preview(),
            _ => return false,
        }
        if let Some(doc) = &self.preview_doc {
            self.preview_scroll = doc.scroll().min(u16::MAX as usize) as u16;
        }
        true
    }
    fn switch_preview(&mut self, forward: bool) {
        self.preview_pending = None;
        if self.preview_documents.is_empty() {
            return;
        }
        self.preview_request = self.preview_request.wrapping_add(1);
        let index = if forward {
            0
        } else {
            self.preview_documents.len() - 1
        };
        let saved = self.preview_documents.remove(index);
        if self.preview_pinned
            && let (Some(path), Some(doc)) = (self.preview_path.take(), self.preview_doc.take())
        {
            self.preview_documents.push(PreviewDocument {
                target: self
                    .preview_target
                    .as_ref()
                    .filter(|(p, _)| p == &path)
                    .map(|(_, line)| *line),
                path,
                doc,
            });
        }
        self.preview_target = saved.target.map(|line| (saved.path.clone(), line));
        self.preview_path = Some(saved.path);
        self.text = saved.doc.plain_text();
        self.preview_doc = Some(saved.doc);
        self.preview_pinned = true;
    }
    fn close_preview(&mut self) {
        self.preview_pending = None;
        self.preview_request = self.preview_request.wrapping_add(1);
        self.preview_doc = None;
        self.preview_path = None;
        self.preview_target = None;
        self.text.clear();
        self.preview_pinned = false;
        self.switch_preview(false);
        if self.preview_doc.is_none() {
            self.focus = Focus::Sidebar;
        }
    }
    pub(crate) fn note_preview_click(&mut self) {
        let Some(path) = self.preview_path.clone() else {
            return;
        };
        let now = Instant::now();
        if self.preview_last_click.as_ref().is_some_and(|(old, at)| {
            old == &path && now.saturating_duration_since(*at) <= Duration::from_millis(400)
        }) {
            self.preview_pinned = true;
        }
        self.preview_last_click = Some((path, now));
    }
    pub(crate) fn preview_mouse(&mut self, event: &MouseEvent, area: Rect) -> bool {
        if self.editor.is_some() || !matches!(self.tab, Tab::Files | Tab::Search | Tab::Scm) {
            return false;
        }
        let content = self.panes(area).content;
        if !content.contains((event.column, event.row).into()) {
            return false;
        }
        let body = self.preview_body(content);
        self.focus = Focus::Content;
        if matches!(event.kind, MouseEventKind::Down(_)) {
            if self.search_prompt() {
                self.prompt = None;
                self.input.clear();
            }
            if self.tab == Tab::Scm {
                self.git.editing = None;
            }
        }
        let Some(doc) = self.readonly_doc_mut() else {
            return false;
        };
        match event.kind {
            MouseEventKind::ScrollDown => doc.set_scroll(
                doc.scroll()
                    .saturating_add(3)
                    .min(doc.row_count().saturating_sub(1)),
            ),
            MouseEventKind::ScrollUp => doc.set_scroll(doc.scroll().saturating_sub(3)),
            _ => {
                if doc.handle_mouse(event, body) {
                    self.copy_preview(true);
                }
            }
        }
        if let Some(doc) = &self.preview_doc {
            self.preview_scroll = doc.scroll().min(u16::MAX as usize) as u16;
        }
        true
    }
    fn copy_preview(&mut self, automatic: bool) {
        let Some(text) = self.readonly_doc().and_then(Doc::selection_text) else {
            self.status = "请先选择预览文本".into();
            return;
        };
        if self.preview_copier.is_none() {
            self.preview_copier = ReleaseCopier::product(self.preferences.copy_on_select);
        }
        if let Some(copier) = &self.preview_copier {
            copier.set_copy_on_select(self.preferences.copy_on_select);
            copier.copy(text, automatic);
        }
    }
    /// Central tick drains ordered clipboard results; rendering never writes stdout.
    pub(crate) fn tick_preview(&mut self) -> bool {
        let before = self.status.clone();
        if let Err(error) = self.dispatch_preview() {
            self.status = error.to_string();
        }
        if let Some(copier) = &self.preview_copier {
            while let Some(result) = copier.next_result() {
                self.status = match result {
                    ReleaseCopy::Disabled => continue,
                    ReleaseCopy::Copied => "已复制预览选区".into(),
                    ReleaseCopy::Failed(error) => format!("复制失败：{error}"),
                    ReleaseCopy::Osc52(text) => {
                        let result = herdr_work_code::actions::write_osc52_stdout(&text);
                        copier.acknowledge_terminal();
                        match result {
                            Ok(_) => "已发送到终端剪贴板".into(),
                            Err(error) => format!("复制失败：{error}"),
                        }
                    }
                };
            }
        }
        self.status != before
    }
    pub(crate) fn clear_previews(&mut self) {
        self.preview_loading = None;
        self.preview_pending = None;
        self.preview_request = self.preview_request.wrapping_add(1);
        self.preview_doc = None;
        self.preview_documents.clear();
        self.preview_pinned = false;
        self.preview_last_click = None;
    }
}
