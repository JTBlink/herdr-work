//! Alignment and column layout share the unified renderer's source-row anchors.
use super::Ev;
use crate::{ui::palette, wrap::wrap_line};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct SideBySide {
    pub(crate) fragments: Vec<Fragment>,
    pairs: Vec<Pair>,
    marks: BTreeMap<usize, ChangeKind>,
    digits: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Fragment {
    pub(crate) side: usize,
    pub(crate) source: usize,
    pub(crate) row: usize,
    pub(crate) column: usize,
    pub(crate) text: String,
}
#[derive(Debug, PartialEq, Eq)]
struct Pair {
    old: Option<(usize, usize)>,
    new: Option<(usize, usize)>,
}

impl SideBySide {
    pub(super) fn from_events(events: &[Ev]) -> Option<Self> {
        if events.iter().any(|e| matches!(e, Ev::Plain(_) | Ev::Hunk)) {
            return None;
        }
        let mut result = Self::default();
        let mut at = 0;
        while at < events.len() {
            if let Ev::Ctx(old, new, _) = events[at] {
                result.pairs.push(Pair {
                    old: Some((at, old)),
                    new: Some((at, new)),
                });
                at += 1;
                continue;
            }
            let begin = at;
            while matches!(events.get(at), Some(Ev::Del(..))) {
                at += 1;
            }
            let middle = at;
            while matches!(events.get(at), Some(Ev::Add(..))) {
                at += 1;
            }
            if at == begin {
                return None;
            }
            let removed = middle - begin;
            let added = at - middle;
            let kind = match (removed, added) {
                (0, _) => ChangeKind::Added,
                (_, 0) => ChangeKind::Removed,
                _ => ChangeKind::Modified,
            };
            for i in begin..at {
                result.marks.insert(i, kind);
            }
            for i in 0..removed.max(added) {
                result.pairs.push(Pair {
                    old: match events.get(begin + i).filter(|_| i < removed) {
                        Some(Ev::Del(no, _)) => Some((begin + i, *no)),
                        _ => None,
                    },
                    new: match events.get(middle + i).filter(|_| i < added) {
                        Some(Ev::Add(no, _)) => Some((middle + i, *no)),
                        _ => None,
                    },
                });
            }
        }
        let max = result
            .pairs
            .iter()
            .flat_map(|p| [p.old, p.new])
            .flatten()
            .map(|(_, no)| no)
            .max()
            .unwrap_or(1);
        result.digits = max.to_string().len().max(2);
        Some(result)
    }

    pub(crate) fn kind(&self, source: usize) -> Option<ChangeKind> {
        self.marks.get(&source).copied()
    }
    pub(crate) fn wide(width: u16) -> bool {
        width >= 72
    }

    pub(crate) fn rows(
        &mut self,
        lines: &[Line<'static>],
        width: u16,
        wrap: bool,
    ) -> Vec<(usize, Line<'static>)> {
        self.fragments.clear();
        if width == 0 {
            return Vec::new();
        }
        if !Self::wide(width) {
            return lines
                .iter()
                .enumerate()
                .flat_map(|(src, line)| {
                    let rows = if wrap {
                        wrap_line(line, usize::from(width))
                    } else {
                        vec![line.clone()]
                    };
                    rows.into_iter().map(move |line| (src, line))
                })
                .collect();
        }
        let left = usize::from(width.saturating_sub(1)) / 2;
        let right = usize::from(width) - left - 1;
        let mut rows = Vec::new();
        for pair in &self.pairs {
            let old = side_rows(pair.old, lines, left, self.digits, wrap);
            let new = side_rows(pair.new, lines, right, self.digits, wrap);
            let source = pair.old.or(pair.new).map_or(0, |(src, _)| src);
            for i in 0..old.len().max(new.len()) {
                let mut spans = side_spans(old.get(i), pair.old, lines, left);
                let right_column = spans
                    .iter()
                    .map(|span| span.content.chars().count())
                    .sum::<usize>()
                    + 1;
                for (side, content, origin, column) in [
                    (0, old.get(i), pair.old, 0),
                    (1, new.get(i), pair.new, right_column),
                ] {
                    if let (Some(content), Some((source, _))) = (content, origin) {
                        self.fragments.push(Fragment {
                            side,
                            source,
                            row: rows.len(),
                            column: column + self.digits + 3,
                            text: content
                                .spans
                                .iter()
                                .skip(1)
                                .map(|span| span.content.as_ref())
                                .collect(),
                        });
                    }
                }
                spans.push(Span::styled("│", Style::default().fg(palette().ignored)));
                spans.extend(side_spans(new.get(i), pair.new, lines, right));
                rows.push((source, Line::from(spans)));
            }
        }
        rows
    }
}

fn side_rows(
    source: Option<(usize, usize)>,
    lines: &[Line<'static>],
    width: usize,
    digits: usize,
    wrap: bool,
) -> Vec<Line<'static>> {
    let Some((src, no)) = source else {
        return vec![];
    };
    let Some(line) = lines.get(src) else {
        return vec![];
    };
    // The unified renderer owns syntax/word tints; its first two spans are gutters.
    let code = Line::from(line.spans.iter().skip(2).cloned().collect::<Vec<_>>()).style(line.style);
    let mut pieces = wrap_line(&code, width.saturating_sub(digits + 3).max(1));
    if !wrap {
        pieces.truncate(1);
    }
    pieces
        .into_iter()
        .enumerate()
        .map(|(i, mut row)| {
            let mark = if line.style.bg == Some(palette().diff_del_bg) {
                '-'
            } else if line.style.bg == Some(palette().diff_add_bg) {
                '+'
            } else {
                ' '
            };
            let label = if i == 0 {
                format!("{no:>digits$} {mark} ")
            } else {
                " ".repeat(digits + 3)
            };
            row.spans
                .insert(0, Span::styled(label, Style::default().dim()));
            row
        })
        .collect()
}

fn side_spans(
    row: Option<&Line<'static>>,
    source: Option<(usize, usize)>,
    lines: &[Line<'static>],
    width: usize,
) -> Vec<Span<'static>> {
    let style = source
        .and_then(|(src, _)| lines.get(src))
        .map_or(Style::default(), |line| line.style);
    let mut spans = row.map_or_else(Vec::new, |row| {
        row.spans
            .iter()
            .map(|span| Span::styled(span.content.clone(), style.patch(span.style)))
            .collect()
    });
    let used: usize = spans.iter().map(Span::width).sum();
    let pad = width.saturating_sub(used);
    spans.push(if source.is_none() {
        Span::styled(
            "╱".repeat(pad),
            Style::default().fg(palette().ignored).dim(),
        )
    } else {
        Span::styled(" ".repeat(pad), style)
    });
    spans
}
