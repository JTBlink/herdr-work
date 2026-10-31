//! Git's graph topology with structured commit columns. No terminal or I/O ownership.
use crate::{ansi, ui::palette};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub oid: String,
    pub short: String,
    pub refs: Vec<String>,
    pub subject: String,
    pub date: String,
    pub author: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub graph: String,
    pub commit: Option<Commit>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    pub rows: Vec<Row>,
}

fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

impl Graph {
    pub(crate) fn parse(output: &str) -> Result<Self, String> {
        let mut rows = Vec::new();
        for line in output.lines().filter(|line| !line.trim().is_empty()) {
            let mut fields = line.split('\0');
            let graph = fields.next().unwrap_or_default().to_owned();
            let commit = if let Some(oid) = fields.next() {
                if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("Git graph 返回了无效提交号".into());
                }
                let short = fields.next().ok_or("Git graph 缺少短提交号")?;
                let refs = fields.next().ok_or("Git graph 缺少引用")?;
                let subject = fields.next().ok_or("Git graph 缺少提交说明")?;
                let date = fields.next().ok_or("Git graph 缺少日期")?;
                let author = fields.next().ok_or("Git graph 缺少作者")?;
                Some(Commit {
                    oid: oid.into(),
                    short: clean(short),
                    refs: refs
                        .split(", ")
                        .filter(|r| !r.is_empty())
                        .map(clean)
                        .collect(),
                    subject: clean(subject),
                    date: clean(date),
                    author: clean(author),
                })
            } else {
                None
            };
            rows.push(Row { graph, commit });
        }
        Ok(Self { rows })
    }

    pub fn commit(&self, row: usize) -> Option<&Commit> {
        self.rows.get(row)?.commit.as_ref()
    }

    /// Preserve the existing drawer contract: the commit ID precedes user text.
    pub fn legacy_lines(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|row| match &row.commit {
                Some(c) => format!(
                    "{} {} {} {} · {} · {}",
                    row.graph,
                    c.short,
                    c.refs.join(", "),
                    c.subject,
                    c.date,
                    c.author
                ),
                None => row.graph.clone(),
            })
            .collect()
    }

    pub fn header(&self, width: u16) -> Line<'static> {
        let cols = Columns::new(width, self.graph_width());
        let mut spans = vec![
            Span::raw(fit("Graph", cols.graph)),
            Span::raw(fit("提交说明", cols.description)),
        ];
        for (label, size) in [
            ("日期", cols.date),
            ("作者", cols.author),
            ("Commit", cols.sha),
        ] {
            if size > 0 {
                spans.push(Span::raw(fit(label, size)));
            }
        }
        Line::from(spans).style(Style::default().fg(palette().header_accent).bold())
    }

    pub fn lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.rows.is_empty() {
            return vec![Line::raw("暂无提交")];
        }
        let cols = Columns::new(width, self.graph_width());
        self.rows
            .iter()
            .map(|row| {
                let mut spans =
                    clip_spans(topology_spans(&row.graph), cols.graph.saturating_sub(1));
                let used: usize = spans.iter().map(Span::width).sum();
                spans.push(Span::raw(" ".repeat(cols.graph.saturating_sub(used))));
                if let Some(c) = &row.commit {
                    let mut description = Vec::new();
                    for reference in &c.refs {
                        let (label, color) = ref_label(reference);
                        description.push(Span::styled(
                            format!(" {label} "),
                            Style::default()
                                .fg(color)
                                .bg(palette().selection_unfocused_bg)
                                .bold(),
                        ));
                        description.push(Span::raw(" "));
                    }
                    description.push(Span::raw(c.subject.clone()));
                    // Long ref names must leave space for the subject itself.
                    let refs_width: usize = description
                        .iter()
                        .take(description.len().saturating_sub(1))
                        .map(Span::width)
                        .sum();
                    if refs_width > cols.description / 2 && !c.refs.is_empty() {
                        let mut refs = clip_spans(
                            description[..description.len() - 1].to_vec(),
                            cols.description / 2,
                        );
                        refs.push(Span::raw(" "));
                        refs.push(Span::raw(c.subject.clone()));
                        description = refs;
                    }
                    let description = clip_spans(description, cols.description.saturating_sub(1));
                    let used: usize = description.iter().map(Span::width).sum();
                    spans.extend(description);
                    spans.push(Span::raw(" ".repeat(cols.description.saturating_sub(used))));
                    if cols.date > 0 {
                        spans.push(Span::styled(
                            fit(
                                if cols.date < 17 {
                                    c.date.get(..10).unwrap_or(&c.date)
                                } else {
                                    &c.date
                                },
                                cols.date,
                            ),
                            Style::default().fg(palette().ignored),
                        ));
                    }
                    if cols.author > 0 {
                        spans.push(Span::raw(fit(&c.author, cols.author)));
                    }
                    if cols.sha > 0 {
                        spans.push(Span::styled(
                            fit(&c.short, cols.sha),
                            Style::default().fg(palette().header_accent),
                        ));
                    }
                }
                Line::from(spans)
            })
            .collect()
    }

    /// One inset topology gutter shared by compact commit and file rows.
    /// Reserve the same lane width on every row so disclosure arrows align.
    pub fn compact_gutter(&self, source: &str, width: usize) -> Line<'static> {
        if width == 0 {
            return Line::default();
        }
        let lanes = self
            .rows
            .iter()
            .map(|row| ansi::to_lines(&row.graph)[0].width())
            .max()
            .unwrap_or(2)
            .max(1)
            .min((width / 3).max(1))
            .min(width - 1);
        let mut spans = vec![Span::raw(" ")];
        let graph = clip_spans(topology_spans(source), lanes);
        let used: usize = graph.iter().map(Span::width).sum();
        spans.extend(graph);
        spans.push(Span::raw(" ".repeat(lanes.saturating_sub(used))));
        Line::from(spans)
    }

    fn graph_width(&self) -> usize {
        self.rows
            .iter()
            .map(|row| ansi::to_lines(&row.graph)[0].width() + 1)
            .max()
            .unwrap_or(6)
            .clamp(6, 28)
    }
}

