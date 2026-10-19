//! Explorer and source-control rendering reuses Sidebar's status and focus palette.
use crate::{layout::offset, loading::LoadState, state::*};
use herdr_sidebar::ui::{palette, selection_style, status_color};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

impl App {
    pub(crate) fn sidebar_block(&self, title: String) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(if self.focus == Focus::Sidebar {
                palette().accent_focus
            } else {
                palette().ignored
            }))
    }
    pub(crate) fn draw_sidebar(&self, f: &mut Frame, area: Rect) {
        if area.width == 0 {
            return;
        }
        if self.tab == Tab::Tasks {
            self.draw_tasks(f, area);
            return;
        }
        let scm = matches!(self.tab, Tab::Scm | Tab::Review);
        let branch = self.code.as_ref().map(|c| c.branch.as_str()).unwrap_or("");
        let title = if scm {
            format!("SOURCE CONTROL · {branch}")
        } else {
            "EXPLORER".into()
        };
        let block = self.sidebar_block(title);
        let code = match (&self.code_state, &self.code) {
            (LoadState::Ready, Some(code)) => code,
            (LoadState::Failed(error), _) => {
                f.render_widget(
                    Paragraph::new(format!("加载失败：{error}\n按 r 重试")).block(block),
                    area,
                );
                return;
            }
            _ => {
                f.render_widget(Paragraph::new("正在加载文件/Git…").block(block), area);
                return;
            }
        };
        if scm {
            let rows = self.scm_rows();
            let selected = if self.tab == Tab::Review {
                self.scm_selection
            } else {
                self.selection
            };
            let row = rows.iter().position(|r| r.0 == Some(selected)).unwrap_or(0);
            let items = rows
                .into_iter()
                .skip(offset(row, area.height))
                .map(|(index, text, status)| {
                    let style = if index == Some(selected) {
                        selection_style(self.focus == Focus::Sidebar)
                    } else if index.is_none() {
                        Style::default()
                            .fg(palette().header_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(status_color(status))
                    };
                    ListItem::new(text).style(style)
                })
                .collect::<Vec<_>>();
            f.render_widget(List::new(items).block(block), area);
        } else {
            let root_name = self
                .root
                .file_name()
                .map(|p| p.to_string_lossy())
                .unwrap_or_default();
            let block = block.title_bottom(format!("{root_name} · {branch}"));
            let items = code
                .files
                .iter()
                .enumerate()
                .skip(offset(self.selection, area.height))
                .map(|(i, row)| {
                    let status = code
                        .changes
                        .iter()
                        .find(|c| {
                            let path = c.root.join(&c.path);
                            if row.directory {
                                path.starts_with(&row.path)
                            } else {
                                path == row.path
                            }
                        })
                        .map(|c| if c.status == '?' { 'U' } else { c.status });
                    let folder = if row.directory {
                        if self.expanded.contains(&row.path) {
                            "▾ "
                        } else {
                            "▸ "
                        }
                    } else {
                        "  "
                    };
                    let name = format!("{}{}{}", "  ".repeat(row.depth), folder, row.name);
                    let line = Line::from(vec![
                        Span::raw(name),
                        Span::styled(
                            status.map(|s| format!(" {s}")).unwrap_or_default(),
                            Style::default().fg(status.map(status_color).unwrap_or_default()),
                        ),
                    ]);
                    let item = ListItem::new(line);
                    if i == self.selection {
                        item.style(selection_style(self.focus == Focus::Sidebar))
                    } else {
                        item
                    }
                })
                .collect::<Vec<_>>();
            f.render_widget(List::new(items).block(block), area);
        }
    }
    pub(crate) fn scm_rows(&self) -> Vec<(Option<usize>, String, char)> {
        let mut rows = vec![];
        if let Some(code) = &self.code {
            for (staged, title) in [(true, "Staged"), (false, "Changes")] {
                let count = code.changes.iter().filter(|c| c.staged == staged).count();
                rows.push((None, format!("{title} ({count})"), ' '));
                for (index, change) in code
                    .changes
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.staged == staged)
                {
                    let status = if change.status == '?' {
                        'U'
                    } else {
                        change.status
                    };
                    rows.push((Some(index), format!(" {status} {}", change.path), status));
                }
            }
        }
        rows
    }
}
