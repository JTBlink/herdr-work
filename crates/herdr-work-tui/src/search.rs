//! Search UI delegates indexing, ranking and matching to Sidebar's shared module.
use crate::{loading::LoadState, state::*};
use anyhow::{Context, Result};
use herdr_work_code::search_widgets::{
    SearchLayout, draw_field, search_input_line, search_toolbar_icons,
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
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
            Some(
                Prompt::Search(_)
                    | Prompt::SearchReplace
                    | Prompt::SearchInclude
                    | Prompt::SearchExclude
            )
        )
    }
    pub(crate) fn query_prompt(&mut self, quick: bool) {
        self.edit_request = None;
        if self.search_quick != quick {
            self.search_query.clear();
        }
        self.search_quick = quick;
        self.switch_tab(Tab::Search);
        self.search_cursor = usize::MAX;
        self.search_select_all = false;
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
            Some(Prompt::SearchReplace) => {
                self.search_replacement = self.input.clone();
                return Ok(());
            }
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
    pub(crate) fn search_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<bool> {
        use crossterm::event::{KeyCode, KeyModifiers};
        if self.tab != Tab::Search
            || self.editor.is_some()
            || (self.prompt.is_some() && !self.search_prompt())
        {
            return Ok(false);
        }
        if key.modifiers.contains(KeyModifiers::ALT) && !self.search_quick {
            match key.code {
                KeyCode::Char('c' | 'C') => {
                    self.search_options.match_case = !self.search_options.match_case
                }
                KeyCode::Char('w' | 'W') => {
                    self.search_options.whole_word = !self.search_options.whole_word
                }
                KeyCode::Char('r' | 'R') => self.search_options.regex = !self.search_options.regex,
                KeyCode::Char('p' | 'P') => {
                    self.search_preserve_case = !self.search_preserve_case;
                    return Ok(true);
                }
                KeyCode::Enter => {
                    self.prepare_search_replacement(true)?;
                    return Ok(true);
                }
                _ => return Ok(false),
            }
            self.search()?;
            return Ok(true);
        }
        let control = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);
        if control && key.code == KeyCode::Enter && !self.search_quick {
            self.prepare_search_replacement(false)?;
            return Ok(true);
        }
        if !self.search_prompt() {
            return Ok(false);
        }
        let cursor = self.search_cursor_position(&self.input);
        self.search_cursor = cursor;
        match key.code {
            KeyCode::Home => self.search_cursor = 0,
            KeyCode::End => self.search_cursor = self.input.len(),
            KeyCode::Left => {
                self.search_cursor = self.input[..cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(i, _)| i)
            }
            KeyCode::Right => {
                self.search_cursor += self.input[cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8)
            }
            KeyCode::Char('a' | 'A') if control => {
                self.search_select_all = true;
                return Ok(true);
            }
            KeyCode::Char(c) if !control => {
                self.insert_search_text(&c.to_string())?;
                return Ok(true);
            }
            KeyCode::Backspace | KeyCode::Delete => {
                if self.search_select_all {
                    self.input.clear();
                    self.search_cursor = 0;
                } else if key.code == KeyCode::Backspace && cursor > 0 {
                    let previous = self.input[..cursor].char_indices().next_back().unwrap().0;
                    self.input.replace_range(previous..cursor, "");
                    self.search_cursor = previous;
                } else if key.code == KeyCode::Delete && cursor < self.input.len() {
                    let end = cursor + self.input[cursor..].chars().next().unwrap().len_utf8();
                    self.input.replace_range(cursor..end, "");
                }
                self.search_select_all = false;
                self.search_input_changed()?;
                return Ok(true);
            }
            _ if control => return Ok(true),
            _ => {
                self.search_cursor = usize::MAX;
                self.search_select_all = false;
                return Ok(false);
            }
        }
        self.search_select_all = false;
        Ok(true)
    }
    pub(crate) fn insert_search_text(&mut self, text: &str) -> Result<()> {
        let text = text.replace(['\r', '\n'], " ");
        if self.search_select_all {
            self.input.clear();
            self.search_cursor = 0;
        }
        let cursor = self.search_cursor_position(&self.input);
        self.input.insert_str(cursor, &text);
        self.search_cursor = cursor + text.len();
        self.search_select_all = false;
        self.search_input_changed()
    }
    fn search_cursor_position(&self, value: &str) -> usize {
        let mut cursor = self.search_cursor.min(value.len());
        while !value.is_char_boundary(cursor) {
            cursor -= 1;
        }
        cursor
    }
    pub(crate) fn prepare_search_replacement(&mut self, all: bool) -> Result<()> {
        anyhow::ensure!(!self.search_quick, "快速打开不支持替换");
        anyhow::ensure!(
            self.editor.is_none() && self.editor_write.is_none(),
            "请先保存并关闭编辑器再替换"
        );
        anyhow::ensure!(!self.search_replace_pending, "替换正在准备或执行");
        anyhow::ensure!(
            matches!(self.search_state, LoadState::Ready),
            "请等待搜索完成"
        );
        anyhow::ensure!(!self.search_truncated, "结果已截断，请缩小搜索范围");
        let selected = if all {
            None
        } else {
            let row = self
                .search_rows
                .get(self.selection)
                .context("没有选中的匹配")?;
            Some((row.path.clone(), row.line))
        };
        let request = herdr_work_engine::ReplaceRequest {
            root: self.root.clone(),
            query: self.search_query.clone(),
            replacement: self.search_replacement.clone(),
            options: self.search_options,
            preserve_case: self.search_preserve_case,
            show_hidden: self.preferences.show_hidden,
            include: self.search_include.clone(),
            exclude: self.search_exclude.clone(),
            selected,
        };
        let next = self.search_replace_request + 1;
        let search = self.search_request;
        self.job(move |d| {
            Ok(Data::ReplaceReady(
                next,
                search,
                d.prepare_replacement(request),
            ))
        })?;
        self.search_replace_request = next;
        self.search_replace_pending = true;
        self.status = "正在准备替换预览…".into();
        Ok(())
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
        if self.search_quick {
            let height = inner.height.min(1);
            let query = Rect::new(inner.x, inner.y, inner.width, height);
            return SearchLayout {
                header: query,
                query_box: query,
                query,
                body: Rect::new(
                    inner.x,
                    inner.y + height,
                    inner.width,
                    inner.height.saturating_sub(height),
                ),
                ..Default::default()
            };
        }
        SearchLayout::vscode(
            Rect::new(inner.x, area.y, inner.width, area.height),
            self.search_filters_visible(),
            self.search_replace_visible,
        )
    }
    pub(crate) fn search_toolbar_click(&mut self, area: Rect, point: Position) -> Result<bool> {
        if self.tab != Tab::Search || self.search_quick {
            return Ok(false);
        }
        let layout = self.search_layout(area);
        if layout.replace_toggle.contains(point) {
            self.search_replace_visible = !self.search_replace_visible;
            if !self.search_replace_visible && matches!(self.prompt, Some(Prompt::SearchReplace)) {
                self.prompt = Some(Prompt::Search(false));
                self.input = self.search_query.clone();
            }
            return Ok(true);
        }
        if layout.preserve_case.contains(point) {
            self.search_preserve_case = !self.search_preserve_case;
            return Ok(true);
        }
        if layout.replace_all.contains(point) {
            self.prepare_search_replacement(true)?;
            return Ok(true);
        }
        if layout.collapse.contains(point) {
            self.search_results_collapsed = !self.search_results_collapsed;
            self.search_collapsed_files = if self.search_results_collapsed {
                self.search_rows.iter().map(|r| r.path.clone()).collect()
            } else {
                Default::default()
            };
            return Ok(true);
        }
        let Some(index) = layout.toolbar.iter().position(|rect| rect.contains(point)) else {
            return Ok(false);
        };
        self.focus = Focus::Sidebar;
        match index {
            0 => self.search()?,
            1 => {
                self.search_query.clear();
                self.search_replacement.clear();
                self.search_include.clear();
                self.search_exclude.clear();
                self.prompt = Some(Prompt::Search(false));
                self.input.clear();
                self.search()?;
            }
            _ => {
                self.search_details = !self.search_filters_visible();
                if matches!(
                    self.prompt,
                    Some(Prompt::SearchInclude | Prompt::SearchExclude)
                ) {
                    self.prompt = Some(Prompt::Search(false));
                    self.input = self.search_query.clone();
                }
            }
        }
        Ok(true)
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
        let focused = active && self.focus == Focus::Sidebar;
        let mut line = search_input_line(value, placeholder, focused, area.width);
        if focused && self.search_cursor < value.len() {
            let cursor = self.search_cursor_position(value);
            let before = herdr_work_code::ui::input_tail(
                &value[..cursor],
                usize::from(area.width.saturating_sub(1)),
            );
            let used = ratatui::text::Span::raw(&before).width() + 1;
            let after = herdr_work_code::ui::truncate_to(
                value[cursor..].to_owned(),
                usize::from(area.width).saturating_sub(used),
            );
            line = ratatui::text::Line::from(format!("{before}▏{after}"));
        }
        if focused && self.search_select_all {
            line = line.style(herdr_work_code::ui::selection_style(true));
        }
        f.render_widget(Paragraph::new(line), area);
    }
    pub(crate) fn draw_search_sidebar(&self, f: &mut Frame, area: Rect) {
        let block = self.sidebar_block(
            if self.search_quick {
                "QUICK OPEN"
            } else {
                "搜索"
            }
            .into(),
        );
        let layout = self.search_layout(area);
        let body = layout.body;
        f.render_widget(block, area);
        let colors = herdr_work_code::ui::palette();
        let active = matches!(self.prompt, Some(Prompt::Search(_)));
        if !self.search_quick {
            draw_field(f, layout.query_box, active);
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
            f.render_widget(
                Paragraph::new(if self.search_replace_visible {
                    "⌄"
                } else {
                    "›"
                }),
                layout.replace_toggle,
            );
            if self.search_replace_visible {
                let replace_active = matches!(self.prompt, Some(Prompt::SearchReplace));
                draw_field(f, layout.replace_box, replace_active);
                self.draw_search_field(
                    f,
                    layout.replace,
                    &self.search_replacement,
                    "替换",
                    replace_active,
                );
                f.render_widget(
                    Paragraph::new("AB").style(if self.search_preserve_case {
                        herdr_work_code::ui::selection_style(true)
                    } else {
                        Style::default().fg(colors.ignored)
                    }),
                    layout.preserve_case,
                );
                f.render_widget(
                    Paragraph::new(" ⇄ ").style(Style::default().fg(
                        if self.search_replace_pending {
                            colors.ignored
                        } else {
                            colors.accent_focus
                        },
                    )),
                    layout.replace_all,
                );
            }
            f.render_widget(
                Paragraph::new(if self.search_results_collapsed {
                    "⊞"
                } else {
                    "⊟"
                }),
                layout.collapse,
            );
            for ((rect, label), enabled) in
                layout.options.into_iter().zip(["Aa ", "ab ", ".* "]).zip([
                    self.search_options.match_case,
                    self.search_options.whole_word,
                    self.search_options.regex,
                ])
            {
                f.render_widget(
                    Paragraph::new(label).style(if enabled {
                        herdr_work_code::ui::selection_style(true)
                    } else {
                        Style::default().fg(colors.ignored)
                    }),
                    rect,
                );
            }
            let icons = search_toolbar_icons(self.icon_theme());
            for (index, (rect, icon)) in layout
                .toolbar
                .into_iter()
                .zip([icons.0, icons.1, icons.2])
                .enumerate()
            {
                let hovered = self
                    .interaction
                    .pointer
                    .is_some_and(|point| rect.contains(point));
                let style = if index == 2
                    && (self.search_filters_visible()
                        || !self.search_include.is_empty()
                        || !self.search_exclude.is_empty())
                {
                    Style::default().fg(colors.accent_focus)
                } else {
                    herdr_work_code::ui::chrome_button_style(hovered)
                };
                f.render_widget(Paragraph::new(format!(" {icon} ")).style(style), rect);
            }
            if self.search_filters_visible() {
                for (label_area, field_box, field, label, value, active) in [
                    (
                        layout.include_label,
                        layout.include_box,
                        layout.include,
                        "包含文件",
                        self.search_include.as_str(),
                        matches!(self.prompt, Some(Prompt::SearchInclude)),
                    ),
                    (
                        layout.exclude_label,
                        layout.exclude_box,
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
                    draw_field(f, field_box, active);
                    self.draw_search_field(f, field, value, "例如 *.rs, src/**", active);
                }
            }
        }
        let hint = match &self.search_state {
            _ if !self.search_quick && self.search_query.is_empty() => Some("输入搜索内容".into()),

            LoadState::Loading => Some("正在搜索…".to_string()),
            LoadState::Failed(e) => Some(format!("搜索失败：{e}")),
            _ if self.search_rows.is_empty() => Some("没有匹配结果".into()),
            _ => None,
        };
        if let Some(hint) = hint {
            f.render_widget(Paragraph::new(hint), body);
            return;
        }
        if !self.search_quick {
            let rows = self.search_display_rows(body.width);
            let offset = self.search_display_offset(&rows, body.height);
            for (y, (target, line)) in rows
                .iter()
                .skip(offset)
                .take(usize::from(body.height))
                .enumerate()
            {
                let selected = match target {
                    SearchTarget::Match(i) => *i == self.selection,
                    SearchTarget::File(i) => {
                        self.search_collapsed_files
                            .contains(&self.search_rows[*i].path)
                            && self
                                .search_rows
                                .get(self.selection)
                                .is_some_and(|r| r.path == self.search_rows[*i].path)
                    }
                };
                f.render_widget(
                    Paragraph::new(line.clone()).style(if selected {
                        herdr_work_code::ui::selection_style(self.focus == Focus::Sidebar)
                    } else {
                        Style::default()
                    }),
                    Rect::new(body.x, body.y + y as u16, body.width, 1),
                );
                if self.search_replace_visible
                    && body.width >= 8
                    && matches!(target, SearchTarget::Match(_))
                {
                    let row_rect = Rect::new(body.x, body.y + y as u16, body.width, 1);
                    if selected
                        || self
                            .interaction
                            .pointer
                            .is_some_and(|p| row_rect.contains(p))
                    {
                        f.render_widget(
                            Paragraph::new(" ↪ ")
                                .style(herdr_work_code::ui::chrome_button_style(true)),
                            Rect::new(body.right() - 3, row_rect.y, 3, 1),
                        );
                    }
                }
            }
            herdr_work_code::ui::draw_scrollbar(
                f,
                body,
                rows.len(),
                usize::from(body.height),
                offset,
            );
        } else {
            let offset = crate::layout::offset(self.selection, body.height);
            let rows = self
                .search_rows
                .iter()
                .enumerate()
                .skip(offset)
                .map(|(i, row)| {
                    ListItem::new(row.label.clone()).style(if i == self.selection {
                        herdr_work_code::ui::selection_style(self.focus == Focus::Sidebar)
                    } else {
                        Style::default()
                    })
                })
                .collect::<Vec<_>>();
            f.render_widget(List::new(rows), body);
        }
    }

    fn search_display_rows(&self, width: u16) -> Vec<(SearchTarget, ratatui::text::Line<'static>)> {
        use herdr_work_code::search_widgets::{highlighted_search_context, truncate_path_tail};
        use ratatui::{
            style::Modifier,
            text::{Line, Span},
        };
        let mut rows = vec![];
        let mut first = 0;
        while first < self.search_rows.len() {
            let row = &self.search_rows[first];
            let end = self.search_rows[first..]
                .iter()
                .position(|r| r.path != row.path)
                .map_or(self.search_rows.len(), |i| first + i);
            let collapsed = self.search_collapsed_files.contains(&row.path);
            let count = format!(" {}", end - first);
            rows.push((
                SearchTarget::File(first),
                Line::from(vec![
                    Span::raw(if collapsed { "▸ " } else { "▾ " }),
                    Span::styled(
                        truncate_path_tail(
                            &row.label,
                            usize::from(width).saturating_sub(2 + count.len()),
                        ),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        count,
                        Style::default().fg(herdr_work_code::ui::palette().ignored),
                    ),
                ]),
            ));
            if !collapsed {
                for (index, row) in self.search_rows.iter().enumerate().take(end).skip(first) {
                    let prefix = format!("  {}: ", row.line);
                    let available = usize::from(width).saturating_sub(Span::raw(&prefix).width());
                    let mut spans = vec![Span::styled(
                        prefix,
                        Style::default().fg(herdr_work_code::ui::palette().ignored),
                    )];
                    spans.extend(highlighted_search_context(
                        &row.context,
                        &row.matches,
                        available,
                        Style::default()
                            .fg(herdr_work_code::ui::palette().header_accent)
                            .add_modifier(Modifier::BOLD),
                    ));
                    rows.push((SearchTarget::Match(index), Line::from(spans)));
                }
            }
            first = end;
        }
        rows
    }
    fn search_display_offset(
        &self,
        rows: &[(SearchTarget, ratatui::text::Line<'static>)],
        height: u16,
    ) -> usize {
        let selected = rows.iter().position(|(target,_)| matches!(target, SearchTarget::Match(i) if *i == self.selection)).or_else(|| rows.iter().position(|(target,_)| matches!(target, SearchTarget::File(i) if self.search_rows.get(self.selection).is_some_and(|r| r.path == self.search_rows[*i].path)))).unwrap_or(0);
        crate::layout::offset(selected, height).min(rows.len().saturating_sub(usize::from(height)))
    }
    pub(crate) fn search_result_click(&mut self, body: Rect, point: Position) -> Result<()> {
        let rows = self.search_display_rows(body.width);
        let offset = self.search_display_offset(&rows, body.height);
        match rows
            .get(offset + usize::from(point.y.saturating_sub(body.y)))
            .map(|r| &r.0)
        {
            Some(SearchTarget::File(index)) => {
                let path = self.search_rows[*index].path.clone();
                if !self.search_collapsed_files.remove(&path) {
                    self.search_collapsed_files.insert(path);
                }
                self.search_results_collapsed = self
                    .search_rows
                    .iter()
                    .all(|r| self.search_collapsed_files.contains(&r.path));
            }
            Some(SearchTarget::Match(index)) => {
                self.selection = *index;
                if self.search_replace_visible && point.x >= body.right().saturating_sub(3) {
                    self.prepare_search_replacement(false)?;
                } else {
                    self.open_search_result()?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum SearchTarget {
    File(usize),
    Match(usize),
}
