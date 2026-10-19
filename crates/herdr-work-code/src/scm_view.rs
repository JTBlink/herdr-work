//! Sidebar SCM hover geometry and row rendering, shared with the source UI.
use crate::{
    git::FileEntry,
    icons::{IconTheme, icon},
    scm_tree::DirEntry,
    ui::{
        chrome_button_style, icon_style as ui_icon_style, palette, status_color, truncate_to,
        within,
    },
};
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::ListItem,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangesHeaderAction {
    Discard,
    Stash,
    Stage,
}

impl ChangesHeaderAction {
    pub fn footer_hint(self) -> &'static str {
        match self {
            Self::Discard => "↶ Discard All Changes",
            Self::Stash => "⇩ Stash Changes",
            Self::Stage => "+ Stage All Changes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileHoverAction {
    Open,
    Discard,
    Stage,
    Unstage,
}

impl FileHoverAction {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Open => "↗",
            Self::Discard => "↶",
            Self::Stage => "+",
            Self::Unstage => "−",
        }
    }

    pub fn footer_hint(self) -> &'static str {
        match self {
            Self::Open => "↗ Open Changes",
            Self::Discard => "↶ Discard Changes",
            Self::Stage => "+ Stage Changes",
            Self::Unstage => "− Unstage Changes",
        }
    }
}

pub fn changes_header_action_zones(
    width: u16,
    count: usize,
) -> Option<[(ChangesHeaderAction, (u16, u16)); 3]> {
    const ACTION_WIDTH: u16 = 3;
    let left_width = Span::raw(" ▾ Changes").width() as u16;
    let badge_width = Span::raw(format!(" {count} ")).width() as u16;
    let actions_width = ACTION_WIDTH * 3;
    let reserved = badge_width + 1;
    if width < left_width + 1 + actions_width + reserved {
        return None;
    }
    let start = width - reserved - actions_width;
    Some([
        (ChangesHeaderAction::Discard, (start, start + ACTION_WIDTH)),
        (
            ChangesHeaderAction::Stash,
            (start + ACTION_WIDTH, start + ACTION_WIDTH * 2),
        ),
        (
            ChangesHeaderAction::Stage,
            (start + ACTION_WIDTH * 2, start + actions_width),
        ),
    ])
}

pub fn changes_header_action_at(x: u16, width: u16, count: usize) -> Option<ChangesHeaderAction> {
    changes_header_action_zones(width, count)?
        .into_iter()
        .find_map(|(action, zone)| within(x, zone).then_some(action))
}

pub fn file_hover_actions(staged: bool) -> &'static [FileHoverAction] {
    const STAGED: &[FileHoverAction] = &[FileHoverAction::Open, FileHoverAction::Unstage];
    const UNSTAGED: &[FileHoverAction] = &[
        FileHoverAction::Open,
        FileHoverAction::Discard,
        FileHoverAction::Stage,
    ];
    if staged { STAGED } else { UNSTAGED }
}

pub fn file_hover_action_start(width: u16, staged: bool) -> Option<u16> {
    const ACTION_WIDTH: u16 = 3;
    const MIN_FILE_CONTENT: u16 = 8;
    let actions_width = file_hover_actions(staged).len() as u16 * ACTION_WIDTH;
    (width >= MIN_FILE_CONTENT + actions_width + 2).then(|| width - actions_width - 2)
}

pub fn file_hover_action_at(x: u16, width: u16, staged: bool) -> Option<FileHoverAction> {
    const ACTION_WIDTH: u16 = 3;
    let start = file_hover_action_start(width, staged)?;
    if x < start {
        return None;
    }
    let index = usize::from((x - start) / ACTION_WIDTH);
    file_hover_actions(staged).get(index).copied()
}

/// A collapsible section header; `count` renders as a right-aligned badge
/// (the drawers have no badge, like Git Graph's).
/// The visible two-column stage/unstage control immediately before a section badge.
pub fn section_action_zone(title: &str, count: usize, width: u16) -> Option<(u16, u16)> {
    let left = Span::raw(format!(" ▾ {title}")).width() as u16;
    let badge = Span::raw(format!(" {count} ")).width() as u16;
    if width < left + badge + 4 {
        return None;
    }
    let start = width - badge - 3;
    Some((start, start + 2))
}

