//! Product sidebar controls. Drawing and pointer routing share these rectangles.
use crate::{
    icons::IconTheme,
    ui::{self, ActivityIcon},
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Modifier,
    widgets::{Clear, Paragraph},
};

// An odd-width icon column centers single-cell controls with equal margins.
// The host may retain a wider PTY buffer; only this rail is visible.
pub(crate) const RAIL_WIDTH: u16 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Files,
    Search,
    Git,
    Settings,
    Expand,
    Collapse,
    Close,
}

pub(crate) fn controls(area: Rect, collapsed: bool) -> Vec<(Rect, Action)> {
    if area.is_empty() {
        return vec![];
    }
    if !collapsed {
        return vec![(ui::collapse_button_area(area), Action::Collapse)];
    }
    let rail = Rect::new(area.x, area.y, area.width.min(RAIL_WIDTH), area.height);
    let mut buttons = vec![(Rect::new(rail.x, rail.y, rail.width, 1), Action::Expand)];
    // Keep close on the last row; short terminals use compact activity rows.
    let body_end = area.height.saturating_sub(1);
    let actions = [Action::Files, Action::Search, Action::Git, Action::Settings];
    let step = if area.height >= 11 { 2 } else { 1 };
    let first = step;
    for (index, action) in actions.into_iter().enumerate() {
        let row = first + index as u16 * step;
        if row < body_end {
            buttons.push((Rect::new(rail.x, rail.y + row, rail.width, 1), action));
        }
    }
    if area.height >= 2 {
        buttons.push((
            Rect::new(rail.x, area.bottom() - 1, rail.width, 1),
            Action::Close,
        ));
    }
    buttons
}

pub(crate) fn hit(area: Rect, collapsed: bool, point: Position) -> Option<Action> {
    controls(area, collapsed)
        .into_iter()
        .find(|(rect, _)| rect.contains(point))
        .map(|(_, action)| action)
}

pub(crate) fn draw(
    frame: &mut Frame,
    collapsed: bool,
    active: Action,
    theme: IconTheme,
    pointer: Option<Position>,
) {
    let area = frame.area();
    if collapsed {
        frame.render_widget(Clear, area);
    }
    for (rect, action) in controls(area, collapsed) {
        let hovered = pointer.is_some_and(|p| rect.contains(p));
        let style = ui::activity_button_style(action == active, hovered);
        let style = if collapsed {
            style
                .fg(ui::palette().keycap_fg)
                .remove_modifier(Modifier::DIM)
                .add_modifier(Modifier::BOLD)
        } else {
            style
        };
        let icon = match action {
            Action::Files => ui::activity_icon(theme, ActivityIcon::Files),
            Action::Search => ui::activity_icon(theme, ActivityIcon::Search),
            Action::Git => ui::activity_icon(theme, ActivityIcon::SourceControl),
            Action::Settings => ui::activity_icon(theme, ActivityIcon::Settings),
            Action::Expand => "»",
            Action::Close => "×",
            Action::Collapse => "«",
        };
        frame.render_widget(Clear, rect);
        frame.render_widget(Paragraph::new("").style(style), rect);
        let glyph = if collapsed {
            Rect::new(rect.x, rect.y + rect.height / 2, rect.width, 1)
        } else {
            Rect::new(rect.x + rect.width / 2, rect.y, 1, 1)
        };
        frame.render_widget(Paragraph::new(icon).style(style).centered(), glyph);
    }
}
