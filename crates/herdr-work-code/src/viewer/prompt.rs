//! Editor confirmations own the whole pane and consume all pointer input.
use super::{Pending, Prompt};
use crate::ui::{chrome_button_style, palette};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Clear, Paragraph},
};

fn choices(prompt: &Prompt) -> [(&'static str, KeyCode); 3] {
    match prompt {
        Prompt::Unsaved(Pending::Close) => [
            ("[s] 保存并关闭", KeyCode::Char('s')),
            ("[d] 不保存并关闭", KeyCode::Char('d')),
            ("[Esc] 取消", KeyCode::Esc),
        ],
        Prompt::Unsaved(_) => [
            ("[s] 保存并继续", KeyCode::Char('s')),
            ("[d] 不保存并继续", KeyCode::Char('d')),
            ("[Esc] 取消", KeyCode::Esc),
        ],
        Prompt::Conflict(_) => [
            ("[o] 覆盖磁盘版本", KeyCode::Char('o')),
            ("[r] 重新加载磁盘版本", KeyCode::Char('r')),
            ("[Esc] 取消", KeyCode::Esc),
        ],
    }
}

fn close_rect(area: Rect) -> Rect {
    Rect::new(
        area.right().saturating_sub(4).max(area.x),
        area.y,
        area.width.min(3),
        area.height.min(1),
    )
}

fn button_rects(area: Rect) -> [Rect; 3] {
    let width = area.width.saturating_sub(4).min(44);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let top = area.y + area.height.saturating_sub(3) / 2;
    std::array::from_fn(|n| {
        let y = top + n as u16;
        Rect::new(x, y, width, u16::from(y < area.bottom().saturating_sub(1)))
    })
}

pub(super) fn draw(frame: &mut Frame, prompt: &Prompt, selected: usize, error: Option<&str>) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    let title = match prompt {
        Prompt::Unsaved(_) => " 保存修改？ ",
        Prompt::Conflict(_) => " 文件已在磁盘上修改 ",
    };
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(palette().header_accent)),
        area,
    );
    frame.render_widget(
        Paragraph::new(" × ").style(chrome_button_style(false)),
        close_rect(area),
    );
    let buttons = button_rects(area);
    if buttons[0].y > area.y + 2 {
        frame.render_widget(
            Paragraph::new(match prompt {
                Prompt::Unsaved(_) => "文件有未保存的修改，是否保存？",
                Prompt::Conflict(_) => "请选择如何处理磁盘上的新版本。",
            })
            .centered(),
            Rect::new(
                area.x + 1,
                buttons[0].y - 2,
                area.width.saturating_sub(2),
                1,
            ),
        );
    }
    for (n, ((label, _), rect)) in choices(prompt).into_iter().zip(buttons).enumerate() {
        let style = if n == selected {
            Style::default()
                .fg(palette().button_fg)
                .bg(palette().button_focus_bg)
                .bold()
        } else {
            Style::default()
                .fg(palette().muted_button_fg)
                .bg(palette().muted_button_bg)
        };
        frame.render_widget(Paragraph::new(label).centered().style(style), rect);
    }
    if let Some(error) = error
        && area.height >= 10
    {
        frame.render_widget(
            Paragraph::new(error)
                .centered()
                .style(Style::default().fg(palette().conflict)),
            Rect::new(
                area.x + 1,
                area.bottom() - 3,
                area.width.saturating_sub(2),
                1,
            ),
        );
    }
}

/// Convert a modal button to its existing keyboard action; swallow everything else.
pub(super) fn event(prompt: &Prompt, event: Event, area: Rect, selected: &mut usize) -> Event {
    let code = match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Tab | KeyCode::Down | KeyCode::Right => {
                *selected = (*selected + 1) % 3;
                None
            }
            KeyCode::BackTab | KeyCode::Up | KeyCode::Left => {
                *selected = (*selected + 2) % 3;
                None
            }
            KeyCode::Enter => Some(choices(prompt)[*selected].1),
            _ => return Event::Key(key),
        },
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
            let point = (mouse.column, mouse.row).into();
            if close_rect(area).contains(point) {
                Some(KeyCode::Esc)
            } else {
                button_rects(area)
                    .iter()
                    .position(|r| r.contains(point))
                    .map(|n| choices(prompt)[n].1)
            }
        }
        _ => None,
    };
    code.map(|code| Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
        .unwrap_or(Event::FocusGained)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_fills_the_pane_and_close_cancels_without_editing() {
        let prompt = Prompt::Unsaved(Pending::Close);
        for (width, height) in [(80, 24), (28, 10)] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| draw(frame, &prompt, 0, None))
                .unwrap();
            if width == 80
                && let Some(output) = std::env::var_os("HERDR_WORK_TEST_FRAMES")
            {
                std::fs::create_dir_all(&output).unwrap();
                let cells = terminal.backend().buffer().content.iter().map(|cell| serde_json::json!({
                    "text":cell.symbol(), "fg":format!("{:?}",cell.fg), "bg":format!("{:?}",cell.bg)
                })).collect::<Vec<_>>();
                std::fs::write(std::path::Path::new(&output).join("editor-save-confirmation.json"),
                    serde_json::to_vec(&serde_json::json!({"width":width,"height":height,"cells":cells,"fixture":true})).unwrap()).unwrap();
            }
            let area = Rect::new(0, 0, width, height);
            assert_eq!(terminal.backend().buffer()[(width - 3, 0)].symbol(), "×");
            let click = Event::Mouse(crossterm::event::MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: width - 3,
                row: 0,
                modifiers: KeyModifiers::NONE,
            });
            assert!(matches!(
                event(&prompt, click, area, &mut 0),
                Event::Key(KeyEvent {
                    code: KeyCode::Esc,
                    ..
                })
            ));
            let paste = Event::Paste("do not edit".into());
            assert!(matches!(
                event(&prompt, paste, area, &mut 0),
                Event::FocusGained
            ));
        }
    }
}
