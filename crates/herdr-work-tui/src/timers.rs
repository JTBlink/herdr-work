//! Timer backpressure is recoverable: the event loop and owned input stay alive.
use crate::state::App;
use std::time::{Duration, Instant};

impl App {
    pub(crate) fn tick_background(&mut self, now: Instant, last_editor_poll: &mut Instant) -> bool {
        let mut redraw = self.tick_preview() | self.tick_hover() | self.tick_history();
        let mut errors = Vec::new();
        for (name, result) in [
            ("活动请求", self.tick_activation(now)),
            ("搜索", self.tick_search(now)),
            ("提交草稿", self.tick_git_drafts(now)),
            ("工作区状态", self.tick_persistence(now)),
            ("设置", self.tick_preferences()),
            ("自动刷新", self.tick_refresh(now)),
        ] {
            if let Err(error) = result {
                errors.push(format!("{name}：{error:#}"));
            }
        }
        if now.saturating_duration_since(*last_editor_poll) >= Duration::from_secs(2) {
            match self.poll_editor() {
                Ok(()) => *last_editor_poll = now,
                Err(error) => errors.push(format!("编辑器检查：{error:#}")),
            }
        }
        if !errors.is_empty() {
            self.status = format!(
                "后台任务暂未排入，已保留输入，将自动重试 · {}",
                errors.join(" · ")
            );
            redraw = true;
        }
        redraw
    }
}
