//! Product sidebar controls. Drawing and pointer routing share these rectangles.
use crate::{
    icons::IconTheme,
    ui::{self, ActivityIcon},
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::{Clear, Paragraph},
};

// The visible rail is one column even when Herdr keeps a four-column PTY.
// Always draw and hit-test its first cell; the host clips the remaining cells.
pub(crate) const RAIL_WIDTH: u16 = 1;

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
    // Reserve the final two rows even in a short terminal.
    let body_height = area.height.saturating_sub(2);
    let actions = [Action::Files, Action::Search, Action::Git, Action::Settings];
    let slot = if body_height >= 15 { 3 } else { 1 };
    for (index, action) in actions.into_iter().enumerate() {
        let row = index as u16 * slot;
        if row < body_height {
            buttons.push((
                Rect::new(
                    rail.x,
                    rail.y + 1 + row,
                    rail.width,
                    slot.min(body_height - row),
                ),
                action,
            ));
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
        let icon = match action {
            Action::Files => activity_icon(theme, ActivityIcon::Files, collapsed),
            Action::Search => activity_icon(theme, ActivityIcon::Search, collapsed),
            Action::Git => activity_icon(theme, ActivityIcon::SourceControl, collapsed),
            Action::Settings => activity_icon(theme, ActivityIcon::Settings, collapsed),
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
        frame.render_widget(Paragraph::new(icon).style(style), glyph);
    }
}

// Nerd Font glyphs can paint across two cells despite reporting width one.
// The narrow rail always uses text symbols; expanded views keep their theme.
fn activity_icon(theme: IconTheme, icon: ActivityIcon, collapsed: bool) -> &'static str {
    if !collapsed {
        return ui::activity_icon(theme, icon);
    }
    match icon {
        ActivityIcon::Files => "▣",
        ActivityIcon::Search => "⌕",
        ActivityIcon::SourceControl => "⑂",
        ActivityIcon::Settings => "⚙︎",
        ActivityIcon::Review => "◉",
    }
}
