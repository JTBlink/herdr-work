//! File-local find. All input belongs to this widget while it has focus;
//! preview navigation and editor commands keep their existing bindings.
use crate::preview::{Doc, RenderPos, row_text_without_gutter};
use crate::search::{SearchOptions, content_line_match_ranges};
use crate::ui::palette;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};
use regex::RegexBuilder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hit {
    start: RenderPos,
    end: RenderPos,
    side: Option<usize>,
}

struct Cache {
    query: String,
    options: SearchOptions,
    lines: Vec<String>,
    starts: Vec<Vec<(usize, usize, usize)>>,
}

#[derive(Default)]
pub(super) struct Find {
    pub(super) open: bool,
    focused: bool,
    query: String,
    cursor: usize,
    select_all: bool,
    options: SearchOptions,
    focus: usize,
    hits: Vec<Hit>,
    active: Option<usize>,
    error: bool,
    cache: Option<Cache>,
    follow: bool,
}

impl Find {
    pub(super) fn height(&self, width: u16) -> u16 {
        if !self.open {
            0
        } else if width < 58 {
            2
        } else {
            1
        }
    }

    pub(super) fn entry(header: Rect) -> Rect {
        let width = header
            .width
            .saturating_sub(3)
            .min(if header.width < 45 { 3 } else { 18 });
        Rect::new(
            header.right().saturating_sub(width),
            header.y,
            width,
            header.height.min(1),
        )
    }

    pub(super) fn accepts_paste(&self) -> bool {
        self.open && self.focused && self.focus == 0
    }

    pub(super) fn show(&mut self) {
        self.open = true;
        self.focused = true;
        self.focus = 0;
        self.cursor = self.query.chars().count();
        self.select_all = true;
    }

