//! File deletion confirmation. Painting and hit-testing share the last layout;
//! the Explorer owns the actual filesystem operation.
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use herdr_work_code::{settings_view, ui::palette, wrap::wrap_line};
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Response {
    None,
    Cancel,
    Confirm,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Choice {
    #[default]
    Cancel,
    Delete,
}

#[derive(Default)]
struct Geometry {
    popup: Rect,
    content: Rect,
    cancel: Rect,
    delete: Rect,
    can_confirm: bool,
}

pub(super) struct DeleteConfirmation {
    path: PathBuf,
    is_dir: bool,
    display_path: String,
    selected: Choice,
    hovered: Option<Choice>,
    error: Option<String>,
    geometry: Geometry,
}

impl DeleteConfirmation {
    pub(super) fn new(path: PathBuf, is_dir: bool, root: &Path) -> Self {
        let display_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        Self {
            path,
            is_dir,
            display_path,
            selected: Choice::Cancel,
            hovered: None,
            error: None,
            geometry: Geometry::default(),
        }
    }

    pub(super) fn target(&self) -> (&Path, bool) {
        (&self.path, self.is_dir)
    }

    pub(super) fn fail(&mut self, error: String) {
        self.error = Some(format!("删除失败：{error}"));
        self.selected = Choice::Cancel;
        self.hovered = None;
        self.geometry = Geometry::default();
    }

    pub(super) fn key(&mut self, key: KeyEvent) -> Response {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return Response::None;
        }
        self.hovered = None;
        match key.code {
            KeyCode::Esc | KeyCode::Char('n' | 'N') => Response::Cancel,
            KeyCode::Tab
            | KeyCode::BackTab
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down => {
                self.selected = if self.selected == Choice::Cancel {
                    Choice::Delete
                } else {
                    Choice::Cancel
                };
                Response::None
            }
            KeyCode::Char('y' | 'Y') if self.geometry.can_confirm => Response::Confirm,
            KeyCode::Enter | KeyCode::Char(' ') => match self.selected {
                Choice::Cancel => Response::Cancel,
                Choice::Delete if self.geometry.can_confirm => Response::Confirm,
                _ => Response::None,
            },
            _ => Response::None,
        }
    }

    pub(super) fn mouse(&mut self, mouse: MouseEvent) -> Response {
        let point = (mouse.column, mouse.row).into();
        let choice = if self.geometry.cancel.contains(point) {
            Some(Choice::Cancel)
        } else if self.geometry.delete.contains(point) {
            Some(Choice::Delete)
        } else {
            None
        };
        if mouse.kind == MouseEventKind::Moved {
            self.hovered = choice;
        }
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Response::None;
        }
        if settings_view::close_button(self.geometry.popup).contains(point) {
            return Response::Cancel;
        }
        match choice {
            Some(Choice::Cancel) => Response::Cancel,
            Some(Choice::Delete) if self.geometry.can_confirm => Response::Confirm,
            _ => Response::None,
        }
    }

    fn body(&self, width: usize) -> Vec<Line<'static>> {
        let mut body = Vec::new();
        let mut push = |text: String, style: Style| {
            body.extend(wrap_line(&Line::styled(text, style), width));
        };
        push(
            if self.is_dir {
                "将永久删除文件夹及其全部内容："
            } else {
                "将永久删除文件："
            }
            .into(),
            Style::default(),
        );
        push(
            self.display_path.clone(),
            Style::default().fg(palette().modified).bold(),
        );
        push(
            "删除后无法恢复。".into(),
            Style::default().fg(palette().deleted),
        );
        if let Some(error) = &self.error {
            push(error.clone(), Style::default().fg(palette().conflict));
        }
        body
    }

    fn button_style(&self, choice: Choice, enabled: bool) -> Style {
        let focused = self.selected == choice;
        let active = focused || self.hovered == Some(choice);
        let style = if choice == Choice::Delete {
            Style::default()
                .fg(palette().deleted)
                .bg(palette().diff_del_bg)
        } else if active {
            Style::default()
                .fg(palette().button_fg)
                .bg(palette().button_focus_bg)
        } else {
            Style::default()
                .fg(palette().muted_button_fg)
                .bg(palette().muted_button_bg)
        };
        let style = if focused {
            style.bold().underlined()
        } else if active {
            style.bold()
        } else {
            style
        };
        if enabled { style } else { style.dim() }
    }

    pub(super) fn draw(&mut self, frame: &mut Frame) {
        let body = self.body(content_width(frame.area()));
        let geometry = geometry(frame.area(), body.len());
        frame.render_widget(Clear, geometry.popup);
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title(if self.is_dir {
                    " 删除文件夹？ "
                } else {
                    " 删除文件？ "
                })
                .style(
                    Style::default()
                        .bg(palette().keycap_bg)
                        .fg(palette().keycap_fg),
                )
                .border_style(Style::default().fg(palette().header_accent)),
            geometry.popup,
        );
        settings_view::draw_close(frame, geometry.popup);
        if !geometry.can_confirm {
            frame.render_widget(
                Paragraph::new("请放大窗口以查看完整确认信息")
                    .wrap(ratatui::widgets::Wrap { trim: false }),
                geometry.content,
            );
        } else {
            frame.render_widget(Paragraph::new(body), geometry.content);
        }
        for (choice, label, rect) in [
            (Choice::Cancel, "取消", geometry.cancel),
            (Choice::Delete, "确认删除", geometry.delete),
        ] {
            let enabled = choice == Choice::Cancel || geometry.can_confirm;
            frame.render_widget(
                Paragraph::new(label)
                    .centered()
                    .style(self.button_style(choice, enabled)),
                rect,
            );
        }
        self.geometry = geometry;
    }
}

fn content_width(area: Rect) -> usize {
    usize::from(area.width.min(60).saturating_sub(4)).max(1)
}

fn geometry(area: Rect, rows: usize) -> Geometry {
    let width = area.width.min(60);
    let stacked = content_width(area) < 22;
    let button_height = if stacked { 3 } else { 1 };
    let height = rows
        .saturating_add(button_height + 3)
        .min(usize::from(area.height)) as u16;
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let inner = popup.inner(Margin::new(2, 1));
    let button_height = button_height as u16;
    let can_place = inner.height >= button_height;
    let button_y = inner.bottom().saturating_sub(button_height);
    let content = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        button_y.saturating_sub(inner.y + 1),
    );
    let mut result = Geometry {
        popup,
        content,
        ..Geometry::default()
    };
    if can_place {
        result.cancel = Rect::new(inner.x, button_y, inner.width, 1);
        result.delete = if stacked {
            Rect::new(inner.x, button_y + 2, inner.width, 1)
        } else {
            let left = inner.width.saturating_sub(2) / 2;
            result.cancel.width = left;
            Rect::new(
                inner.x + left + 2,
                button_y,
                inner.width.saturating_sub(left + 2),
                1,
            )
        };
    }
    result.can_confirm = can_place && result.delete.width >= 8 && content.height as usize >= rows;
    result
}
