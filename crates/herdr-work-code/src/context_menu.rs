//! File-menu presentation and hit testing share the same visible viewport.
use crate::{actions::MenuEntry, ui::palette};
use ratatui::{
    Frame,
    layout::{Margin, Position, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Clear, List, ListItem},
};

#[derive(Clone, Copy, Default)]
pub struct Viewport {
    pub rect: Rect,
    pub offset: usize,
}

impl Viewport {
    pub fn item_at(self, x: u16, y: u16, entries: &[MenuEntry]) -> Option<usize> {
        let inner = self.rect.inner(Margin::new(1, 1));
        if !inner.contains(Position::new(x, y)) {
            return None;
        }
        let index = self.offset + usize::from(y - inner.y);
        matches!(entries.get(index), Some(MenuEntry::Action(..))).then_some(index)
    }
}

pub fn draw(
    frame: &mut Frame,
    anchor: Position,
    entries: &[MenuEntry],
    selected: usize,
    previous_offset: usize,
) -> Viewport {
    let area = frame.area();
    let label_width = entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Action(_, label) => Line::raw(*label).width(),
            MenuEntry::Separator => 0,
        })
        .max()
        .unwrap_or(0);
    let width = (label_width + 4).min(usize::from(area.width)) as u16;
    let height = (entries.len() + 2).min(usize::from(area.height)) as u16;
    let rect = Rect::new(
        anchor.x.clamp(area.x, area.right().saturating_sub(width)),
        anchor
            .y
            .saturating_add(1)
            .clamp(area.y, area.bottom().saturating_sub(height)),
        width,
        height,
    );
    let visible = usize::from(height.saturating_sub(2));
    let mut offset = previous_offset.min(entries.len().saturating_sub(visible));
    if selected < offset {
        offset = selected;
    } else if selected >= offset.saturating_add(visible) {
        offset = selected.saturating_add(1).saturating_sub(visible);
    }
    let colors = palette();
    let items = entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .map(|(index, entry)| match entry {
            MenuEntry::Separator => ListItem::new(Line::from(
                "─".repeat(usize::from(width.saturating_sub(2))).dim(),
            )),
            MenuEntry::Action(_, label) => {
                let item = ListItem::new(format!(" {label} "));
                if index == selected {
                    item.style(Style::default().bg(colors.button_bg).fg(colors.button_fg))
                } else {
                    item
                }
            }
        })
        .collect::<Vec<_>>();
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().dim());
    if offset > 0 {
        block = block.title(" ↑ ");
    }
    if offset + visible < entries.len() {
        block = block.title_bottom(" ↓ ");
    }
    frame.render_widget(Clear, rect);
    frame.render_widget(
        List::new(items)
            .style(Style::default().bg(colors.hover_bg))
            .block(block),
        rect,
    );
    Viewport { rect, offset }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{MenuAction, menu_entries};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn chinese_menu_fits_sidebar_and_clicks_track_scrolled_selection() {
        let entries = menu_entries(Some(false), true);
        for (width, height) in [(28, 32), (22, 9), (1, 1), (0, 0)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            for selected in 0..entries.len() {
                let mut viewport = Viewport::default();
                terminal
                    .draw(|f| {
                        viewport = draw(
                            f,
                            Position::new(u16::MAX, u16::MAX),
                            &entries,
                            selected,
                            viewport.offset,
                        )
                    })
                    .unwrap();
                assert!(viewport.rect.right() <= width && viewport.rect.bottom() <= height);
                let inner = viewport.rect.inner(Margin::new(1, 1));
                if inner.width > 0 && inner.height > 0 {
                    let row = inner.y + (selected - viewport.offset) as u16;
                    assert_eq!(
                        viewport.item_at(inner.x, row, &entries),
                        matches!(entries[selected], MenuEntry::Action(..)).then_some(selected)
                    );
                }
            }
        }
        let mut terminal = Terminal::new(TestBackend::new(28, 32)).unwrap();
        let selected = entries
            .iter()
            .position(|entry| matches!(entry, MenuEntry::Action(MenuAction::CopyPath, _)))
            .unwrap();
        terminal
            .draw(|frame| {
                draw(frame, Position::new(5, 4), &entries, selected, 0);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        let text = text.replace(' ', "");
        assert!(text.contains("复制相对路径") && text.contains("使用默认应用打开"));
        if let Some(directory) = std::env::var_os("HERDR_WORK_TEST_FRAMES") {
            std::fs::create_dir_all(&directory).unwrap();
            let cells = terminal.backend().buffer().content.iter().map(|cell|
                serde_json::json!({"text":cell.symbol(), "fg":format!("{:?}",cell.fg), "bg":format!("{:?}",cell.bg)})
            ).collect::<Vec<_>>();
            std::fs::write(
                Path::new(&directory).join("file-context-menu.json"),
                serde_json::to_vec(
                    &serde_json::json!({"width":28,"height":32,"cells":cells,"fixture":true}),
                )
                .unwrap(),
            )
            .unwrap();
        }
    }

    use std::path::Path;

    #[test]
    fn hovering_a_scrolled_menu_keeps_visible_rows_under_the_pointer() {
        let entries = menu_entries(Some(false), true);
        let mut terminal = Terminal::new(TestBackend::new(28, 9)).unwrap();
        let mut viewport = Viewport::default();
        terminal
            .draw(|f| viewport = draw(f, Position::default(), &entries, entries.len() - 1, 0))
            .unwrap();
        let previous = viewport;
        let selected = previous.offset + 1;
        terminal
            .draw(|f| viewport = draw(f, Position::default(), &entries, selected, previous.offset))
            .unwrap();
        assert_eq!(viewport.offset, previous.offset);
        assert_eq!(
            viewport.item_at(viewport.rect.x + 1, viewport.rect.y + 2, &entries),
            matches!(entries[selected], MenuEntry::Action(..)).then_some(selected)
        );
    }
}
