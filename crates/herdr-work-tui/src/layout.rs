//! The same pane geometry drives drawing and pointer hit testing.
use crate::state::{Focus, Tab};
use ratatui::layout::{Constraint, Layout, Rect};

pub(crate) const ACTIVITIES: [(Tab, &str, char); 4] = [
    (Tab::Files, "Files", '2'),
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
pub(crate) fn panes(area: Rect, focus: Focus) -> Panes {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .areas(area);
    let [activity, workspace] =
        Layout::horizontal([Constraint::Length(9), Constraint::Min(1)]).areas(body);
    let (sidebar, content) = if area.width >= 70 {
        let width = (workspace.width / 3).clamp(24, 42);
        let [sidebar, content] =
            Layout::horizontal([Constraint::Length(width), Constraint::Min(1)]).areas(workspace);
        (sidebar, content)
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
pub(crate) fn offset(selected: usize, height: u16) -> usize {
    selected.saturating_sub(height.saturating_sub(3) as usize)
}
