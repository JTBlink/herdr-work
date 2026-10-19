//! Search controls extracted from Sidebar's `explorer_app::draw_content_search`.
//! Source: Sidebar 081ed133f3faa377438b51ce435fe8a87c439593 (MIT).
//! Keep the source layout and input presentation shared with the reference UI.
use crate::{
    icons::IconTheme,
    ui::{TitleAction, input_tail, palette, title_action_icon, truncate_to},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Block,
};
use unicode_width::UnicodeWidthChar;

#[derive(Default)]
pub struct SearchLayout {
    pub header: Rect,
    pub toolbar: [Rect; 3],
    pub query_box: Rect,
    pub query: Rect,
    pub options: [Rect; 3],
    pub replace_box: Rect,
    pub replace: Rect,
    pub preserve_case: Rect,
    pub replace_all: Rect,
    pub replace_toggle: Rect,
    pub collapse: Rect,
    pub include_label: Rect,
    pub include_box: Rect,
    pub include: Rect,
    pub exclude_label: Rect,
    pub exclude_box: Rect,
    pub exclude: Rect,
    pub body: Rect,
}
impl SearchLayout {
    /// Compact Search/Replace layout for the workbench, sharing Sidebar's
    /// input geometry and clipping with a VS Code style replacement row.
    pub fn vscode(area: Rect, details: bool, replace: bool) -> Self {
        let mut layout = Self::new(area, details, replace);
        for rect in [
            &mut layout.query_box,
            &mut layout.query,
            &mut layout.replace_box,
            &mut layout.include_label,
            &mut layout.include_box,
            &mut layout.include,
            &mut layout.exclude_label,
            &mut layout.exclude_box,
            &mut layout.exclude,
            &mut layout.body,
        ]
        .into_iter()
        .chain(layout.options.iter_mut())
        {
            if !rect.is_empty() {
                rect.y = rect.y.saturating_sub(1).max(area.y);
            }
        }
        layout.body.height = area.bottom().saturating_sub(layout.body.y);
        layout.header.height = layout.header.height.saturating_sub(1);
        if area.width >= 22 {
            for (index, rect) in layout.toolbar.iter_mut().enumerate() {
                *rect = Rect::new(
                    area.right() - 12 + index as u16 * 3,
                    area.y,
                    3,
                    area.height.min(1),
                );
            }
            layout.collapse = Rect::new(area.right() - 3, area.y, 3, area.height.min(1));
        }
        let toggle_y = layout.query_box.y + 1;
        if toggle_y < area.bottom() && area.width >= 3 {
            layout.replace_toggle = Rect::new(area.x, toggle_y, 2, 1);
        }
        if !layout.replace_box.is_empty()
            && layout.replace_box.width >= 10
            && layout.replace_box.height >= 3
        {
            layout.replace_all = Rect::new(
                layout.replace_box.right() - 3,
                layout.replace_box.y + 1,
                3,
                1,
            );
            layout.replace_box.width = layout.replace_box.width.saturating_sub(4);
            let inner = Block::bordered().inner(layout.replace_box);
            layout.preserve_case = Rect::new(inner.right() - 3, inner.y, 3, inner.height);
            layout.replace = Rect::new(
                inner.x,
                inner.y,
                inner.width.saturating_sub(3),
                inner.height,
            );
        }
        layout
    }
    /// `area` includes the title row. Replacement is optional for consumers
    /// which do not expose Sidebar's replacement workflow.
    pub fn new(area: Rect, details: bool, replace: bool) -> Self {
        let rect = |x: u16, y: u16, width: u16, height: u16| {
            Rect::new(
                area.x.saturating_add(x).min(area.right()),
                area.y.saturating_add(y).min(area.bottom()),
                area.width.saturating_sub(x).min(width),
                area.height.saturating_sub(y).min(height),
            )
        };
        let field = |y| rect(2, y, area.width.saturating_sub(3), 3);
        let query_box = field(2);
        let inner = Block::bordered().inner(query_box);
        // Source options use three columns each; hide them only when the query
        // would otherwise have no room. The key bindings remain available.
        let option_width = if inner.width >= 13 { 9 } else { 0 };
        let query = Rect::new(
            inner.x,
            inner.y,
            inner.width.saturating_sub(option_width),
            inner.height,
        );
        let mut options = [Rect::default(); 3];
        if option_width > 0 {
            for (i, option) in options.iter_mut().enumerate() {
                *option = Rect::new(query.right() + i as u16 * 3, inner.y, 3, inner.height);
            }
        }
        let replace_box = if replace { field(5) } else { Rect::default() };
        let filters_y = if replace { 9 } else { 6 };
        let include_label = if details {
            rect(2, filters_y, area.width.saturating_sub(3), 1)
        } else {
            Rect::default()
        };
        let include_box = if details {
            field(filters_y + 1)
        } else {
            Rect::default()
        };
        let exclude_label = if details {
            rect(2, filters_y + 4, area.width.saturating_sub(3), 1)
        } else {
            Rect::default()
        };
        let exclude_box = if details {
            field(filters_y + 5)
        } else {
            Rect::default()
        };
        let body_y = if details { filters_y + 9 } else { filters_y };
        let toolbar_x = area.width.saturating_sub(9);
        let toolbar = if area.width >= 18 {
            [
                rect(toolbar_x, 0, 3, 1),
                rect(toolbar_x + 3, 0, 3, 1),
                rect(toolbar_x + 6, 0, 3, 1),
            ]
        } else {
            [Rect::default(); 3]
        };
        Self {
            header: rect(0, 0, area.width, body_y),
            toolbar,
            query_box,
            query,
            options,
            replace_box,
            include_label,
            include_box,
            include: Block::bordered().inner(include_box),
            exclude_label,
            exclude_box,
            exclude: Block::bordered().inner(exclude_box),
            body: rect(0, body_y, area.width, area.height.saturating_sub(body_y)),
            ..Default::default()
        }
    }
}

