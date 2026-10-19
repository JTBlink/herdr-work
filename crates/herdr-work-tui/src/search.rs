//! Search UI delegates indexing, ranking and matching to Sidebar's shared module.
use crate::{loading::LoadState, state::*};
use anyhow::{Context, Result};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{List, ListItem, Paragraph},
};

impl App {
    pub(crate) fn query_prompt(&mut self, quick: bool) {
        self.edit_request = None;
        self.prompt = Some(Prompt::Search(quick));
        self.input = if self.search_quick == quick {
            self.search_query.clone()
        } else {
            String::new()
        };
    }
    pub(crate) fn search(&mut self) -> Result<()> {
        self.search_request += 1;
        let request = self.search_request;
        let root = self.root.clone();
        let query = self.search_query.clone();
        let quick = self.search_quick;
        let hidden = self.preferences.show_hidden;
        let options = self.search_options;
        let include = self.search_include.clone();
        let exclude = self.search_exclude.clone();
        self.search_state = LoadState::Loading;
        self.job(move |d| {
            Ok(Data::Search(
                request,
                d.search_files(&root, &query, quick, hidden, options, &include, &exclude),
            ))
        })
    }
    pub(crate) fn open_search_result(&mut self) -> Result<()> {
        self.edit_request = None;
        let row = self
            .search_rows
            .get(self.selection)
            .context("暂无结果；按 / 输入搜索内容")?
            .clone();
        self.preview_path = Some(row.path.clone());
        self.text = "正在读取文件…".into();
        let root = self.root.clone();
        let file = row.path.strip_prefix(&root)?.to_string_lossy().into_owned();
        self.job(move |d| Ok(Data::Preview(row.path, d.file(&root, &file))))
    }
    pub(crate) fn draw_search_sidebar(&self, f: &mut Frame, area: Rect) {
        let block = self.sidebar_block(
            if self.search_quick {
                "QUICK OPEN"
            } else {
                "SEARCH"
            }
            .into(),
        );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let options = self.search_options;
        let header = format!(
            "{}\n[Aa:{}] [W:{}] [.*:{}]\n包含: {}\n排除: {}",
            self.search_query,
            options.match_case,
            options.whole_word,
            options.regex,
            self.search_include,
            self.search_exclude
        );
        let height = inner.height.min(4);
        f.render_widget(
            Paragraph::new(header),
            Rect::new(inner.x, inner.y, inner.width, height),
        );
        let body = Rect::new(
            inner.x,
            inner.y + height,
            inner.width,
            inner.height.saturating_sub(height),
        );
        let hint = match &self.search_state {
            LoadState::Loading => Some("正在搜索…".to_string()),
            LoadState::Failed(e) => Some(format!("搜索失败：{e}")),
            _ if self.search_rows.is_empty() => Some("暂无结果 · / 搜索 · Ctrl+P 快速打开".into()),
            _ => None,
        };
        if let Some(hint) = hint {
            f.render_widget(Paragraph::new(hint), body);
            return;
        }
        let offset = crate::layout::offset(self.selection, body.height);
        let rows = self
            .search_rows
            .iter()
            .enumerate()
            .skip(offset)
            .map(|(i, row)| {
                ListItem::new(format!("{}:{} {}", row.label, row.line, row.context)).style(
                    if i == self.selection {
                        herdr_work_code::ui::selection_style(self.focus == Focus::Sidebar)
                    } else {
                        Default::default()
                    },
                )
            })
            .collect::<Vec<_>>();
        f.render_widget(List::new(rows), body);
    }
}
