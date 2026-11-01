//! Independent read states for files, Git and search.
#[derive(Default)]
pub(crate) enum LoadState {
    #[default]
    Loading,
    Ready,
    Failed(String),
}

use crate::state::App;
impl App {
    pub(crate) fn load_summary(&self) -> String {
        match &self.code_state {
            LoadState::Loading => "文件/Git：加载中".into(),
            LoadState::Ready => self
                .code
                .as_ref()
                .map(|code| {
                    format!(
                        "文件：{} · Git 修改：{}{}",
                        code.files.len(),
                        code.changes.len(),
                        if code.repositories.iter().any(|r| r.write_error.is_some()) {
                            " · Git 安全快照只读"
                        } else {
                            ""
                        }
                    )
                })
                .unwrap_or_default(),
            LoadState::Failed(error) => format!("文件/Git 加载失败：{error}"),
        }
    }
}
