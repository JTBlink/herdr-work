//! Task list, selection details and initial-load guidance.
use crate::{loading::LoadState, state::*};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

impl App {
    pub(crate) fn draw_tasks(&self, f: &mut Frame, area: Rect) {
        let rows = self
            .tasks
            .iter()
            .enumerate()
            .skip(
                self.task
                    .saturating_sub(area.height.saturating_sub(4) as usize / 2),
            )
            .map(|(i, t)| {
                ListItem::new(format!(
                    "{}{} [{}]\n  {}",
                    if i == self.task { "> " } else { "  " },
                    t.title,
                    t.stage,
                    t.id
                ))
            })
            .collect::<Vec<_>>();
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Tasks · Enter select · n new · d delegate · x stop");
        if self.tasks.is_empty() {
            let message = match &self.tasks_state {
                LoadState::Loading => "正在加载任务…",
                LoadState::Ready => "暂无任务\n按 n 创建任务",
                LoadState::Failed(_) => "任务后端不可用\n按 r 重试，或查看详情",
            };
            f.render_widget(
                Paragraph::new(message)
                    .wrap(Wrap { trim: false })
                    .block(block),
                area,
            );
        } else {
            f.render_widget(List::new(rows).block(block), area);
        }
    }
    pub(crate) fn load_summary(&self) -> String {
        let tasks = match &self.tasks_state {
            LoadState::Loading => "任务：加载中".into(),
            LoadState::Ready => format!("任务：{}", self.tasks.len()),
            LoadState::Failed(_) => "任务后端不可用".into(),
        };
        let code = match &self.code_state {
            LoadState::Loading => "文件/Git：加载中".into(),
            LoadState::Ready => self
                .code
                .as_ref()
                .map(|code| {
                    format!(
                        "文件：{} · Git 修改：{}",
                        code.files.len(),
                        code.changes.len()
                    )
                })
                .unwrap_or_default(),
            LoadState::Failed(error) => format!("文件/Git 加载失败：{error}"),
        };
        format!("{tasks} · {code}")
    }
    pub(crate) fn draw_task_detail(&self, f: &mut Frame, area: Rect) {
        let text = match &self.tasks_state {
            LoadState::Loading if self.tasks.is_empty() => "正在读取任务数据…".into(),
            LoadState::Failed(error) => format!(
                "任务后端不可用\n\n{error}\n\n按 r 重试；运行 herdr-work doctor 检查后端。\n仍可按 2 浏览文件、按 3 查看 Git。"
            ),
            _ => match self.tasks.get(self.task) {
                None => format!(
                    "暂无任务\n\n按 n 创建任务，Enter 确认标题。\n按 2 浏览文件，按 3 查看 Git 修改。\n\n任务保存在 HerdrWork 自身的数据目录。\n创建后按 d 委派，按 r 刷新状态。\n\n当前仓库：{}",
                    self.root.display()
                ),
                Some(task) => {
                    let mut detail =
                        format!("{}\n\nID：{}\n状态：{}", task.title, task.id.0, task.stage);
                    if task.executions.is_empty() {
                        detail.push_str("\n\n尚无执行实例。按 d 委派任务，按 r 刷新。\n文件和 Git 视图仍可浏览当前仓库。");
                    }
                    for execution in &task.executions {
                        detail.push_str(&format!(
                            "\n\n执行：{}\n目录：{}\n窗格：{}",
                            execution.id.0,
                            execution.worktree.display(),
                            execution
                                .target
                                .as_ref()
                                .map(|target| target.pane.as_str())
                                .unwrap_or("未绑定")
                        ));
                    }
                    detail
                }
            },
        };
        f.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::ALL).title("Task details")),
            area,
        );
    }
}
