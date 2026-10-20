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
    widgets::{Block, Borders, List, Paragraph},
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
            Line::from("SOURCE CONTROL")
        } else {
            let (_, zones) = self.sidebar_title_actions(area);
            let inner = self.sidebar_block(String::new()).inner(area);
            let end = zones
                .first()
                .map_or(inner.right().saturating_sub(3), |z| z.0.x);
            let name = self
                .root
                .file_name()
                .unwrap_or(self.root.as_os_str())
                .to_string_lossy();
            Line::from(herdr_work_code::file_view::workspace_folder_span(
                &name,
                self.file_root_collapsed,
                usize::from(end.saturating_sub(inner.x)),
            ))
        };
        let block = self
            .sidebar_block(String::new())
            .title(title)
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
            let layout = self.scm_layout(body);
            let models = self.scm_hover_models();
            let selected = self.scm_cursor.min(layout.rows.len().saturating_sub(1));
            let hovered = self
                .hover
                .point
                .and_then(|point| layout.hit(body, point))
                .map(|hit| hit.0);
            let items = layout
                .visible
                .clone()
                .map(|index| {
                    let entry = &layout.rows[index];
                    let target = &entry.3;
                    let style = if matches!(target, ScmTarget::Commit(_)) {
                        Style::default() // Sidebar's widget owns its caps and button fill.
                    } else if index == selected {
                        selection_style(self.focus == Focus::Sidebar)
                    } else if matches!(
                        target,
                        ScmTarget::Repository(_) | ScmTarget::Section(..) | ScmTarget::Drawer(_)
                    ) {
                        Style::default()
                            .fg(palette().header_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        let style = Style::default().fg(status_color(entry.2));
                        if hovered == Some(index) {
                            style.patch(herdr_work_code::ui::hover_style())
                        } else {
                            style
                        }
                    };
                    self.scm_hover_item(
                        entry,
                        body.width,
                        hovered == Some(index),
                        self.hover
                            .point
                            .filter(|point| body.contains(*point))
                            .map(|point| point.x - body.x),
                        &models,
                    )
                    .style(style)
                })
                .collect::<Vec<_>>();
            f.render_widget(List::new(items).block(block), area);
            if let Some(root) = &self.git.editing
                && let Some(index) = layout.rows.iter().position(|row| {
                    matches!(row.3,ScmTarget::Message(i)
                    if code.repositories.get(i).is_some_and(|repo|&repo.root==root))
                })
                && let Some(rect) = layout.rect(body, index)
                && let Some(draft) = self.git.drafts.get(root)
            {
                let source = herdr_work_code::scm_widgets::wrap_message(
                    &draft.message,
                    draft.cursor,
                    usize::from(herdr_work_code::scm_widgets::inline_field_width(body.width)),
                );
                let (top, _) =
                    herdr_work_code::scm_widgets::message_window(source.0.len(), source.1, true);
                let cursor = ratatui::layout::Position::new(
                    rect.x + 1 + source.2 as u16,
                    rect.y + 1 + (source.1 - top) as u16,
                );
                if rect.contains(cursor) && cursor.y < rect.bottom().saturating_sub(1) {
                    f.set_cursor_position(cursor);
                }
            }
        } else {
            if self.file_root_collapsed {
                f.render_widget(block, area);
                self.draw_sidebar_actions(f, area);
                return;
            }
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
                    let item = herdr_work_code::file_view::row_item(
                        &row,
                        self.icon_theme(),
                        self.sidebar_hovered(body, i, self.selection),
                        false,
                        status,
                        body.width,
                    );
                    if i == self.selection {
                        item.style(selection_style(self.focus == Focus::Sidebar))
                    } else {
                        item
                    }
                })
                .collect::<Vec<_>>();
            f.render_widget(List::new(items).block(block), area);
            herdr_work_code::ui::draw_scrollbar(
                f,
                body,
                code.files.len(),
                usize::from(body.height),
                offset(self.selection, body.height),
            );
        }
        self.draw_sidebar_actions(f, area);
    }
    pub(crate) fn scm_rows(&self) -> Vec<ScmRow> {
        let mut rows = vec![];
        if let Some(code) = &self.code {
            for (repo_index, repo) in code.repositories.iter().enumerate() {
                let active = self
                    .git
                    .active
                    .as_ref()
                    .map_or(repo_index == 0, |r| r == &repo.root);
                let collapsed = self.git.repository_collapsed.contains(&repo.root);
                let name = repo
                    .root
                    .strip_prefix(&self.root)
                    .ok()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or_else(|| {
                        std::path::Path::new(repo.root.file_name().unwrap_or_default())
                    })
                    .display();
                rows.push((
                    None,
                    format!(
                        "{}{} {}{name} · {} ↑{} ↓{}{}",
                        if collapsed { "▸" } else { "▾" },
                        if active { "●" } else { "○" },
                        if repo.write_error.is_some() {
                            "⚠只读 "
                        } else {
                            ""
                        },
                        repo.branch,
                        repo.ahead,
                        repo.behind,
                        if repo.has_upstream {
                            ""
                        } else {
                            " · 无 upstream"
                        }
                    ),
                    ' ',
                    ScmTarget::Repository(repo_index),
                ));
                if collapsed {
                    continue;
                }
                let draft = self.git.drafts.get(&repo.root);
                let message = draft
                    .map(|d| {
                        if self.git.editing.as_ref() == Some(&repo.root) {
                            format!(
                                "{}▏{}",
                                d.message.iter().take(d.cursor).collect::<String>(),
                                d.message.iter().skip(d.cursor).collect::<String>()
                            )
                        } else {
                            d.message.iter().collect::<String>()
                        }
                    })
                    .unwrap_or_default()
                    .replace('\n', " ↵ ");
                let message = if message.is_empty() {
                    "提交消息…（Enter 编辑）".into()
                } else {
                    message
                };
                rows.push((
                    None,
                    format!(" ✎ {message}"),
                    ' ',
                    ScmTarget::Message(repo_index),
                ));
                rows.push((
                    None,
                    if self.git.committing.contains(&repo.root) {
                        "  正在提交…".into()
                    } else {
                        " ✓ 提交暂存更改 [C]".into()
                    },
                    ' ',
                    ScmTarget::Commit(repo_index),
                ));
                for (staged, title) in [(true, "Staged"), (false, "Changes")] {
                    let entries = code
                        .changes
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.root == repo.root && c.staged == staged)
                        .collect::<Vec<_>>();
                    let section_collapsed = self
                        .git
                        .section_collapsed
                        .contains(&(repo.root.clone(), staged));
                    rows.push((
                        None,
                        format!(
                            " {} {title} ({})",
                            if section_collapsed { "▸" } else { "▾" },
                            entries.len()
                        ),
                        ' ',
                        ScmTarget::Section(repo_index, staged),
                    ));
                    if section_collapsed {
                        continue;
                    }
                    let files = entries
                        .iter()
                        .map(|(_, c)| herdr_work_code::git::FileEntry {
                            path: c.path.clone(),
                            orig: c.original.clone(),
                            letter: c.status,
                        })
                        .collect::<Vec<_>>();
                    let collapsed = self
                        .git
                        .collapsed
                        .get(&(repo.root.clone(), staged))
                        .cloned()
                        .unwrap_or_default();
                    let (dirs, depths, tree_rows) =
                        herdr_work_code::scm_tree::build_tree_model(&files, &collapsed);
                    let tree_rows = if self.git.tree {
                        tree_rows
                    } else {
                        (0..files.len())
                            .map(herdr_work_code::scm_tree::TreeItem::File)
                            .collect()
                    };
                    for item in tree_rows {
                        match item {
                            herdr_work_code::scm_tree::TreeItem::Dir(i) => {
                                let dir = &dirs[i];
                                rows.push((
                                    None,
                                    format!(
                                        "  {}{} {} ({})",
                                        "  ".repeat(dir.depth),
                                        if dir.expanded { "▾" } else { "▸" },
                                        dir.name,
                                        dir.count
                                    ),
                                    ' ',
                                    ScmTarget::Directory(repo_index, staged, dir.path.clone()),
                                ));
                            }
                            herdr_work_code::scm_tree::TreeItem::File(i) => {
                                let (index, change) = entries[i];
                                let name = if self.git.tree {
                                    change.path.rsplit('/').next().unwrap_or(&change.path)
                                } else {
                                    &change.path
                                };
                                let status = if change.status == '?' {
                                    'U'
                                } else {
                                    change.status
                                };
                                rows.push((
                                    Some(index),
                                    format!(
                                        "  {}{status} {name}",
                                        if self.git.tree {
                                            "  ".repeat(depths[i])
                                        } else {
                                            String::new()
                                        }
                                    ),
                                    status,
                                    ScmTarget::Change(index),
                                ));
                            }
                        }
                    }
                }
            }
        }
        for kind in herdr_work_code::scm::Drawer::ALL {
            if kind == herdr_work_code::scm::Drawer::Graph {
                continue;
            }
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