pub fn section_item(
    title: &str,
    collapsed: bool,
    count: Option<usize>,
    width: usize,
    action: Option<char>,
) -> ListItem<'static> {
    let arrow = if collapsed { "▸" } else { "▾" };
    let left = Span::styled(format!(" {arrow} {title}"), Style::default().bold());
    let Some(count) = count else {
        return ListItem::new(Line::from(left));
    };
    let badge = Span::styled(
        format!(" {count} "),
        Style::default()
            .bg(palette().button_bg)
            .fg(palette().button_fg),
    );
    // Hovering shows the section-wide stage/unstage glyph before the badge.
    let action = action.filter(|_| section_action_zone(title, count, width as u16).is_some());
    let action_span = action.map(|a| Span::styled(format!("{a} "), Style::default().bold()));
    let aw = action_span.as_ref().map(Span::width).unwrap_or(0);
    let pad = width
        .saturating_sub(left.width() + badge.width() + 1 + aw)
        .max(1);
    let mut spans = vec![left, Span::raw(" ".repeat(pad))];
    if let Some(a) = action_span {
        spans.push(a);
    }
    spans.push(badge);
    spans.push(Span::raw(" "));
    ListItem::new(Line::from(spans))
}

pub fn changes_header_item(
    collapsed: bool,
    count: usize,
    width: usize,
    hovered: bool,
    hovered_action: Option<ChangesHeaderAction>,
) -> ListItem<'static> {
    let Some(actions) = hovered
        .then(|| changes_header_action_zones(width as u16, count))
        .flatten()
    else {
        return section_item("Changes", collapsed, Some(count), width, None);
    };
    let arrow = if collapsed { "▸" } else { "▾" };
    let left = Span::styled(format!(" {arrow} Changes"), Style::default().bold());
    let badge = Span::styled(
        format!(" {count} "),
        Style::default()
            .bg(palette().button_bg)
            .fg(palette().button_fg),
    );
    let actions_width = usize::from(actions[2].1.1 - actions[0].1.0);
    let pad = width
        .saturating_sub(left.width() + actions_width + badge.width() + 1)
        .max(1);
    let mut spans = vec![left, Span::raw(" ".repeat(pad))];
    for (action, _) in actions {
        let glyph = match action {
            ChangesHeaderAction::Discard => "↶",
            ChangesHeaderAction::Stash => "⇩",
            ChangesHeaderAction::Stage => "+",
        };
        spans.push(Span::styled(
            format!(" {glyph} "),
            chrome_button_style(hovered_action == Some(action)),
        ));
    }
    spans.push(badge);
    spans.push(Span::raw(" "));
    ListItem::new(Line::from(spans))
}

/// A file row: icon, name colored by status, dimmed parent directory, and a
/// right-aligned status letter — VS Code Source Control's row anatomy.
pub fn file_item(
    entry: &FileEntry,
    width: usize,
    theme: IconTheme,
    staged: bool,
    hovered: bool,
    hovered_action: Option<FileHoverAction>,
) -> ListItem<'static> {
    let (dir, name) = match entry.path.rsplit_once('/') {
        Some((dir, name)) => (Some(dir), name),
        None => (None, entry.path.as_str()),
    };
    let color = status_color(entry.letter);
    let file_icon = icon(theme, name, false, false);
    let icon_style = ui_icon_style(file_icon.rgb);
    let mut spans = vec![
        Span::raw("   "),
        Span::styled(format!("{} ", file_icon.glyph), icon_style),
    ];
    let actions = file_hover_actions(staged);
    let show_actions = hovered && file_hover_action_start(width as u16, staged).is_some();
    let actions_width = usize::from(show_actions) * actions.len() * 3;
    let tail = 2 + actions_width;
    let prefix_width: usize = spans.iter().map(Span::width).sum();
    let content_width = width.saturating_sub(prefix_width + tail);
    let visible_name = truncate_to(name.to_string(), content_width);
    spans.push(Span::styled(visible_name, Style::default().fg(color)));
    if let Some(dir) = dir {
        let sep = std::path::MAIN_SEPARATOR.to_string();
        let used: usize = spans.iter().map(Span::width).sum();
        let avail = width.saturating_sub(used + tail);
        let text = truncate_to(format!(" {}", dir.replace('/', &sep)), avail);
        if !text.is_empty() {
            spans.push(Span::styled(text, Style::default().dim()));
        }
    }
    let letter = Span::styled(entry.letter.to_string(), Style::default().fg(color).bold());
    let left_width: usize = spans.iter().map(Span::width).sum();
    let pad = width.saturating_sub(left_width + tail);
    spans.push(Span::raw(" ".repeat(pad)));
    if show_actions {
        for action in actions {
            spans.push(Span::styled(
                format!(" {} ", action.glyph()),
                chrome_button_style(hovered_action == Some(*action)),
            ));
        }
    }
    spans.push(letter);
    spans.push(Span::raw(" "));
    ListItem::new(Line::from(spans))
}

