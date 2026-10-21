//! Sidebar's original repository header, boxed commit input and button.
//! Both the reference UI and HerdrWork render through this module.
use crate::{
    git::Status,
    icons::{IconTheme, icon},
    scm_view::REPO_HEADER_ACTIONS,
    ui::{branch_icon, hover_style, palette, sparkle_icon, truncate_to},
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};

#[derive(Clone, Copy)]
pub struct RepositoryHeader<'a> {
    pub name: &'a str,
    pub status: &'a Status,
    pub collapsed: bool,
}
impl RepositoryHeader<'_> {
    fn branch_decor(self) -> String {
        let dirty = if self.status.staged.is_empty() && self.status.unstaged.is_empty() {
            ""
        } else {
            "*"
        };
        format!("{}{dirty}", self.status.branch)
    }
}
pub fn repo_header_item(
    repo: RepositoryHeader<'_>,
    active: bool,
    theme: IconTheme,
    width: usize,
    branch_hovered: bool,
) -> ListItem<'static> {
    let arrow = if repo.collapsed { "▸" } else { "▾" };
    let repo_icon = icon(theme, "", true, false);
    let name_style = if active {
        Style::default().bold()
    } else {
        Style::default().dim().bold()
    };
    let s = &repo.status;
    let counts = if s.ahead + s.behind > 0 {
        format!(" {}↑ {}↓", s.ahead, s.behind)
    } else {
        String::new()
    };
    let branch_text = format!("{} {}{}", branch_icon(theme), repo.branch_decor(), counts);
    let full_icons_width = Span::raw(REPO_HEADER_ACTIONS).width();
    let icons_width = if width >= full_icons_width {
        full_icons_width
    } else {
        0
    };
    let content_width = width.saturating_sub(icons_width);
    let branch_width = Span::raw(branch_text.as_str())
        .width()
        .min(content_width / 2);
    let left_width = content_width.saturating_sub(branch_width);
    let left = truncate_to(
        format!(" {arrow} {} {}", repo_icon.glyph, repo.name),
        left_width,
    );
    let branch = truncate_to(branch_text, branch_width);
    let left_pad = left_width.saturating_sub(Span::raw(left.as_str()).width());
    let branch_pad = branch_width.saturating_sub(Span::raw(branch.as_str()).width());
    ListItem::new(Line::from(vec![
        Span::styled(left, name_style),
        Span::raw(" ".repeat(left_pad + branch_pad)),
        Span::styled(
            branch,
            if branch_hovered {
                hover_style()
            } else {
                Style::default().dim()
            },
        ),
        Span::styled(
            truncate_to(REPO_HEADER_ACTIONS.to_string(), icons_width),
            Style::default().dim(),
        ),
    ]))
}

pub fn repo_header_branch_zone(
    repo: RepositoryHeader<'_>,
    theme: IconTheme,
    width: u16,
) -> (u16, u16) {
    let counts = if repo.status.ahead + repo.status.behind > 0 {
        format!(" {}↑ {}↓", repo.status.ahead, repo.status.behind)
    } else {
        String::new()
    };
    let full_icons_width = Span::raw(REPO_HEADER_ACTIONS).width();
    let icons_width = if usize::from(width) >= full_icons_width {
        full_icons_width
    } else {
        0
    };
    let content_width = usize::from(width).saturating_sub(icons_width);
    let branch_width = Span::raw(format!(
        "{} {}{}",
        branch_icon(theme),
        repo.branch_decor(),
        counts
    ))
    .width()
    .min(content_width / 2);
    let start = content_width.saturating_sub(branch_width) as u16;
    (start, start + branch_width as u16)
}

/// Columns the inline message box's input field spans (between the left
/// border and the ✧ button).
pub fn inline_field_width(pane_width: u16) -> u16 {
    pane_width.saturating_sub(2 + 3)
}

