//! A VS Code-style activity rail, Explorer/SCM sidebar and independent content pane.
use crate::state::*;
use herdr_work_code::ui::palette;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

impl App {
    pub(crate) fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        self.interaction.area = area;
        let layout = self.panes(area);
        self.draw_activity_rail(f, layout.activity);
        self.draw_sidebar(f, layout.sidebar);
        if layout.content.width > 0 {
            match self.tab {
                Tab::Tasks => self.draw_task_detail(f, layout.content),
                Tab::Files | Tab::Search => self.draw_preview(f, layout.content),
                Tab::Scm if self.active_drawer.is_some() => self.draw_drawer(f, layout.content),
                Tab::Scm | Tab::Review => self.draw_diff(f, layout.content),
            }
        }
        if matches!(self.tab, Tab::Scm | Tab::Review) && layout.content.width > 0 {
            self.draw_scm_content_tabs(f, layout.content);
        }
        let hints = match (self.tab, self.focus) {
            (Tab::Search, _) => "/ 搜索 · c 大小写 · w 全词 · * 正则 · i 包含 · o 排除 · e 编辑",
            (Tab::Files, Focus::Content) => "↑/↓ 滚动预览 · Tab 返回文件树",
            (Tab::Files, _) => "Enter 展开/预览 · e 编辑 · m 菜单 · , 设置 · Tab 聚焦预览",
            (Tab::Scm, _) => "Enter 查看差异 · g 图谱/差异 · s 暂存 · u 取消暂存 · C 提交",
            (Tab::Review, _) => "c 评论 · s 发送 · b 绑定 · g 图谱 · f 聚焦 Agent · Tab 返回 Git",
            (Tab::Tasks, _) => "Enter 选择执行 · n 创建 · d 委派 · x 停止",
        };
        self.draw_footer(f, layout.footer, hints);
        self.draw_editor(f, &layout);
        if let Some(prompt) = &self.prompt
            && !self.search_prompt()
        {
            let label = match prompt {
                Prompt::Comment { .. } => "Comment",
                Prompt::Commit(_) => "Commit staged changes",
                Prompt::NewTask => "New task",
                Prompt::Stop(_) => "Type STOP to stop execution",
                Prompt::Search(true) => "快速打开文件",
                Prompt::Search(false) => "搜索文本",
                Prompt::SearchReplace => "替换",
                Prompt::ReplaceConfirm(_) => "替换预览：输入 REPLACE 确认，Esc 取消",
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
                Prompt::WorkGit(plan) => {
                    if matches!(
                        plan.action(),
                        herdr_work_engine::WorkGitAction::ForceDeleteBranch(_)
                    ) {
                        "分支未合并：输入 FORCE DELETE 再次确认永久删除"
                    } else {
                        "Git 操作：输入 APPLY 确认"
                    }
                }
                Prompt::Resolve(_) => "核对后输入实际证据（仅保存核对，不重试）",
            };
            let details = match prompt {
                Prompt::ReplaceConfirm(plan) => Some(plan.description()),
                Prompt::Git(plan) => Some(format!(
                    "工作目录：{}\ngit {}",
                    plan.root.display(),
                    plan.arguments().join(" ")
                )),
                Prompt::WorkGit(plan) => Some(plan.description()),
                Prompt::Resolve(name) => Some(format!(
                    "{}\n{}",
                    name,
                    self.git
                        .unresolved
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, r)| r.as_str())
                        .unwrap_or_default()
                )),
                _ => None,
            };
            let height = if matches!(prompt, Prompt::Resolve(_)) {
                13
            } else if matches!(prompt, Prompt::ReplaceConfirm(_)) {
                18
            } else if details.is_some() {
                8
            } else {
                5
            };
            let height = height.min(area.height);
            let popup = Rect::new(
                area.x + area.width / 10,
                area.y + (area.height.saturating_sub(height)) / 2,
                area.width * 8 / 10,
                height,
            );
            f.render_widget(Clear, popup);
            let block = Block::default()
                .borders(Borders::ALL)
                .title(format!("{label} · Enter confirm / Esc cancel"));
            let inner = block.inner(popup);
            f.render_widget(block, popup);
            if let Some(details) = details {
                f.render_widget(
                    Paragraph::new(details).wrap(Wrap { trim: false }),
                    Rect::new(
                        inner.x,
                        inner.y,
                        inner.width,
                        inner.height.saturating_sub(1),
                    ),
                );
                let tail = herdr_work_code::ui::input_tail(
                    &self.input,
                    inner.width.saturating_sub(2) as usize,
                );
                f.render_widget(
                    Paragraph::new(format!("> {tail}")),
                    Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
                );
            } else {
                f.render_widget(
                    Paragraph::new(self.input.clone()).wrap(Wrap { trim: false }),
                    inner,
                );
            }
        }

        self.draw_activity_tooltip(f, layout.activity);
        self.draw_overlay(f, area);
        if let Some(picker) = self.git.picker.as_mut() {
            picker.draw(f);
        }
    }
    pub(crate) fn content_block(&self, title: String) -> Block<'static> {
        Block::default()
            .title(title)
            .title_style(Style::default().fg(if self.focus == Focus::Content {
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
