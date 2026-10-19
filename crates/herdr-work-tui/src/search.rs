//! Search UI delegates indexing, ranking and matching to Sidebar's shared module.
use crate::{loading::LoadState, state::*};
use anyhow::{Context, Result};
use ratatui::{
    Frame,
    layout::{Alignment, Position, Rect},
    style::Style,
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph},
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
    pub(crate) fn search_filters_visible(&self) -> bool {
        self.search_details
            || matches!(
                self.prompt,
                Some(Prompt::SearchInclude | Prompt::SearchExclude)
            )
    }
    pub(crate) fn search_layout(&self, area: Rect) -> SearchLayout {
        let inner = self.sidebar_block(String::new()).inner(area);
        let height = if self.search_quick {
            1
        } else if self.search_filters_visible() {
            8
        } else {
            4
        };
        let header = Rect::new(inner.x, inner.y, inner.width, inner.height.min(height));
        let row = |y: u16, height: u16| {
            Rect::new(
                inner.x + inner.width.min(1),
                inner.y + y.min(inner.height),
                inner.width.saturating_sub(2),
                inner.height.saturating_sub(y).min(height),
            )
        };
        let query_box = row(0, if self.search_quick { 1 } else { 3 });
        let mut query = if self.search_quick {
            query_box
        } else {
            Block::default().borders(Borders::ALL).inner(query_box)
        };
        let mut options = [Rect::default(); 3];
        if !self.search_quick && query.width >= 16 {
            let start = query.right() - 12;
            for (i, rect) in options.iter_mut().enumerate() {
                *rect = Rect::new(start + i as u16 * 4, query.y, 4, query.height);
            }
            query.width -= 12;
        }
        SearchLayout {
            header,
            query_box,
            query,
            options,
            details: if self.search_quick {
                Rect::default()
            } else {
                row(3, 1)
            },
            include_label: row(4, 1),
            include: row(5, 1),
            exclude_label: row(6, 1),
            exclude: row(7, 1),
            body: Rect::new(
                inner.x,
                inner.y + header.height,
                inner.width,
                inner.height.saturating_sub(header.height),
            ),
        }
    }
    pub(crate) fn search_regions(&self, area: Rect) -> (Rect, Rect) {
        let layout = self.search_layout(area);
        (layout.header, layout.body)
    }
    fn draw_search_field(
        &self,
        f: &mut Frame,
        area: Rect,
        value: &str,
        placeholder: &str,
        active: bool,
    ) {
        if area.is_empty() {
            return;
        }
        let colors = herdr_work_code::ui::palette();
        let style = Style::default()
            .bg(colors.keycap_bg)
            .fg(if value.is_empty() {
                colors.ignored
            } else {
                colors.keycap_fg
            });
        // Scroll from the end using terminal column widths, including CJK input.
        let mut visible = String::new();
        for c in value.chars().rev() {
            let candidate = format!("{c}{visible}");
            if Span::raw(candidate.as_str()).width() >= usize::from(area.width) {
                break;
            }
            visible = candidate;
        }
        let cursor = Span::raw(visible.as_str()).width() as u16;
        f.render_widget(
            Paragraph::new(if value.is_empty() {
                placeholder.to_string()
            } else {
                visible
            })
            .style(style),
            area,
        );
        if active && self.focus == Focus::Sidebar {
            f.set_cursor_position(Position::new(area.x + cursor, area.y));
        }
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
        let layout = self.search_layout(area);
        let body = layout.body;
        f.render_widget(block, area);
        let colors = herdr_work_code::ui::palette();
        let active = matches!(self.prompt, Some(Prompt::Search(_)));
        if !self.search_quick {
            f.render_widget(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if active {
                        colors.accent_focus
                    } else {
                        colors.ignored
                    })),
                layout.query_box,
            );
        }
        self.draw_search_field(
            f,
            layout.query,
            &self.search_query,
            if self.search_quick {
                "搜索文件"
            } else {
                "搜索"
            },
            active,
        );
        if !self.search_quick {
            for ((rect, label), enabled) in layout
                .options
                .into_iter()
                .zip([" Aa ", " ab ", " .* "])
                .zip([
                    self.search_options.match_case,
                    self.search_options.whole_word,
                    self.search_options.regex,
                ])
            {
                f.render_widget(
                    Paragraph::new(label).style(if enabled {
                        herdr_work_code::ui::selection_style(true)
                    } else {
                        Style::default().fg(colors.keycap_fg).bg(colors.keycap_bg)
                    }),
                    rect,
                );
            }
            let filtered = !self.search_include.is_empty() || !self.search_exclude.is_empty();
            f.render_widget(
                Paragraph::new(if self.search_filters_visible() {
                    "▾ 搜索范围 ⋯"
                } else if filtered {
                    "▸ 搜索范围 ● ⋯"
                } else {
                    "⋯"
                })
                .alignment(Alignment::Right)
                .style(Style::default().fg(colors.ignored)),
                layout.details,
            );
            if self.search_filters_visible() {
                for (label_area, field, label, value, active) in [
                    (
                        layout.include_label,
                        layout.include,
                        "包含文件",
                        self.search_include.as_str(),
                        matches!(self.prompt, Some(Prompt::SearchInclude)),
                    ),
                    (
                        layout.exclude_label,
                        layout.exclude,
                        "排除文件",
                        self.search_exclude.as_str(),
                        matches!(self.prompt, Some(Prompt::SearchExclude)),
                    ),
                ] {
                    f.render_widget(
                        Paragraph::new(label).style(Style::default().fg(colors.ignored)),
                        label_area,
                    );
                    self.draw_search_field(f, field, value, "例如 *.rs, src/**", active);
                }
            }
        }
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

/// Drawing and pointer handling consume these same clipped rectangles.
pub(crate) struct SearchLayout {
    pub header: Rect,
    pub query_box: Rect,
    pub query: Rect,
    pub options: [Rect; 3],
    pub details: Rect,
    pub include_label: Rect,
    pub include: Rect,
    pub exclude_label: Rect,
    pub exclude: Rect,
    pub body: Rect,
}
