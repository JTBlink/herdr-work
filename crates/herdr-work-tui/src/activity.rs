//! Compact navigation using the shared Codicon family and palette.
use crate::{
    layout::{
        ACTIVITIES, activity_badge, activity_button, activity_glyph, horizontal_activity,
        settings_button,
    },
    state::{App, Tab},
};
use herdr_work_code::ui::{self, ActivityIcon, palette};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::Line,
    widgets::{Clear, Paragraph},
};

impl App {
    pub(crate) fn draw_activity_rail(&self, frame: &mut Frame, area: Rect) {
        for (index, (tab, _, _)) in ACTIVITIES.iter().enumerate() {
            let rect = activity_button(area, index);
            let icon = match tab {
                Tab::Files => ActivityIcon::Files,
                Tab::Search => ActivityIcon::Search,
                Tab::Scm => ActivityIcon::SourceControl,
                Tab::Tasks => ActivityIcon::Tasks,
                Tab::Review => ActivityIcon::Review,
            };
            self.draw_rail_button(frame, rect, icon, self.tab == *tab);
            if *tab == Tab::Scm && rect.height > 0 {
                let count = self.code.as_ref().map_or(0, |code| code.changes.len());
                if count > 0 {
                    let badge = if count > 99 {
                        "99+".into()
                    } else {
                        count.to_string()
                    };
                    let width = (badge.len() as u16).min(rect.width.saturating_sub(1));
                    frame.render_widget(
                        Paragraph::new(badge).style(
                            Style::default()
                                .fg(palette().button_fg)
                                .bg(palette().button_bg),
                        ),
                        activity_badge(rect, width, self.preferences.dock_right),
                    );
                }
            }
        }
        self.draw_rail_button(
            frame,
            settings_button(area),
            ActivityIcon::Settings,
            matches!(self.overlay, Some(crate::menus::Overlay::Settings(_))),
        );
    }

    fn draw_rail_button(&self, frame: &mut Frame, rect: Rect, icon: ActivityIcon, active: bool) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let hovered = self
            .interaction
            .pointer
            .is_some_and(|point| rect.contains(point));
        let style = if active {
            Style::default()
                .fg(palette().keycap_fg)
                .bg(palette().selection_unfocused_bg)
        } else {
            ui::activity_button_style(false, hovered)
        };
        frame.render_widget(Paragraph::new("").style(style), rect);
        // Fixed two-cell glyph slot for both Material and Emoji themes.
        let glyph = ui::activity_icon(self.icon_theme(), icon);
        frame.render_widget(
            Paragraph::new(glyph)
                .alignment(Alignment::Center)
                .style(style),
            activity_glyph(rect),
        );
        if active && !self.sidebar_mode {
            let x = if self.preferences.dock_right {
                rect.right() - 1
            } else {
                rect.x
            };
            frame.render_widget(
                Paragraph::new(vec![Line::from("▎"); usize::from(rect.height)])
                    .style(Style::default().fg(palette().accent_focus)),
                Rect::new(x, rect.y, 1, rect.height),
            );
        }
    }

    pub(crate) fn draw_activity_tooltip(&self, frame: &mut Frame, rail: Rect) {
        if self.overlay.is_some() || self.prompt.is_some() || self.git.picker.is_some() {
            return;
        }
        let Some(point) = self.interaction.pointer else {
            return;
        };
        let (rect, label, key) = if settings_button(rail).contains(point) {
            (settings_button(rail), "设置", ',')
        } else if let Some((index, (_, label, key))) = ACTIVITIES
            .iter()
            .enumerate()
            .find(|(index, _)| activity_button(rail, *index).contains(point))
        {
            (activity_button(rail, index), *label, *key)
        } else {
            return;
        };
        let line = Line::from(format!(" {label} · {key} "));
        let horizontal = horizontal_activity(rail);
        let width = (line.width() as u16).min(if horizontal {
            frame.area().width
        } else {
            frame.area().width.saturating_sub(rail.width)
        });
        if width == 0 {
            return;
        }
        let x = if horizontal {
            rect.x.min(frame.area().right().saturating_sub(width))
        } else if self.preferences.dock_right {
            rail.x.saturating_sub(width)
        } else {
            rail.right()
        };
        let tip = Rect::new(
            x,
            if horizontal {
                rail.bottom().min(frame.area().bottom().saturating_sub(1))
            } else {
                rect.y + rect.height / 2
            },
            width,
            1,
        );
        frame.render_widget(Clear, tip);
        frame.render_widget(
            Paragraph::new(line).style(
                Style::default()
                    .fg(palette().keycap_fg)
                    .bg(palette().keycap_bg),
            ),
            tip,
        );
    }
}