// Mechanically moved from explorer_app; the source tests remain there.
pub fn search_input_line(
    value: &str,
    placeholder: &str,
    focused: bool,
    width: u16,
) -> Line<'static> {
    let available = usize::from(width).saturating_sub(usize::from(focused));
    let mut spans = if value.is_empty() && !focused {
        vec![Span::styled(
            truncate_to(placeholder.to_string(), available),
            Style::default().dim(),
        )]
    } else if value.is_empty() {
        Vec::new()
    } else {
        vec![Span::raw(input_tail(value, available))]
    };
    if focused {
        spans.push(Span::raw("█"));
    }
    Line::from(spans)
}

pub fn search_toolbar_icons(theme: IconTheme) -> (&'static str, &'static str, &'static str) {
    match theme {
        IconTheme::Material => (
            title_action_icon(theme, TitleAction::Refresh),
            "\u{eabf}",
            "\u{ea7c}",
        ),
        IconTheme::Emoji => (title_action_icon(theme, TitleAction::Refresh), "×", "⋯"),
    }
}

pub fn draw_field(frame: &mut Frame, area: Rect, focused: bool) {
    frame.render_widget(
        Block::bordered().border_style(if focused {
            Style::default().fg(palette().accent_focus)
        } else {
            Style::default().dim()
        }),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_spacing_keeps_boxed_filters_and_optional_replacement() {
        let area = Rect::new(5, 7, 40, 30);
        let layout = SearchLayout::new(area, true, true);
        assert_eq!(layout.query_box, Rect::new(7, 9, 37, 3));
        assert_eq!(layout.replace_box.y, 12);
        assert_eq!(layout.include_label.y, 16);
        assert_eq!(layout.include_box.y, 17);
        assert_eq!(layout.exclude_box.y, 21);
        assert_eq!(layout.body.y, 25);
        let no_replace = SearchLayout::new(area, true, false);
        assert_eq!(no_replace.include_box.y, layout.include_box.y - 3);
        assert_eq!(no_replace.body.y, layout.body.y - 3);
    }

    #[test]
    fn clipped_controls_cannot_receive_clicks_outside_the_sidebar() {
        for width in 0..45 {
            for height in 0..22 {
                let area = Rect::new(4, 3, width, height);
                let layout = SearchLayout::new(area, true, false);
                for rect in [
                    layout.header,
                    layout.query_box,
                    layout.query,
                    layout.include_label,
                    layout.include_box,
                    layout.include,
                    layout.exclude_label,
                    layout.exclude_box,
                    layout.exclude,
                    layout.body,
                ]
                .into_iter()
                .chain(layout.options)
                .chain(layout.toolbar)
                {
                    if !rect.is_empty() {
                        assert!(area.contains(rect.as_position()), "{area:?} {rect:?}");
                        assert!(rect.right() <= area.right() && rect.bottom() <= area.bottom());
                    }
                }
            }
        }
    }
}

// Moved from Sidebar explorer_app; original context/Unicode tests stay at the caller.
pub fn highlighted_search_context(
    context: &str,
    matches: &[(usize, usize)],
    max_width: usize,
    match_style: Style,
) -> Vec<Span<'static>> {
    let context_width = Span::raw(context).width();
    let (visible_end, truncated) = if context_width <= max_width {
        (context.len(), false)
    } else if max_width < 2 {
        (0, false)
    } else {
        let mut width = 0;
        let mut end = 0;
        for (index, character) in context.char_indices() {
            let character_width = character.width().unwrap_or(0);
            if width + character_width + 1 > max_width {
                break;
            }
            width += character_width;
            end = index + character.len_utf8();
        }
        (end, true)
    };
    let mut spans = Vec::new();
    let mut cursor = 0;
    for &(start, end) in matches {
        if start >= visible_end {
            break;
        }
        let start = start.max(cursor);
        let end = end.min(visible_end);
        if start >= end {
            continue;
        }
        if start > cursor {
            spans.push(Span::raw(context[cursor..start].to_string()));
        }
        spans.push(Span::styled(context[start..end].to_string(), match_style));
        cursor = end;
    }
    if cursor < visible_end {
        spans.push(Span::raw(context[cursor..visible_end].to_string()));
    }
    if truncated {
        spans.push(Span::raw("…"));
    }
    spans
}

