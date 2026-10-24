//! Responsive find bar. Painting and pointer dispatch share the same rectangles.
use super::*;
use crate::ui::chrome_button_style;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout};

impl Find {
    pub(in crate::viewer) fn layout(&self, area: Rect) -> [Rect; 4] {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(self.height(area.width)),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area)
    }

    pub(in crate::viewer) fn hint(&self, width: u16) -> String {
        use unicode_width::UnicodeWidthStr;
        let mut hint = String::from(" Esc 收起查找");
        for part in [
            "Enter/Shift+Enter 上下",
            "Tab 切换控件",
            "Aa 大小写",
            "ab 全词",
            ".* 正则",
        ] {
            if hint.width() + part.width() + 2 <= usize::from(width) {
                hint.push_str("  ");
                hint.push_str(part);
            }
        }
        hint
    }

    pub(super) fn controls(&self, area: Rect) -> [Rect; 8] {
        let width = area.width.min(78);
        let x = area.right().saturating_sub(width);
        let compact = area.width < 58;
        let input = Rect::new(
            x,
            area.y,
            width.saturating_sub(if compact { 3 } else { 33 }),
            1.min(area.height),
        );
        let y = area.y + u16::from(compact);
        let start = if compact { x } else { input.right() };
        let rect = |offset: u16, size: u16| {
            Rect::new(
                start.saturating_add(offset),
                y,
                size.min(area.right().saturating_sub(start.saturating_add(offset))),
                u16::from(y < area.bottom()),
            )
        };
        let count_width = if compact {
            width.saturating_sub(18).min(12)
        } else {
            12
        };
        [
            input,
            rect(0, 4),
            rect(4, 4),
            rect(8, 4),
            rect(12 + count_width, 3),
            rect(15 + count_width, 3),
            if compact {
                Rect::new(
                    area.right().saturating_sub(3),
                    area.y,
                    area.width.min(3),
                    1.min(area.height),
                )
            } else {
                rect(30, 3)
            },
            rect(12, count_width),
        ]
    }

    pub(in crate::viewer) fn mouse(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        header: Rect,
        doc: &Doc,
    ) -> bool {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return false;
        }
        let at = ratatui::layout::Position::new(mouse.column, mouse.row);
        if !doc.is_media() && Self::entry(header).contains(at) {
            self.show();
            return true;
        }
        if !self.open {
            return false;
        }
        if area.contains(at) {
            if let Some(target) = self.controls(area)[..7]
                .iter()
                .position(|rect| rect.contains(at))
            {
                self.focused = true;
                self.focus = target;
                if target == 0 {
                    self.cursor = self.query.chars().count();
                    self.select_all = false;
                } else {
                    self.activate(target);
                }
            }
            return true;
        }
        self.focused = false;
        false
    }

    pub(in crate::viewer) fn draw(&self, frame: &mut Frame, area: Rect) {
        let rects = self.controls(area);
        let input = rects[0];
        let width = usize::from(input.width.saturating_sub(2));
        let (shown, caret) = self.input_view(width);
        let text = if self.query.is_empty() {
            "查找".into()
        } else {
            shown
        };
        let input_style = if self.select_all && self.focused && self.focus == 0 {
            Style::default().bg(palette().text_selection_bg)
        } else {
            chrome_button_style(self.focused && self.focus == 0)
        };
        frame.render_widget(Paragraph::new(format!(" {text}")).style(input_style), input);
        let enabled = [
            self.options.match_case,
            self.options.whole_word,
            self.options.regex,
            false,
            false,
            false,
        ];
        for (index, label) in [" Aa ", " ab ", " .* ", " ↑ ", " ↓ ", " × "]
            .iter()
            .enumerate()
        {
            let target = index + 1;
            let style = chrome_button_style(self.focused && self.focus == target);
            let style = if enabled[index] {
                style.fg(palette().accent).bold().underlined()
            } else {
                style
            };
            frame.render_widget(Paragraph::new(*label).style(style), rects[target]);
        }
        let count = if self.error {
            "正则有误".into()
        } else if self.query.is_empty() {
            "".into()
        } else if self.hits.is_empty() {
            "无结果".into()
        } else {
            format!("{}/{}", self.active.map_or(0, |at| at + 1), self.hits.len())
        };
        frame.render_widget(
            Paragraph::new(count).style(if self.error {
                Style::default().fg(palette().conflict)
            } else {
                Style::default().dim()
            }),
            rects[7],
        );
        if self.focused && self.focus == 0 && input.width > 2 && input.height > 0 {
            frame.set_cursor_position((
                input.x + 1 + (caret as u16).min(input.width.saturating_sub(2)),
                input.y,
            ));
        }
    }

    pub(super) fn input_view(&self, width: usize) -> (String, usize) {
        use unicode_width::UnicodeWidthChar;
        let chars: Vec<char> = self.query.chars().collect();
        let mut start = self.cursor;
        let mut caret = 0;
        while start > 0 {
            let size = chars[start - 1].width().unwrap_or(0);
            if caret + size >= width {
                break;
            }
            caret += size;
            start -= 1;
        }
        let mut used = 0;
        let shown = chars[start..]
            .iter()
            .take_while(|c| {
                used += c.width().unwrap_or(0);
                used <= width
            })
            .collect();
        (shown, caret)
    }
}
