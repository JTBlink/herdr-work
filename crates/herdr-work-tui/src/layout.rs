//! The same pane geometry drives drawing and pointer hit testing.
use crate::state::{Focus, Tab};
use ratatui::layout::{Constraint, Layout, Rect};

pub(crate) const ACTIVITY_WIDTH: u16 = 6;
pub(crate) const ACTIVITY_HEIGHT: u16 = 3;
pub(crate) const ACTIVITY_GLYPH_WIDTH: u16 = 2;

pub(crate) fn activity_glyph(rect: Rect) -> Rect {
    let width = rect.width.min(ACTIVITY_GLYPH_WIDTH);
    Rect::new(
        rect.x + rect.width.saturating_sub(width) / 2,
        rect.y + rect.height / 2,
        width,
        rect.height.min(1),
    )
}

pub(crate) fn activity_badge(rect: Rect, width: u16, dock_right: bool) -> Rect {
    let glyph = activity_glyph(rect);
    let right = rect.right().saturating_sub(u16::from(dock_right));
    let width = width.min(right.saturating_sub(rect.x + 1));
    let x = glyph
        .right()
        .saturating_sub(1)
        .min(right.saturating_sub(width))
        .max(rect.x);
    Rect::new(
        x,
        glyph.bottom().min(rect.bottom().saturating_sub(1)),
        width,
        rect.height.min(1),
    )
}

pub(crate) const ACTIVITIES: [(Tab, &str, char); 5] = [
    (Tab::Files, "文件", '2'),
    (Tab::Search, "搜索", '5'),
    (Tab::Scm, "Git", '3'),
    (Tab::Tasks, "任务", '1'),
    (Tab::Review, "审查", '4'),
];
pub(crate) struct Panes {
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
    let [body, footer] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(if preferences.show_hotkeys { 2 } else { 1 }),
    ])
    .areas(area);
    let [first, second] = Layout::horizontal(if preferences.dock_right {
        [Constraint::Min(1), Constraint::Length(ACTIVITY_WIDTH)]
    } else {
        [Constraint::Length(ACTIVITY_WIDTH), Constraint::Min(1)]
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
        activity,
        sidebar,
        content,
        footer,
    }
}
pub(crate) fn settings_button(activity: Rect) -> Rect {
    if horizontal_activity(activity) {
        return Rect::new(
            activity
                .right()
                .saturating_sub(ACTIVITY_WIDTH)
                .max(activity.x),
            activity.y,
            activity.width.min(ACTIVITY_WIDTH),
            activity.height,
        );
    }
    Rect::new(
        activity.x,
        activity
            .bottom()
            .saturating_sub(ACTIVITY_HEIGHT)
            .max(activity.y),
        activity.width,
        activity.height.min(ACTIVITY_HEIGHT),
    )
}
pub(crate) fn activity_button(activity: Rect, index: usize) -> Rect {
    if horizontal_activity(activity) {
        let x = activity
            .x
            .saturating_add(index as u16 * ACTIVITY_WIDTH)
            .min(activity.right());
        return Rect::new(
            x,
            activity.y,
            settings_button(activity)
                .x
                .saturating_sub(x)
                .min(ACTIVITY_WIDTH),
            activity.height,
        );
    }
    let y = activity.y.saturating_add(index as u16 * ACTIVITY_HEIGHT);
    Rect::new(
        activity.x,
        y,
        activity.width,
        settings_button(activity)
            .y
            .saturating_sub(y)
            .min(ACTIVITY_HEIGHT),
    )
}
impl crate::state::App {
    pub(crate) fn panes(&self, area: Rect) -> Panes {
        if self.sidebar_mode {
            let [activity, body, footer] = Layout::vertical([
                Constraint::Length(ACTIVITY_HEIGHT),
                Constraint::Min(0),
                Constraint::Length(if self.preferences.show_hotkeys { 2 } else { 1 }),
            ])
            .areas(area);
            let content_active = self.focus == Focus::Content
                || (self.tab == Tab::Scm && self.active_drawer.is_some());
            return Panes {
                activity,
                sidebar: if content_active {
                    Rect::default()
                } else {
                    body
                },
                content: if content_active {
                    body
                } else {
                    Rect::default()
                },
                footer,
            };
        }
        configured_panes(area, self.focus, &self.preferences)
    }
}

pub(crate) fn horizontal_activity(activity: Rect) -> bool {
    activity.height <= ACTIVITY_HEIGHT && activity.width > ACTIVITY_WIDTH
}
/// First visible item, given the actual number of body rows (no panel chrome).
pub(crate) fn offset(selected: usize, height: u16) -> usize {
    selected.saturating_sub(height.saturating_sub(1) as usize)
}
pub(crate) fn task_offset(selected: usize, height: u16) -> usize {
    offset(selected, height / 2)
}
