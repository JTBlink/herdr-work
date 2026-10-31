//! Shared geometry for the one-shot browser action in an HTML source preview.
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

pub fn button(header: Rect, reserved_right: u16) -> Rect {
    if header.height == 0 || header.width < reserved_right + 22 {
        return Rect::default();
    }
    Rect::new(header.right() - reserved_right - 12, header.y, 12, 1)
}
pub fn draw(frame: &mut Frame, button: Rect) {
    if button.width > 0 {
        frame.render_widget(
            Paragraph::new(" 浏览器 B ").style(crate::ui::chrome_button_style(false)),
            button,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_action_keeps_title_find_and_close_controls_disjoint() {
        for reserved in [3, 18] {
            let header = Rect::new(37, 4, 80, 1);
            let action = button(header, reserved);
            assert!(action.x >= header.x + 10);
            assert_eq!(action.right(), header.right() - reserved);
            assert_eq!(button(Rect::new(37, 4, 20, 1), reserved).width, 0);
        }
    }
}
