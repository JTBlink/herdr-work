//! Shared source footer rendering, attached to the active product repository.
use crate::state::*;
use anyhow::Result;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use herdr_work_code::{branch_ui, git::Status, ui};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::Paragraph,
};

impl App {
    pub(crate) fn draw_footer(&mut self, frame: &mut Frame, area: Rect, hints: &str) {
        self.interaction.footer = Default::default();
        self.interaction.footer_root = None;
        let project = ui::truncate_to(
            self.root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            16,
        );
        let prefix = format!("{project} ");
        let width = ratatui::text::Span::raw(prefix.as_str())
            .width()
            .min(usize::from(area.width)) as u16;
        frame.render_widget(Paragraph::new(prefix), Rect::new(area.x, area.y, width, 1));
        let mut left = area.x + width;
        if self.preferences.show_git_footer {
            let root = self.selected_repository();
            if let Some(repository) = self
                .code
                .as_ref()
                .and_then(|code| code.repositories.iter().find(|repo| repo.root == root))
            {
                let status = Status {
                    branch: repository.branch.clone(),
                    ahead: repository.ahead,
                    behind: repository.behind,
                    has_upstream: repository.has_upstream,
                    ..Default::default()
                };
                // Keep room for status even when a branch has a long Unicode name.
                let available = area.width.saturating_sub(width).saturating_sub(12).min(34);
                let zones = branch_ui::draw_git_footer(
                    frame,
                    Rect::new(left, area.y, available, 1),
                    self.icon_theme(),
                    &status,
                    false,
                    self.interaction.pointer.map(|point| (point.x, point.y)),
                );
                left = zones.sync.right();
                self.interaction.footer = zones;
                self.interaction.footer_root = Some(root);
            }
        }
        let status = if self.status.is_empty() {
            self.load_summary()
        } else {
            self.status.clone()
        };
        let notice = if self
            .editor
            .as_ref()
            .is_some_and(|session| session.editor.external_changed)
        {
            "磁盘已改变，保留编辑内容 · "
        } else {
            ""
        };
        let status = status.lines().collect::<Vec<_>>().join(" · ");
        frame.render_widget(
            Paragraph::new(format!(" | {notice}{status}")),
            Rect::new(left, area.y, area.right().saturating_sub(left), 1),
        );
        if self.preferences.show_hotkeys && area.height > 1 {
            frame.render_widget(
                Paragraph::new(format!("{hints} · r 刷新 · q 退出")),
                Rect::new(area.x, area.y + 1, area.width, 1),
            );
        }
    }
    pub(crate) fn footer_mouse(&mut self, event: &MouseEvent) -> Result<bool> {
        if !self.preferences.show_git_footer
            || event.kind != MouseEventKind::Down(MouseButton::Left)
            || self.interaction.footer_root.as_ref() != Some(&self.selected_repository())
        {
            return Ok(false);
        }
        let point = Position::new(event.column, event.row);
        let branch = self.interaction.footer.branch.contains(point);
        let sync = self.interaction.footer.sync.contains(point);
        if !branch && !sync {
            return Ok(false);
        }
        if self.editor.is_some() || self.edit_request.is_some() || self.git.editing.is_some() {
            self.status = "请先完成当前编辑，再操作 Git 分支或同步".into();
        } else if branch {
            self.open_branches()?;
        } else {
            self.work_git(herdr_work_engine::WorkGitAction::Sync)?;
        }
        Ok(true)
    }
}
