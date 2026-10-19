//! Original Explorer row rendering shared by the reference UI and HerdrWork.
use crate::{
    icons::{IconTheme, icon},
    tree::Row,
    ui::{hover_style, icon_style as ui_icon_style, selection_style, status_color, truncate_to},
};
use ratatui::{
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::ListItem,
};

/// The right-aligned decoration for a status letter (issue #19): the letter
/// itself for a file, a filled dot for a directory whose DESCENDANTS changed
/// (VS Code's dirty-folder badge), and nothing at all for ignored paths —
/// those are conveyed by the dimmed name instead.
pub fn deco_marker(letter: char, is_dir: bool) -> Option<String> {
    match letter {
        'I' => None,
        _ if is_dir => Some("●".to_string()),
        c => Some(c.to_string()),
    }
}

/// The name's style for a decoration. Decorations are FOREGROUND-only on
/// purpose: selection and hover own the background, so a decorated row stays
/// readable when it is also the selected one.
pub fn deco_name_style(letter: Option<char>) -> Style {
    match letter {
        Some('I') => Style::default().dim(),
        Some(c) => Style::default().fg(status_color(c)),
        None => Style::default(),
    }
}

pub fn row_item(
    row: &Row,
    theme: IconTheme,
    hovered: bool,
    selected: bool,
    deco: Option<char>,
    width: u16,
) -> ListItem<'static> {
    let item = ListItem::new(row_line(row, theme, deco, width));
    match row_bg(hovered, selected) {
        Some(style) => item.style(style),
        None => item,
    }
}

/// The selection / hover BACKGROUND for a row, if any. Kept apart from the
/// content so a git decoration (foreground-only) can never collide with it.
pub fn row_bg(hovered: bool, selected: bool) -> Option<Style> {
    if selected {
        Some(selection_style(true))
    } else if hovered {
        // Subtler than the selection bg — hover is a hint, not a choice.
        Some(hover_style())
    } else {
        None
    }
}

/// One tree row's content: indent, chevron, icon, name, and the right-aligned
/// git decoration.
pub fn row_line(row: &Row, theme: IconTheme, deco: Option<char>, width: u16) -> Line<'static> {
    let indent = "  ".repeat(row.depth);
    let arrow = if row.is_dir {
        if row.expanded { "▾ " } else { "▸ " }
    } else {
        "  "
    };
    let icon = icon(theme, &row.name, row.is_dir, row.expanded);
    let icon_style = ui_icon_style(icon.rgb);
    // Folder and file names share the default foreground, like VS Code — the
    // chevron and icon carry the distinction. Accent-on-gray (the old blue
    // names) was hard to read against the selection/hover backgrounds. A git
    // status recolors the name on top of that.
    let mut spans = vec![
        Span::styled(format!("{indent}{arrow}"), Style::default().dim()),
        Span::styled(format!("{} ", icon.glyph), icon_style),
    ];
    let marker = deco.and_then(|letter| deco_marker(letter, row.is_dir && !row.is_symlink));
    // Row anatomy with a marker: [prefix][name][pad][marker][2 trailing]. The
    // two trailing cells keep the marker clear of the overflow scrollbar,
    // which overdraws the very last column; the name yields the 4 cells that
    // leaves (gap + marker + 2), so a narrow pane ellipsizes the NAME instead
    // of losing the status.
    let tail = if marker.is_some() { 4 } else { 0 };
    let used: usize = spans.iter().map(Span::width).sum();
    let avail = usize::from(width).saturating_sub(used + tail);
    let name = truncate_to(row.name.clone(), avail);
    let name_width = Span::raw(name.as_str()).width();
    spans.push(Span::styled(name, deco_name_style(deco)));
    if let Some(marker) = marker {
        let pad = usize::from(width).saturating_sub(used + name_width + 3);
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled(
            marker,
            Style::default()
                .fg(status_color(deco.unwrap_or(' ')))
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
    }
    Line::from(spans)
}
