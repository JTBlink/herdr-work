//! Search UI delegates indexing, ranking and matching to Sidebar's shared module.
use crate::{loading::LoadState, state::*};
use anyhow::{Context, Result};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{List, ListItem, Paragraph},
};

use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct SearchRuntime {
    cache: Option<herdr_work_engine::QuickIndex>,
    in_flight: Option<(u64, u64)>,
    dirty_since: Option<Instant>,
}
impl App {
    pub(crate) fn invalidate_quick_index(&mut self) {
        self.search_runtime.cache = None;
        if self.tab == Tab::Search && self.search_quick {
            let _ = self.search();
        }
    }
    pub(crate) fn search_prompt(&self) -> bool {
        matches!(
            self.prompt,
            Some(Prompt::Search(_) | Prompt::SearchInclude | Prompt::SearchExclude)
        )
    }
    pub(crate) fn query_prompt(&mut self, quick: bool) {
        self.edit_request = None;
        if self.search_quick != quick {
            self.search_query.clear();
        }
        self.search_quick = quick;
        self.switch_tab(Tab::Search);
        self.prompt = Some(Prompt::Search(quick));
        self.input = self.search_query.clone();
        if let Err(error) = self.search() {
            self.status = error.to_string();
        }
    }
    /// Called after typing or paste; source search uses a 300 ms quiet interval.
    pub(crate) fn search_input_changed(&mut self) -> Result<()> {
        match self.prompt {
            Some(Prompt::Search(_)) => self.search_query = self.input.clone(),
            Some(Prompt::SearchInclude) => self.search_include = self.input.clone(),
            Some(Prompt::SearchExclude) => self.search_exclude = self.input.clone(),
            _ => return Ok(()),
        }
        self.search_request += 1;
        self.search_runtime.dirty_since = Some(Instant::now());
        self.search_state = LoadState::Loading;
        if self.search_quick {
            self.tick_search(Instant::now())?;
        }
        Ok(())
    }
    pub(crate) fn search(&mut self) -> Result<()> {
        self.search_request += 1;
        self.search_runtime.dirty_since = Some(Instant::now() - Duration::from_millis(300));
        self.search_state = LoadState::Loading;
        self.tick_search(Instant::now())
    }
    pub(crate) fn tick_search(&mut self, now: Instant) -> Result<()> {
        let Some(dirty) = self.search_runtime.dirty_since else {
            return Ok(());
        };
        if self.search_quick {
            if let Some(index) = &self.search_runtime.cache
                && index.root == self.root
                && index.show_hidden == self.preferences.show_hidden
            {
                let view = index.matches(&self.search_query);
                self.search_rows = view.rows;
                self.search_state = LoadState::Ready;
                if self.tab == Tab::Search {
                    self.selection = 0;
                }
                self.search_runtime.dirty_since = None;
                if view.truncated && self.tab == Tab::Search {
                    self.status = "结果已截断，请缩小搜索范围".into();
                }
                return Ok(());
            }
        } else if now.saturating_duration_since(dirty) < Duration::from_millis(300) {
            return Ok(());
        }
        if self
            .search_runtime
            .in_flight
            .is_some_and(|(epoch, _)| epoch == self.context.epoch)
        {
            return Ok(());
        }
        let request = self.search_request;
        let root = self.root.clone();
        let query = self.search_query.clone();
        let hidden = self.preferences.show_hidden;
        let options = self.search_options;
        let include = self.search_include.clone();
        let exclude = self.search_exclude.clone();
        if self.search_quick {
            self.job(move |d| Ok(Data::QuickIndex(request, d.quick_index(&root, hidden))))?;
        } else {
            self.job(move |d| {
                Ok(Data::Search(
                    request,
                    d.search_files(&root, &query, hidden, options, &include, &exclude),
                ))
            })?;
        }
        self.search_runtime.in_flight = Some((self.context.epoch, request));
        self.search_runtime.dirty_since = None;
        Ok(())
    }
    pub(crate) fn search_finished(&mut self, request: u64) {
        if self.search_runtime.in_flight == Some((self.context.epoch, request)) {
            self.search_runtime.in_flight = None;
        }
    }
    pub(crate) fn receive_quick_index(&mut self, result: Result<herdr_work_engine::QuickIndex>) {
        match result {
            Ok(index)
                if index.root == self.root && index.show_hidden == self.preferences.show_hidden =>
            {
                self.search_runtime.cache = Some(index);
                if self.search_quick {
                    self.search_runtime.dirty_since = Some(Instant::now());
                    let _ = self.tick_search(Instant::now());
                }
            }
            Err(error) if self.search_quick => {
                self.search_state = LoadState::Failed(error.to_string())
            }
            _ => {}
        }
    }
    pub(crate) fn open_search_result(&mut self) -> Result<()> {
        self.edit_request = None;
        let row = self
            .search_rows
            .get(self.selection)
            .context("暂无结果；按 / 输入搜索内容")?
            .clone();
        self.open_preview(row.path, Some(row.line))
    }
    pub(crate) fn search_regions(&self, area: Rect) -> (Rect, Rect) {
        let inner = self.sidebar_block(String::new()).inner(area);
        let height = inner.height.min(if self.search_quick { 1 } else { 4 });
        (
            Rect::new(inner.x, inner.y, inner.width, height),
            Rect::new(
                inner.x,
                inner.y + height,
                inner.width,
                inner.height.saturating_sub(height),
            ),
        )
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
        let (header_area, body) = self.search_regions(area);
        f.render_widget(block, area);
        let options = self.search_options;
        let header = format!(
            "{}\n[Aa:{}] [W:{}] [.*:{}]\n包含: {}\n排除: {}",
            format_args!(
                "{}{}",
                if matches!(self.prompt, Some(Prompt::Search(_))) {
                    "› "
                } else {
                    ""
                },
                self.search_query
            ),
            if options.match_case { "●" } else { "○" },
            if options.whole_word { "●" } else { "○" },
            if options.regex { "●" } else { "○" },
            self.search_include,
            self.search_exclude
        );
        let header = if self.search_quick {
            header.lines().next().unwrap_or_default().to_string()
        } else {
            header
        };
        f.render_widget(Paragraph::new(header), header_area);
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