    pub(super) fn key(&mut self, key: KeyEvent, doc: &Doc) -> bool {
        let shortcut = (key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT))
            || key.modifiers.contains(KeyModifiers::SUPER);
        if shortcut && key.code == KeyCode::Char('f') && !doc.is_media() {
            self.show();
            return true;
        }
        if !self.open || (shortcut && key.code == KeyCode::Char('q')) {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                self.open = false;
                self.focused = false;
                return true;
            }
            KeyCode::F(3) => {
                self.step(key.modifiers.contains(KeyModifiers::SHIFT));
                return true;
            }
            _ if !self.focused => return false,
            _ => {}
        }
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                let reverse =
                    key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT);
                self.focus = (self.focus + if reverse { 6 } else { 1 }) % 7;
            }
            KeyCode::Enter => {
                if self.focus == 0 {
                    self.step(key.modifiers.contains(KeyModifiers::SHIFT));
                } else {
                    self.activate(self.focus);
                }
            }
            KeyCode::Char(' ') if self.focus != 0 => self.activate(self.focus),
            KeyCode::Char('a') if shortcut && self.focus == 0 => self.select_all = true,
            KeyCode::Char('v') if shortcut && self.focus == 0 => {
                if let Ok(text) = crate::actions::paste_from_clipboard() {
                    self.insert(&text);
                }
            }
            KeyCode::Char(c)
                if self.focus == 0 && !shortcut && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert(&c.to_string())
            }
            KeyCode::Backspace | KeyCode::Delete if self.focus == 0 => {
                if self.select_all {
                    self.query.clear();
                    self.cursor = 0;
                } else if key.code == KeyCode::Backspace && self.cursor > 0 {
                    self.cursor -= 1;
                    self.query.remove(self.byte_at(self.cursor));
                } else if key.code == KeyCode::Delete && self.cursor < self.query.chars().count() {
                    self.query.remove(self.byte_at(self.cursor));
                }
                self.select_all = false;
                self.follow = true;
            }
            KeyCode::Left if self.focus == 0 => {
                self.cursor = self.cursor.saturating_sub(1);
                self.select_all = false;
            }
            KeyCode::Right if self.focus == 0 => {
                self.cursor = (self.cursor + 1).min(self.query.chars().count());
                self.select_all = false;
            }
            KeyCode::Home if self.focus == 0 => {
                self.cursor = 0;
                self.select_all = false;
            }
            KeyCode::End if self.focus == 0 => {
                self.cursor = self.query.chars().count();
                self.select_all = false;
            }
            _ => {}
        }
        true
    }

    fn byte_at(&self, col: usize) -> usize {
        self.query
            .char_indices()
            .nth(col)
            .map_or(self.query.len(), |(at, _)| at)
    }

    pub(super) fn insert(&mut self, text: &str) {
        if !self.open || !self.focused || self.focus != 0 {
            return;
        }
        if self.select_all {
            self.query.clear();
            self.cursor = 0;
        }
        let text: String = text
            .chars()
            .filter(|c| !c.is_control())
            .take(4096usize.saturating_sub(self.query.chars().count()))
            .collect();
        self.query.insert_str(self.byte_at(self.cursor), &text);
        self.cursor += text.chars().count();
        self.select_all = false;
        self.follow = true;
    }

    fn activate(&mut self, target: usize) {
        match target {
            1 => self.options.match_case = !self.options.match_case,
            2 => self.options.whole_word = !self.options.whole_word,
            3 => self.options.regex = !self.options.regex,
            4 => self.step(true),
            5 => self.step(false),
            6 => {
                self.open = false;
                self.focused = false;
            }
            _ => {}
        }
        if (1..=3).contains(&target) {
            self.follow = true;
        }
    }

    fn step(&mut self, reverse: bool) {
        let n = self.hits.len();
        if n == 0 {
            return;
        }
        self.active = Some(match self.active {
            Some(at) if reverse => (at + n - 1) % n,
            Some(at) => (at + 1) % n,
            None if reverse => n - 1,
            None => 0,
        });
        self.follow = true;
    }

    /// Search complete source rows, so a phrase crossing a soft wrap is one
    /// hit. Coordinates use Unicode scalar columns, like preview selection.
    fn sync(&mut self, doc: &mut Doc, body: Rect) {
        let mut lines: Vec<String> = Vec::new();
        // Each logical side/source line has independent wrap fragments. Never
        // search a concatenation of the left and right panes or their padding.
        let mut starts: Vec<Vec<(usize, usize, usize)>> = Vec::new();
        let mut sides = Vec::new();
        if let Some(split) = doc
            .split
            .as_ref()
            .filter(|split| !split.fragments.is_empty())
        {
            let mut groups = std::collections::BTreeMap::new();
            for fragment in &split.fragments {
                let index = *groups
                    .entry((fragment.side, fragment.source))
                    .or_insert_with(|| {
                        lines.push(String::new());
                        starts.push(Vec::new());
                        sides.push(Some(fragment.side));
                        lines.len() - 1
                    });
                starts[index].push((fragment.row, fragment.column, lines[index].chars().count()));
                lines[index].push_str(&fragment.text);
            }
        } else {
            let mut previous = None;
            for (row_index, row) in doc.rows.iter().enumerate() {
                if previous != Some(row.src) {
                    lines.push(String::new());
                    starts.push(Vec::new());
                    sides.push(None);
                    previous = Some(row.src);
                }
                let index = lines.len() - 1;
                starts[index].push((row_index, 0, lines[index].chars().count()));
                lines[index].push_str(&row_text_without_gutter(row, doc.gutter()));
            }
        }
        let query_changed = self
            .cache
            .as_ref()
            .is_none_or(|cached| cached.query != self.query || cached.options != self.options);
        let changed = query_changed
            || self
                .cache
                .as_ref()
                .is_none_or(|cached| cached.lines != lines);
        let layout_changed = self
            .cache
            .as_ref()
            .is_none_or(|cached| cached.starts != starts);
        if changed || layout_changed {
            // Re-map positions on resize even if the reconstructed text is equal.
            let old = self
                .active
                .and_then(|at| self.hits.get(at))
                .map(|hit| hit.start);
            self.hits.clear();
            self.error = false;
            let pattern = if self.options.regex && !self.query.is_empty() {
                let query = if self.options.whole_word {
                    format!(r"\b(?:{})\b", self.query)
                } else {
                    self.query.clone()
                };
                match RegexBuilder::new(&query)
                    .case_insensitive(!self.options.match_case)
                    .build()
                {
                    Ok(pattern) => Some(pattern),
                    Err(_) => {
                        self.error = true;
                        None
                    }
                }
            } else {
                None
            };
            if !self.query.is_empty() && !self.error {
                for (src, line) in lines.iter().enumerate() {
                    let boundaries: Vec<usize> =
                        line.char_indices().map(|(byte, _)| byte).collect();
                    let ranges = if let Some(pattern) = &pattern {
                        pattern
                            .find_iter(line)
                            .map(|found| (found.start(), found.end()))
                            .collect()
                    } else {
                        content_line_match_ranges(line, &self.query, self.options, None)
                    };
                    for (from, to) in ranges {
                        let from = boundaries.partition_point(|byte| *byte < from);
                        let to = boundaries.partition_point(|byte| *byte < to);
                        let position = |col: usize, end: bool| {
                            let parts = &starts[src];
                            let at = parts
                                .partition_point(
                                    |(_, _, start)| if end { *start < col } else { *start <= col },
                                )
                                .saturating_sub(1);
                            let (row, column, start) = parts[at];
                            RenderPos {
                                row,
                                col: column + col - start,
                            }
                        };
                        let start = position(from, false);
                        self.hits.push(Hit {
                            start,
                            side: sides[src],
                            end: if from == to {
                                start
                            } else {
                                position(to, true)
                            },
                        });
                    }
                }
            }
            self.hits.sort_by_key(|hit| (hit.start, hit.end));
            if query_changed {
                self.active = self
                    .hits
                    .iter()
                    .position(|hit| hit.start.row >= doc.scroll)
                    .or_else(|| (!self.hits.is_empty()).then_some(0));
                self.follow = true;
            } else {
                self.active = (!self.hits.is_empty())
                    .then(|| self.active.unwrap_or(0).min(self.hits.len() - 1));
            }
            if old
                != self
                    .active
                    .and_then(|at| self.hits.get(at))
                    .map(|hit| hit.start)
            {
                self.follow = true;
            }
            self.cache = Some(Cache {
                query: self.query.clone(),
                options: self.options,
                lines,
                starts,
            });
        }
        if self.follow {
            if let Some(hit) = self.active.and_then(|at| self.hits.get(at)) {
                let height = usize::from(body.height).max(1);
                if hit.start.row < doc.scroll || hit.end.row >= doc.scroll + height {
                    doc.set_scroll(hit.start.row.saturating_sub(height / 3));
                }
            }
            self.follow = false;
        }
    }

    pub(super) fn draw_body(&mut self, frame: &mut Frame, doc: &mut Doc, body: Rect) {
        doc.prepare(body);
        self.sync(doc, body);
        doc.scroll = doc.scroll.min(
            doc.rows
                .len()
                .saturating_sub(usize::from(body.height).max(1)),
        );
        let gutter = doc.gutter();
        let rows: Vec<Line> = doc
            .rows
            .iter()
            .enumerate()
            .skip(doc.scroll)
            .take(usize::from(body.height))
            .map(|(row, rendered)| {
                let mut line = doc.selected_line(&rendered.line, row);
                let visible: Vec<_> = self
                    .hits
                    .iter()
                    .enumerate()
                    .filter(|(_, hit)| hit.start.row <= row && hit.end.row >= row)
                    .collect();
                let side_ranges: Vec<_> = doc
                    .split
                    .as_ref()
                    .into_iter()
                    .flat_map(|split| &split.fragments)
                    .filter(|fragment| fragment.row == row)
                    .map(|fragment| {
                        (
                            fragment.side,
                            fragment.column..fragment.column + fragment.text.chars().count(),
                        )
                    })
                    .collect();
                let mut col: usize = 0;
                let mut spans = Vec::new();
                for span in &line.spans {
                    for ch in span.content.chars() {
                        let pos = RenderPos {
                            row,
                            col: col.saturating_sub(gutter),
                        };
                        let found = if col >= gutter {
                            visible.iter().copied().find(|(_, hit)| {
                                hit.start <= pos
                                    && (pos < hit.end || hit.start == hit.end)
                                    && hit.side.is_none_or(|side| {
                                        side_ranges.iter().any(|(which, range)| {
                                            *which == side && range.contains(&pos.col)
                                        })
                                    })
                            })
                        } else {
                            None
                        };
                        let style = match found {
                            Some((at, _)) if Some(at) == self.active => span
                                .style
                                .fg(palette().button_fg)
                                .bg(palette().button_focus_bg)
                                .bold(),
                            Some(_) => span.style.bg(palette().text_selection_bg).underlined(),
                            None => span.style,
                        };
                        spans.push(Span::styled(ch.to_string(), style));
                        col += 1;
                    }
                }
                line.spans = spans;
                line
            })
            .collect();
        frame.render_widget(Paragraph::new(rows), doc.text_area(body));
        doc.draw_overview(frame, body);
    }
}

#[cfg(test)]
mod tests;
mod widget;