/// Width-aware commit placeholder: drop detail rather than clipping
/// mid-word when the pane is narrow.
pub fn message_placeholder(branch: &str, width: usize) -> String {
    for text in [
        format!("Message (⏎ to commit on \"{branch}\")"),
        "Message (⏎ to commit)".to_string(),
        "Message".to_string(),
    ] {
        if Span::raw(text.as_str()).width() <= width {
            return text;
        }
    }
    String::new()
}

/// Most content rows a message box shows before it scrolls instead.
pub const MESSAGE_MAX_ROWS: usize = 4;

/// Wrap `message` into `field`-wide rows plus the cursor's (row, col). The
/// cursor may sit one past the end, opening a fresh row when that lands on a
/// wrap boundary.
pub fn wrap_message(message: &[char], cursor: usize, field: usize) -> (Vec<String>, usize, usize) {
    use unicode_width::UnicodeWidthChar;
    let field = field.max(1);
    let cursor = cursor.min(message.len());
    let mut rows = vec![String::new()];
    let mut column = 0;
    let mut caret = (0, 0);
    for (index, ch) in message.iter().copied().enumerate() {
        let width = ch.width().unwrap_or(0);
        if ch != '\n' && column > 0 && column + width > field {
            rows.push(String::new());
            column = 0;
        }
        if index == cursor {
            caret = (rows.len() - 1, column);
        }
        if ch == '\n' {
            rows.push(String::new());
            column = 0;
        } else {
            rows.last_mut().unwrap().push(ch);
            column += width;
        }
    }
    if cursor == message.len() {
        if column >= field {
            rows.push(String::new());
            column = 0;
        }
        caret = (rows.len() - 1, column);
    }
    (rows, caret.0, caret.1)
}

/// The `(top, count)` window of wrapped rows to show: everything when it
/// fits, else the slice keeping the cursor visible (or the start, unfocused).
pub fn message_window(rows: usize, cursor_row: usize, focused: bool) -> (usize, usize) {
    let visible = rows.min(MESSAGE_MAX_ROWS);
    let top = if focused {
        (cursor_row + 1).saturating_sub(visible)
    } else {
        0
    };
    (top, visible)
}

/// A repo's inline message box, VS Code style: a bordered input that grows
/// with its wrapped message, with the ✧ suggest button at its right end.
pub fn message_box_item(
    message: &[char],
    cursor: usize,
    branch: &str,
    focused: bool,
    theme: IconTheme,
    width: usize,
) -> ListItem<'static> {
    let border = if focused {
        Style::default().fg(palette().accent)
    } else {
        Style::default().dim()
    };
    let horizontal = "─".repeat(width.saturating_sub(2));
    let field = usize::from(inline_field_width(width as u16));
    // The ✧ button owns a fixed 3-column tail; pad the glyph to that width so
    // its rendered width can't shove the closing border off the box corners.
    let suggest = sparkle_icon(theme);
    let suggest_tail = format!(
        "{suggest}{}",
        " ".repeat(3usize.saturating_sub(Span::raw(suggest).width()))
    );

    let mut lines = vec![Line::from(Span::styled(format!("┌{horizontal}┐"), border))];
    if message.is_empty() && !focused {
        let placeholder = message_placeholder(branch, field);
        let pad = field.saturating_sub(Span::raw(placeholder.as_str()).width());
        lines.push(Line::from(vec![
            Span::styled("│", border),
            Span::styled(placeholder, Style::default().dim().italic()),
            Span::raw(" ".repeat(pad)),
            Span::raw(suggest_tail.clone()),
            Span::styled("│", border),
        ]));
    } else {
        let (rows, cursor_row, _) = wrap_message(message, cursor, field);
        let (top, visible) = message_window(rows.len(), cursor_row, focused);
        for (i, row) in rows.iter().skip(top).take(visible).enumerate() {
            let pad = field.saturating_sub(Span::raw(row.as_str()).width());
            // The ✧ button owns the 3-column tail of the FIRST line only.
            let tail = if i == 0 {
                Span::raw(suggest_tail.clone())
            } else {
                Span::raw("   ".to_string())
            };
            lines.push(Line::from(vec![
                Span::styled("│", border),
                Span::raw(row.clone()),
                Span::raw(" ".repeat(pad)),
                tail,
                Span::styled("│", border),
            ]));
        }
    }
    lines.push(Line::from(Span::styled(format!("└{horizontal}┘"), border)));
    ListItem::new(lines)
}