pub fn truncate_path_tail(label: &str, max: usize) -> String {
    let width = Span::raw(label).width();
    if width <= max {
        return label.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut used = 1;
    let mut reversed = Vec::new();
    for ch in label.chars().rev() {
        let char_width = ch.width().unwrap_or(0);
        if used + char_width > max {
            break;
        }
        used += char_width;
        reversed.push(ch);
    }
    let tail = reversed.into_iter().rev().collect::<String>();
    format!("…{tail}")
}

#[cfg(test)]
mod vscode_layout_tests {
    use super::*;
    #[test]
    fn every_visible_control_remains_inside_small_viewports() {
        for width in 0..45 {
            for height in 0..30 {
                for details in [false, true] {
                    for replace in [false, true] {
                        let area = Rect::new(4, 3, width, height);
                        let layout = SearchLayout::vscode(area, details, replace);
                        for rect in [
                            layout.query_box,
                            layout.query,
                            layout.replace_box,
                            layout.replace,
                            layout.preserve_case,
                            layout.replace_all,
                            layout.replace_toggle,
                            layout.collapse,
                            layout.include_box,
                            layout.exclude_box,
                            layout.body,
                        ]
                        .into_iter()
                        .chain(layout.toolbar)
                        .chain(layout.options)
                        {
                            if !rect.is_empty() {
                                assert!(
                                    area.contains(rect.as_position())
                                        && rect.right() <= area.right()
                                        && rect.bottom() <= area.bottom(),
                                    "{area:?} {rect:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