fn topology_spans(source: &str) -> Vec<Span<'static>> {
    let mut line = ansi::to_lines(source)
        .into_iter()
        .next()
        .unwrap_or_default();
    for span in &mut line.spans {
        span.content = span
            .content
            .replace('*', "●")
            .replace('|', "│")
            .replace('/', "╱")
            .replace('\\', "╲")
            .replace('_', "─")
            .into();
    }
    line.spans
}

fn ref_label(reference: &str) -> (String, Color) {
    let colors = palette();
    if let Some(name) = reference.strip_prefix("HEAD -> refs/heads/") {
        return (format!("● {name}"), colors.header_accent);
    }
    if let Some(name) = reference.strip_prefix("tag: refs/tags/") {
        return (format!("◇ {name}"), colors.warning);
    }
    if let Some(name) = reference.strip_prefix("refs/remotes/") {
        return (name.into(), colors.added);
    }
    (
        reference
            .strip_prefix("refs/heads/")
            .unwrap_or(reference)
            .into(),
        colors.header_accent,
    )
}

struct Columns {
    graph: usize,
    description: usize,
    date: usize,
    author: usize,
    sha: usize,
}
impl Columns {
    fn new(width: u16, graph: usize) -> Self {
        let width = usize::from(width);
        let (date, author, sha) = if width >= 100 {
            (17, 16, 10)
        } else if width >= 76 {
            (11, 12, 9)
        } else if width >= 32 {
            (0, 0, 9)
        } else {
            (0, 0, 0)
        };
        let graph = graph.min(width / 4).max(1).min(width);
        Self {
            graph,
            description: width.saturating_sub(graph + date + author + sha),
            date,
            author,
            sha,
        }
    }
}

fn fit(text: &str, width: usize) -> String {
    let clipped = clip_spans(vec![Span::raw(text.to_owned())], width.saturating_sub(1));
    let text: String = clipped.iter().map(|s| s.content.as_ref()).collect();
    let pad = width.saturating_sub(text.width());
    format!("{text}{}", " ".repeat(pad))
}

pub(crate) fn clip_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let full: usize = spans.iter().map(Span::width).sum();
    if full <= width {
        return spans;
    }
    if width == 0 {
        return vec![];
    }
    let mut result = Vec::new();
    let mut left = width - 1;
    for span in spans {
        let mut text = String::new();
        let mut stopped = false;
        for c in span.content.chars() {
            let cells = c.width().unwrap_or(0);
            if cells > left {
                stopped = true;
                break;
            }
            text.push(c);
            left -= cells;
        }
        if !text.is_empty() {
            result.push(Span::styled(text, span.style));
        }
        if left == 0 || stopped {
            break;
        }
    }
    result.push(Span::raw("…"));
    result
}
