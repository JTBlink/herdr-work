//! The same pane geometry drives drawing and pointer hit testing.
use crate::state::{Focus, Tab};
use ratatui::layout::{Constraint, Layout, Rect};

pub(crate) const ACTIVITIES: [(Tab, &str, char); 5] = [
    (Tab::Files, "Files", '2'),
    (Tab::Search, "Search", '5'),
    (Tab::Scm, "Git", '3'),
    (Tab::Tasks, "Tasks", '1'),
    (Tab::Review, "Review", '4'),
];
pub(crate) struct Panes {
    pub header: Rect,
    pub activity: Rect,
    pub sidebar: Rect,
    pub content: Rect,
    pub footer: Rect,
}
#[cfg(test)]
pub(crate) fn panes(area: Rect, focus: Focus) -> Panes {
    configured_panes(area, focus, &herdr_work_engine::UiPreferences::default())
}
pub(crate) fn configured_panes(
    area: Rect,
    focus: Focus,
    preferences: &herdr_work_engine::UiPreferences,
) -> Panes {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(if preferences.show_hotkeys { 3 } else { 2 }),
    ])
    .areas(area);
    let [first, second] = Layout::horizontal(if preferences.dock_right {
        [Constraint::Min(1), Constraint::Length(6)]
    } else {
        [Constraint::Length(6), Constraint::Min(1)]
    })
    .areas(body);
    let (activity, workspace) = if preferences.dock_right {
        (second, first)
    } else {
        (first, second)
    };
    let (sidebar, content) = if area.width >= 70 {
        let width = preferences
            .sidebar_width
            .min(workspace.width.saturating_sub(24));
        let [first, second] = Layout::horizontal(if preferences.dock_right {
            [Constraint::Min(1), Constraint::Length(width)]
        } else {
            [Constraint::Length(width), Constraint::Min(1)]
        })
        .areas(workspace);
        if preferences.dock_right {
            (second, first)
        } else {
            (first, second)
        }
    } else if focus == Focus::Sidebar {
        (workspace, Rect::default())
    } else {
        (Rect::default(), workspace)
    };
    Panes {
        header,
        activity,
        sidebar,
        content,
        footer,
    }
}
pub(crate) fn settings_button(activity: Rect) -> Rect {
    Rect::new(
        activity.x,
        activity.bottom().saturating_sub(3).max(activity.y),
        activity.width,
        activity.height.min(3),
    )
}
impl crate::state::App {
    pub(crate) fn panes(&self, area: Rect) -> Panes {
        configured_panes(area, self.focus, &self.preferences)
    }
}
pub(crate) fn offset(selected: usize, height: u16) -> usize {
    selected.saturating_sub(height.saturating_sub(3) as usize)
}
