//! Explorer and source-control rendering reuses Sidebar's status and focus palette.
use crate::{
    layout::offset,
    loading::LoadState,
    scm::{ScmRow, ScmTarget},
    state::*,
};
use herdr_work_code::ui::{palette, selection_style, status_color};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

impl App {
    pub(crate) fn sidebar_block(&self, title: String) -> Block<'static> {
        Block::default()
            .borders(if self.preferences.dock_right {
                Borders::LEFT
            } else {
                Borders::RIGHT
            })
            .title(title)
            .title_style(Style::default().fg(palette().header_accent))
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
        if self.tab == Tab::Search {
            self.draw_search_sidebar(f, area);
            return;
        }
        if self.tab == Tab::Tasks {
            self.draw_tasks(f, area);
            return;
        }
        let scm = matches!(self.tab, Tab::Scm | Tab::Review);
        let title = if scm {
            "SOURCE CONTROL".into()
        } else {
            "EXPLORER".into()
        };
        let block = self
            .sidebar_block(title)
            .title_top(Line::from(" ⋯ ").alignment(ratatui::layout::Alignment::Right));
        let body = block.inner(area);
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

            let row = self.scm_cursor.min(rows.len().saturating_sub(1));
            let items = rows
                .into_iter()
                .skip(offset(row, body.height))
                .enumerate()
                .map(|(position, (_index, text, status, target))| {
                    let style = if matches!(target, ScmTarget::Commit) {
                        Style::default()
                            .bg(palette().button_bg)
                            .fg(palette().button_fg)
                    } else if position + offset(row, body.height) == row {
                        selection_style(self.focus == Focus::Sidebar)
                    } else if matches!(target, ScmTarget::Section | ScmTarget::Drawer(_)) {
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
            let items = code
                .files
                .iter()
                .enumerate()
                .skip(offset(self.selection, body.height))
                .map(|(i, row)| {
                    let status = if self.preferences.git_decorations {
                        row.status
                    } else {
                        None
                    };
                    let row = herdr_work_code::tree::Row {
                        path: row.path.clone(),
                        name: row.name.clone(),
                        depth: row.depth,
                        is_dir: row.directory,
                        is_symlink: row.symlink,
                        expanded: self.expanded.contains(&row.path),
                    };
                    let item = ListItem::new(herdr_work_code::file_view::row_line(
                        &row,
                        self.icon_theme(),
                        status,
                        body.width,
                    ));
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
    pub(crate) fn scm_rows(&self) -> Vec<ScmRow> {
        let mut rows = vec![(None, "✓ 提交暂存更改 [C]".into(), ' ', ScmTarget::Commit)];
        if let Some(code) = &self.code {
            for (staged, title) in [(true, "Staged"), (false, "Changes")] {
                let count = code.changes.iter().filter(|c| c.staged == staged).count();
                rows.push((None, format!("{title} ({count})"), ' ', ScmTarget::Section));
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
                    rows.push((
                        Some(index),
                        format!(" {status} {}", change.path),
                        status,
                        ScmTarget::Change(index),
                    ));
                }
            }
        }
        for kind in herdr_work_code::scm::Drawer::ALL {
            let open = self.drawer_open[kind.index()];
            rows.push((
                None,
                format!("{} {}", if open { "▾" } else { "▸" }, kind.title()),
                ' ',
                ScmTarget::Drawer(kind),
            ));
            if open && let Some(view) = self.drawers.get(&kind) {
                for (index, line) in view.lines.iter().enumerate() {
                    let line = match kind {
                        herdr_work_code::scm::Drawer::Worktrees => {
                            herdr_work_code::scm::pretty_worktree_line(line)
                        }
                        herdr_work_code::scm::Drawer::Remotes => {
                            herdr_work_code::scm::pretty_remote_line(line)
                        }
                        _ => line.clone(),
                    };
                    rows.push((
                        None,
                        format!("  {line}"),
                        ' ',
                        ScmTarget::History(kind, index),
                    ));
                }
            }
        }
        rows
    }
}
