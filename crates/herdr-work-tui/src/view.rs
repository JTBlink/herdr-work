//! One terminal owner, with background jobs and upstream-backed views.
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::state::*;
impl App {
    pub(crate) fn draw(&self, f: &mut Frame) {
        let area = f.area();
        let parts = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);
        f.render_widget(
            Paragraph::new(format!(
                "HerdrWork  1 Tasks  2 Files  3 SCM  4 Review  | {}",
                self.root.display()
            ))
            .style(Style::default().fg(Color::Cyan)),
            parts[0],
        );
        let wide = area.width >= 100;
        let body = if wide {
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(30), Constraint::Min(1)])
                .split(parts[1]);
            self.draw_tasks(f, columns[0]);
            columns[1]
        } else {
            parts[1]
        };
        match self.tab {
            Tab::Tasks if wide => self.draw_task_detail(f, body),
            Tab::Tasks => {
                let rows =
                    Layout::vertical([Constraint::Percentage(45), Constraint::Percentage(55)])
                        .split(body);
                self.draw_tasks(f, rows[0]);
                self.draw_task_detail(f, rows[1]);
            }
            Tab::Files => {
                let files = self
                    .code
                    .as_ref()
                    .map(|c| {
                        c.files
                            .iter()
                            .enumerate()
                            .skip(
                                self.selection
                                    .saturating_sub(body.height.saturating_sub(4) as usize),
                            )
                            .map(|(i, r)| {
                                ListItem::new(format!(
                                    "{}{}{}{}",
                                    if i == self.selection { "> " } else { "  " },
                                    "  ".repeat(r.depth),
                                    if r.directory { "▸ " } else { "" },
                                    r.name
                                ))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let columns =
                    Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                        .split(body);
                f.render_widget(
                    List::new(files).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Files · Enter expand/preview"),
                    ),
                    columns[0],
                );
                f.render_widget(
                    Paragraph::new(self.text.as_str())
                        .scroll((self.selection.min(u16::MAX as usize) as u16, 0))
                        .block(Block::default().borders(Borders::ALL).title("Preview")),
                    columns[1],
                );
            }
            Tab::Scm => {
                let rows = self
                    .code
                    .as_ref()
                    .map(|c| {
                        c.changes
                            .iter()
                            .enumerate()
                            .skip(
                                self.selection
                                    .saturating_sub(body.height.saturating_sub(4) as usize),
                            )
                            .map(|(i, c)| {
                                ListItem::new(format!(
                                    "{}{} {} {}",
                                    if i == self.selection { "> " } else { "  " },
                                    if c.staged { "Index" } else { "Worktree" },
                                    c.status,
                                    c.path
                                ))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                f.render_widget(
                    List::new(rows).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("SCM · Enter diff · s stage · u unstage · C commit"),
                    ),
                    body,
                );
            }
            Tab::Review => {
                let rows: Vec<Line> = self
                    .diff
                    .as_ref()
                    .map(|d| {
                        d.rows
                            .iter()
                            .enumerate()
                            .map(|(i, r)| {
                                Line::styled(
                                    format!(
                                        "{}{:>5} {:>5} {}{}",
                                        if i == self.selection { ">" } else { " " },
                                        r.old.map(|n| n.to_string()).unwrap_or_default(),
                                        r.new.map(|n| n.to_string()).unwrap_or_default(),
                                        r.marker,
                                        r.text
                                    ),
                                    Style::default().fg(match r.marker {
                                        '+' => Color::Green,
                                        '-' => Color::Red,
                                        _ => Color::White,
                                    }),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                f.render_widget(
                    Paragraph::new(rows)
                        .scroll((
                            self.selection
                                .saturating_sub(body.height as usize / 2)
                                .min(u16::MAX as usize) as u16,
                            0,
                        ))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title("Review · c comment · s send · b bind · f focus"),
                        ),
                    body,
                );
            }
        }
        f.render_widget(
            Paragraph::new(format!(
                "{}\n{}\nr refresh · q close UI (Agents and enabled keeper continue)",
                self.load_summary(),
                self.status
            ))
            .wrap(Wrap { trim: false }),
            parts[2],
        );
        if let Some(prompt) = &self.prompt {
            let label = match prompt {
                Prompt::Comment { .. } => "Comment",
                Prompt::Commit(_) => "Commit staged changes",
                Prompt::NewTask => "New task",
                Prompt::Stop(_) => "Type STOP to stop execution",
            };
            let popup = Rect::new(
                area.x + area.width / 10,
                area.y + area.height / 3,
                area.width * 8 / 10,
                5.min(area.height),
            );
            f.render_widget(Clear, popup);
            f.render_widget(
                Paragraph::new(self.input.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("{label} · Enter confirm / Esc cancel")),
                ),
                popup,
            );
        }
    }
}
