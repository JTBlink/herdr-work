//! A VS Code-style activity rail, Explorer/SCM sidebar and independent content pane.
use crate::{
    layout::{ACTIVITIES, settings_button},
    state::*,
};
use herdr_work_code::ui::{activity_button_style, palette};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

impl App {
    pub(crate) fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        let layout = self.panes(area);
        f.render_widget(
            Paragraph::new(format!("HerdrWork  | {}", self.root.display()))
                .style(Style::default().fg(palette().header_accent)),
            layout.header,
        );
        for (index, (tab, label, key)) in ACTIVITIES.iter().enumerate() {
            let y = layout.activity.y + index as u16 * 3;
            let height = settings_button(layout.activity).y.saturating_sub(y).min(3);
            if height == 0 {
                break;
            }
            let (files, search, git) = herdr_work_code::ui::activity_icons(self.icon_theme());
            let icon = match tab {
                Tab::Files => files,
                Tab::Search => search,
                Tab::Scm => git,
                Tab::Tasks => {
                    if self.icon_theme() == herdr_work_code::icons::IconTheme::Material {
                        "\u{f0ae}"
                    } else {
                        "☷"
                    }
                }
                Tab::Review => {
                    if self.icon_theme() == herdr_work_code::icons::IconTheme::Material {
                        "\u{f06e}"
                    } else {
                        "◉"
                    }
                }
            };
            let badge = if *tab == Tab::Scm {
                self.code
                    .as_ref()
                    .map(|c| c.changes.len())
                    .filter(|n| *n > 0)
                    .map(|n| format!(" {n}"))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let _ = label;
            f.render_widget(
                Paragraph::new(format!("{icon}{badge}\n{key}"))
                    .alignment(Alignment::Center)
                    .style(activity_button_style(self.tab == *tab, false)),
                Rect::new(layout.activity.x, y, layout.activity.width, height),
            );
        }
        f.render_widget(
            Paragraph::new(format!(
                "{}\n设置",
                herdr_work_code::ui::gear_icon(self.icon_theme())
            ))
            .alignment(Alignment::Center)
            .style(activity_button_style(self.overlay.is_some(), false)),
            settings_button(layout.activity),
        );
        self.draw_sidebar(f, layout.sidebar);
        if layout.content.width > 0 {
            match self.tab {
                Tab::Tasks => self.draw_task_detail(f, layout.content),
                Tab::Files | Tab::Search => {
                    let title = self
                        .preview_path
                        .as_ref()
                        .map(|p| {
                            format!(
                                "Preview · {}",
                                p.strip_prefix(
                                    self.code
                                        .as_ref()
                                        .map(|c| c.root.as_path())
                                        .unwrap_or(&self.root)
                                )
                                .unwrap_or(p)
                                .display()
                            )
                        })
                        .unwrap_or_else(|| "Preview".into());
                    let text = if self.preview_path.is_none() {
                        "选择文件，按 Enter 或点击预览。\n\ne 编辑文本 · m 文件菜单 · , 设置\nTab 切换焦点 · 3 查看 Git 变更 · 1 管理任务"
                    } else {
                        self.text.as_str()
                    };
                    f.render_widget(
                        Paragraph::new(text)
                            .scroll((self.preview_scroll, 0))
                            .block(self.content_block(title)),
                        layout.content,
                    );
                }
                Tab::Scm if self.active_drawer.is_some() => self.draw_drawer(f, layout.content),
                Tab::Scm | Tab::Review => self.draw_diff(f, layout.content),
            }
        }
        let hints = match (self.tab, self.focus) {
            (Tab::Search, _) => "/ 搜索 · c 大小写 · w 全词 · * 正则 · i 包含 · o 排除 · e 编辑",
            (Tab::Files, Focus::Content) => "↑/↓ 滚动预览 · Tab 返回文件树",
            (Tab::Files, _) => "Enter 展开/预览 · e 编辑 · m 菜单 · , 设置 · Tab 聚焦预览",
            (Tab::Scm, _) => "Enter 查看差异 · s 暂存 · u 取消暂存 · C 提交",
            (Tab::Review, _) => "c 评论 · s 发送 · b 绑定 · f 聚焦 Agent · Tab 返回 Git",
            (Tab::Tasks, _) => "Enter 选择执行 · n 创建 · d 委派 · x 停止",
        };
        f.render_widget(
            Paragraph::new(format!(
                "{}\n{}{}",
                self.load_summary(),
                self.status,
                if self.preferences.show_hotkeys {
                    format!("\n{hints} · r 刷新 · q 退出")
                } else {
                    String::new()
                }
            ))
            .wrap(Wrap { trim: false }),
            layout.footer,
        );
        self.draw_editor(f, &layout);
        if let Some(prompt) = &self.prompt {
            let label = match prompt {
                Prompt::Comment { .. } => "Comment",
                Prompt::Commit(_) => "Commit staged changes",
                Prompt::NewTask => "New task",
                Prompt::Stop(_) => "Type STOP to stop execution",
                Prompt::Search(true) => "快速打开文件",
                Prompt::Search(false) => "搜索文本",
                Prompt::SearchInclude => "包含文件（逗号分隔 glob）",
                Prompt::SearchExclude => "排除文件（逗号分隔 glob）",
                Prompt::File(plan) => match plan.action {
                    herdr_work_code::actions::MenuAction::NewFile => "新建文件",
                    herdr_work_code::actions::MenuAction::NewFolder => "新建文件夹",
                    herdr_work_code::actions::MenuAction::Rename => "重命名",
                    _ => "删除：输入 DELETE 确认",
                },
                Prompt::Folder => "打开文件夹（完整路径）",
                Prompt::Git(_) => "Git 操作：输入 APPLY 确认",
            };
            let popup = Rect::new(
                area.x + area.width / 10,
                area.y + area.height / 3,
                area.width * 8 / 10,
                (if matches!(prompt, Prompt::Git(_)) {
                    8
                } else {
                    5
                })
                .min(area.height),
            );
            f.render_widget(Clear, popup);
            f.render_widget(
                Paragraph::new(if let Prompt::Git(plan) = prompt {
                    format!(
                        "工作目录：{}\ngit {}\n{}",
                        plan.root.display(),
                        plan.args.join(" "),
                        self.input
                    )
                } else {
                    self.input.clone()
                })
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("{label} · Enter confirm / Esc cancel")),
                ),
                popup,
            );
        }
        self.draw_overlay(f, area);
    }
    pub(crate) fn content_block(&self, title: String) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(if self.focus == Focus::Content {
                palette().accent_focus
            } else {
                palette().ignored
            }))
    }
    fn draw_diff(&self, f: &mut Frame, area: Rect) {
        if let Some(diff) = &self.diff {
            let selected = if self.tab == Tab::Review {
                self.selection
            } else {
                0
            };
            let rows = diff
                .rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    Line::styled(
                        format!(
                            "{}{:>5} {:>5} {}{}",
                            if i == selected { ">" } else { " " },
                            r.old.map(|n| n.to_string()).unwrap_or_default(),
                            r.new.map(|n| n.to_string()).unwrap_or_default(),
                            r.marker,
                            r.text
                        ),
                        Style::default().fg(match r.marker {
                            '+' => Color::Green,
                            '-' => Color::Red,
                            _ => Color::Reset,
                        }),
                    )
                })
                .collect::<Vec<_>>();
            f.render_widget(
                Paragraph::new(rows)
                    .scroll((
                        selected
                            .saturating_sub(area.height as usize / 2)
                            .min(u16::MAX as usize) as u16,
                        0,
                    ))
                    .block(self.content_block(format!("Review · {}", diff.file))),
                area,
            );
        } else {
            f.render_widget(Paragraph::new("选择左侧 Git 变更，按 Enter 或点击查看差异。\n\nStaged：已暂存 · Changes：工作区变更")
                .block(self.content_block("Review".into())), area);
        }
    }
}
