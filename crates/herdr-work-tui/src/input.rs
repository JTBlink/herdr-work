//! Recoverable App errors stay inside the sole input loop; terminal I/O stays outside.
use crate::state::App;
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind};
use ratatui::layout::Rect;

impl App {
    /// Only an explicit successful quit command ends the event loop.
    pub(crate) fn input_event(&mut self, event: Event, area: Rect) -> bool {
        let result = match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => self.input_key(key, area),
            Event::Paste(text) if self.overlay.is_none() => self.paste_input(&text).map(|()| false),
            Event::Mouse(mouse) => self.mouse(mouse, area).map(|()| false),
            _ => Ok(false),
        };
        match result {
            Ok(quit) => quit,
            Err(error) => {
                self.status = format!("操作未完成，输入已保留，可重试：{error:#}");
                false
            }
        }
    }
    fn paste_input(&mut self, text: &str) -> Result<()> {
        if self.prompt.is_some() {
            if self.input.len() + text.len() <= 1024 * 1024 {
                if self.search_prompt() {
                    return self.insert_search_text(text);
                }
                self.input.push_str(text);
                self.persist_commit_prompt()?;
                self.search_input_changed()?;
            } else {
                self.status = "粘贴内容过长".into();
            }
        } else if self.git.editing.is_some() {
            self.paste_commit(text)?;
        } else if let Some(session) = &mut self.editor {
            if text.len() + session.editor.text().len() <= 1024 * 1024 {
                session.editor.insert_text(text);
            } else {
                self.status = "粘贴后文本超过 1 MiB".into();
            }
        }
        Ok(())
    }
}