pub fn dir_hover_action_at(
    x: u16,
    width: u16,
    staged: bool,
    count: usize,
) -> Option<FileHoverAction> {
    if width < 10 {
        return None;
    }
    let badge_len = format!(" ({count}) ").len() as u16;
    let action_end = width.saturating_sub(badge_len);
    let action_start = action_end.saturating_sub(3);
    if x >= action_start && x < action_end {
        Some(if staged {
            FileHoverAction::Unstage
        } else {
            FileHoverAction::Stage
        })
    } else {
        None
    }
}

pub fn tree_file_item(
    entry: &FileEntry,
    depth: usize,
    width: usize,
    theme: IconTheme,
    staged: bool,
    hovered: bool,
    hovered_action: Option<FileHoverAction>,
) -> ListItem<'static> {
    let name = match entry.path.rsplit_once('/') {
        Some((_, name)) => name,
        None => entry.path.as_str(),
    };
    let color = status_color(entry.letter);
    let file_icon = icon(theme, name, false, false);
    let icon_style = ui_icon_style(file_icon.rgb);
    let indent = "  ".repeat(depth);
    let mut spans = vec![
        Span::raw(format!(" {indent}  ")),
        Span::styled(format!("{} ", file_icon.glyph), icon_style),
    ];
    let actions = file_hover_actions(staged);
    let show_actions = hovered && file_hover_action_start(width as u16, staged).is_some();
    let actions_width = usize::from(show_actions) * actions.len() * 3;
    let tail = 2 + actions_width;
    let prefix_width: usize = spans.iter().map(Span::width).sum();
    let content_width = width.saturating_sub(prefix_width + tail);
    let visible_name = truncate_to(name.to_string(), content_width);
    spans.push(Span::styled(visible_name, Style::default().fg(color)));
    let letter = Span::styled(entry.letter.to_string(), Style::default().fg(color).bold());
    let left_width: usize = spans.iter().map(Span::width).sum();
    let pad = width.saturating_sub(left_width + tail);
    spans.push(Span::raw(" ".repeat(pad)));
    if show_actions {
        for action in actions {
            spans.push(Span::styled(
                format!(" {} ", action.glyph()),
                chrome_button_style(hovered_action == Some(*action)),
            ));
        }
    }
    spans.push(letter);
    spans.push(Span::raw(" "));
    ListItem::new(Line::from(spans))
}

pub fn tree_dir_item(
    dir: &DirEntry,
    width: usize,
    theme: IconTheme,
    staged: bool,
    hovered: bool,
    hovered_action: Option<FileHoverAction>,
) -> ListItem<'static> {
    let indent = "  ".repeat(dir.depth);
    let arrow = if dir.expanded { "▾ " } else { "▸ " };
    let folder_icon = icon(theme, &dir.name, true, dir.expanded);
    let icon_style = ui_icon_style(folder_icon.rgb);
    let mut spans = vec![
        Span::styled(format!(" {indent}{arrow}"), Style::default().dim()),
        Span::styled(format!("{} ", folder_icon.glyph), icon_style),
    ];
    let action = if staged {
        FileHoverAction::Unstage
    } else {
        FileHoverAction::Stage
    };
    let show_action = hovered && width >= 10;
    let action_width = if show_action { 3 } else { 0 };
    let badge_str = format!(" ({})", dir.count);
    let tail = 1 + action_width + badge_str.len();
    let prefix_width: usize = spans.iter().map(Span::width).sum();
    let content_width = width.saturating_sub(prefix_width + tail);
    let visible_name = truncate_to(dir.name.clone(), content_width);
    spans.push(Span::styled(visible_name, Style::default().bold()));
    let left_width: usize = spans.iter().map(Span::width).sum();
    let pad = width.saturating_sub(left_width + tail);
    spans.push(Span::raw(" ".repeat(pad)));
    if show_action {
        spans.push(Span::styled(
            format!(" {} ", action.glyph()),
            chrome_button_style(hovered_action == Some(action)),
        ));
    }
    spans.push(Span::styled(badge_str, Style::default().dim()));
    spans.push(Span::raw(" "));
    ListItem::new(Line::from(spans))
}

pub const REPO_HEADER_ACTIONS: &str = " ⇅  ✓ ";

pub fn repo_header_action_zones(width: u16) -> ((u16, u16), (u16, u16)) {
    let actions_width = Span::raw(REPO_HEADER_ACTIONS).width() as u16;
    if width < actions_width {
        return ((0, 0), (0, 0));
    }
    let start = width - actions_width;
    let midpoint = start + actions_width / 2;
    ((start, midpoint), (midpoint, width))
}