/// Sidebar's single-line filled button, shared without commit/sync side effects.
pub fn commit_button_item(
    label: &str,
    active: bool,
    focused: bool,
    width: usize,
) -> ListItem<'static> {
    let (bg, fg) = match (active, focused) {
        (true, true) => (palette().button_focus_bg, palette().button_fg),
        (true, false) => (palette().button_bg, palette().button_fg),
        (false, _) => (palette().muted_button_bg, palette().muted_button_fg),
    };
    let button_width = width.saturating_sub(2);
    let body_width = button_width;
    let label = truncate_to(label.to_string(), body_width);
    let label_width = Span::raw(label.as_str()).width();
    let left_pad = body_width.saturating_sub(label_width) / 2;
    let right_pad = body_width.saturating_sub(left_pad + label_width);
    let mut style = Style::default().bg(bg).fg(fg);
    if focused {
        style = style.add_modifier(Modifier::BOLD);
    }
    ListItem::new(vec![Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{}{label}{}", " ".repeat(left_pad), " ".repeat(right_pad)),
            style,
        ),
        Span::raw(" "),
    ])])
}

/// Where the list body was drawn last frame, for mouse hit-testing.
#[derive(Clone, Copy, Default)]
pub struct BodyGeom {
    pub top: u16,
    pub height: u16,
    pub offset: usize,
}

pub fn row_hit_with_heights(
    body: BodyGeom,
    mouse_row: u16,
    rows: impl IntoIterator<Item = (usize, u16)>,
) -> Option<(usize, u16)> {
    if mouse_row < body.top || mouse_row >= body.top + body.height {
        return None;
    }
    let mut y = body.top;
    for (index, height) in rows {
        if mouse_row < y + height {
            return Some((index, mouse_row - y));
        }
        y += height;
    }
    None
}

/// Visible rows including a partially clipped last row. Same selection/scroll
/// rule as the source list; callers reuse it for drawing and pointer hit tests.
pub fn list_window(
    heights: &[u16],
    scroll: usize,
    selected: Option<usize>,
    height: u16,
) -> std::ops::Range<usize> {
    let h = usize::from(height).max(1);
    let mut start = scroll.min(heights.len().saturating_sub(1));
    if let Some(selected) = selected.filter(|index| *index < heights.len()) {
        if selected < start {
            start = selected;
        }
        while start < selected
            && heights[start..=selected]
                .iter()
                .map(|v| usize::from(*v))
                .sum::<usize>()
                > h
        {
            start += 1;
        }
    }
    let mut end = start;
    let mut used = 0;
    while end < heights.len() && used < h {
        used += usize::from(heights[end]);
        end += 1;
    }
    start..end
}

pub fn message_box_height(message: &[char], cursor: usize, width: u16) -> u16 {
    (2 + wrap_message(message, cursor, usize::from(inline_field_width(width)))
        .0
        .len()
        .min(MESSAGE_MAX_ROWS)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_wrap_uses_display_columns_for_chinese_and_pasted_newlines() {
        let message = "修复布局a\n保存".chars().collect::<Vec<_>>();
        let (rows, line, column) = wrap_message(&message, message.len(), 6);
        assert_eq!(rows, ["修复布", "局a", "保存"]);
        assert_eq!((line, column), (2, 4));
        assert!(rows.iter().all(|row| Span::raw(row).width() <= 6));
        let (_, line, column) = wrap_message(&message, 3, 6);
        assert_eq!((line, column), (1, 0));
        let ascii = "abcdef".chars().collect::<Vec<_>>();
        assert_eq!(
            wrap_message(&ascii, 6, 3),
            (vec!["abc".into(), "def".into(), "".into()], 2, 0)
        );
    }
}
