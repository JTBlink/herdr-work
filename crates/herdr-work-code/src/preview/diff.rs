//! Full-file Git loading and a clickable overview of changes and the viewport.
use super::*;
use crate::diffview::{ChangeKind, SideBySide};

const MAX_DIFF_BYTES: u64 = (MAX_BYTES * 4) as u64;
const MAX_DIFF_LINES: usize = MAX_LINES * 2;

#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub(crate) fn load_diff(root: &Path, rel: &str, kind: &str) -> Doc {
    let mut command = std::process::Command::new("git");
    command.current_dir(root).args([
        "--literal-pathspecs",
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--unified=2147483647",
    ]);
    match kind {
        "staged" => {
            command.arg("--cached");
        }
        "untracked" => {
            command.args([
                "--no-index",
                "--",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            ]);
        }
        _ => {}
    }
    if kind != "untracked" {
        command.arg("--");
    }
    command.arg(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
    let loaded = (|| {
        let output = capture_preview_output(&mut command, None, MAX_DIFF_BYTES)?;
        if !output.status.success() && !(kind == "untracked" && output.status.code() == Some(1)) {
            return Err(format!(
                "git failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        check_lines(&text)?;
        let (lines, starts, split) = crate::diffview::render_full(rel, &text);
        if !lines.is_empty() {
            return Ok((lines, starts, split));
        }
        // A refreshed clean diff or a mode-only change still shows the file.
        // Staged views must use the index, even if the worktree has later edits.
        let bytes = if kind == "staged" {
            let mut show = std::process::Command::new("git");
            show.current_dir(root)
                .args(["show", "--no-ext-diff", "--no-textconv"])
                .arg(format!(":./{rel}"));
            let out = capture_preview_output(&mut show, None, MAX_DIFF_BYTES)?;
            if !out.status.success() {
                return Ok((vec![Line::raw("(no changes)")], vec![], None));
            }
            out.stdout
        } else {
            let file = std::fs::File::open(root.join(rel)).map_err(|error| error.to_string())?;
            let mut bytes = Vec::new();
            file.take(MAX_DIFF_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            if bytes.len() as u64 > MAX_DIFF_BYTES {
                return Err("文件过大，无法完整显示 Diff".into());
            }
            bytes
        };
        if bytes.contains(&0) {
            return Ok((vec![Line::raw("(binary file)")], vec![], None));
        }
        let text = String::from_utf8_lossy(&bytes);
        check_lines(&text)?;
        if text.is_empty() {
            return Ok((vec![Line::raw("(empty file — no changes)")], vec![], None));
        }
        let (lines, split) = crate::diffview::render_unchanged(rel, &text);
        Ok((lines, vec![], Some(split)))
    })();
    let (lines, starts, split) = loaded
        .unwrap_or_else(|error: String| (vec![Line::raw(format!("({error})"))], vec![], None));
    let what = match kind {
        "staged" => "staged",
        "untracked" => "untracked",
        _ => "working tree",
    };
    let mut doc = git_show(
        rel.rsplit('/').next().unwrap_or(rel).into(),
        format!("{} — {what} diff", root.join(rel).display()),
        "",
    );
    doc.lines = lines;
    doc.changes = Some(ChangeNavigation::new(starts));
    doc.split = split;
    doc
}

fn check_lines(text: &str) -> Result<(), String> {
    if text.lines().take(MAX_DIFF_LINES + 1).count() > MAX_DIFF_LINES {
        Err("文件过大，无法完整显示 Diff".into())
    } else {
        Ok(())
    }
}

impl Doc {
    pub(crate) fn text_area(&self, body: Rect) -> Rect {
        Rect {
            width: body
                .width
                .saturating_sub(if self.split.is_some() { 2 } else { 0 }),
            ..body
        }
    }

    pub(crate) fn draw_diff_header(&self, frame: &mut Frame, body: Rect) -> Rect {
        if self.split.is_none() || !SideBySide::wide(self.text_area(body).width) || body.height == 0
        {
            return body;
        }
        let width = self.text_area(body).width;
        let left = width.saturating_sub(1) / 2;
        frame.render_widget(
            Paragraph::new(" 原始").style(Style::default().dim()),
            Rect::new(body.x, body.y, left, 1),
        );
        frame.render_widget(
            Paragraph::new("│ 修改后").style(Style::default().dim()),
            Rect::new(body.x + left, body.y, width - left, 1),
        );
        Rect::new(body.x, body.y + 1, body.width, body.height - 1)
    }

    fn overview(&self, height: u16) -> Vec<Option<ChangeKind>> {
        let mut marks = vec![None; usize::from(height)];
        let Some(split) = &self.split else {
            return marks;
        };
        let total = self.rows.len().max(1);
        for (at, row) in self.rows.iter().enumerate() {
            if let Some(kind) = split.kind(row.src) {
                let bucket = at.saturating_mul(usize::from(height)) / total;
                if let Some(mark) = marks.get_mut(bucket) {
                    *mark = Some(match *mark {
                        Some(previous) if previous != kind => ChangeKind::Modified,
                        _ => kind,
                    });
                }
            }
        }
        marks
    }

    pub(crate) fn draw_overview(&self, frame: &mut Frame, body: Rect) {
        if self.split.is_none() || body.width < 2 || body.height == 0 {
            return;
        }
        let total = self.rows.len().max(1);
        let start = self.scroll.saturating_mul(usize::from(body.height)) / total;
        let end = (self.scroll + usize::from(body.height))
            .min(total)
            .saturating_mul(usize::from(body.height))
            .div_ceil(total)
            .max(start + 1);
        for (y, mark) in self.overview(body.height).into_iter().enumerate() {
            let (symbol, color) = match mark {
                Some(ChangeKind::Added) => ("+", palette().diff_add_mark),
                Some(ChangeKind::Removed) => ("−", palette().diff_del_mark),
                Some(ChangeKind::Modified) => ("~", palette().warning),
                None => (" ", palette().ignored),
            };
            let row = Line::from(vec![
                Span::styled(
                    if (start..end).contains(&y) {
                        "┃"
                    } else {
                        "│"
                    },
                    Style::default().fg(if (start..end).contains(&y) {
                        palette().header_accent
                    } else {
                        palette().ignored
                    }),
                ),
                Span::styled(symbol, Style::default().fg(color).bold()),
            ]);
            frame.render_widget(
                Paragraph::new(row),
                Rect::new(body.right() - 2, body.y + y as u16, 2, 1),
            );
        }
    }

    pub(super) fn overview_mouse(&mut self, mouse: &MouseEvent, body: Rect) -> bool {
        if self.split.is_none()
            || body.width < 2
            || body.height == 0
            || mouse.row < body.y
            || mouse.row >= body.bottom()
            || mouse.column < body.right() - 2
            || mouse.column >= body.right()
        {
            return false;
        }
        if matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left)
        ) {
            let bucket = usize::from(mouse.row - body.y);
            let total = self.rows.len().max(1);
            let mark = (mouse.column == body.right() - 1
                && mouse.kind == MouseEventKind::Down(MouseButton::Left))
            .then(|| {
                self.rows
                    .iter()
                    .enumerate()
                    .find(|(at, row)| {
                        at.saturating_mul(usize::from(body.height)) / total == bucket
                            && self
                                .split
                                .as_ref()
                                .and_then(|split| split.kind(row.src))
                                .is_some()
                    })
                    .map(|(at, _)| at)
            })
            .flatten();
            self.set_scroll(
                mark.unwrap_or_else(|| bucket.saturating_mul(total) / usize::from(body.height))
                    .saturating_sub(if mark.is_some() {
                        0
                    } else {
                        usize::from(body.height) / 2
                    }),
            );
            self.selection = PreviewSelection::default();
        }
        true
    }
}

#[cfg(test)]
mod tests;

impl Doc {
    fn selection_side(&self) -> Option<usize> {
        self.split
            .as_ref()
            .filter(|split| !split.fragments.is_empty())?;
        let anchor = self.selection.anchor?;
        let row = self.rows.get(anchor.row)?;
        let width = self.rows_key?.0.saturating_sub(2);
        let x: usize = row
            .line
            .spans
            .iter()
            .flat_map(|span| span.content.chars())
            .take(anchor.col)
            .map(|ch| ch.width().unwrap_or(0))
            .sum();
        Some(usize::from(x > usize::from(width.saturating_sub(1)) / 2))
    }

    pub(super) fn split_selected_text(&self, start: RenderPos, end: RenderPos) -> Option<String> {
        let side = self.selection_side()?;
        let split = self.split.as_ref()?;
        let mut result = String::new();
        let mut previous = None;
        for part in split
            .fragments
            .iter()
            .filter(|part| part.side == side && (start.row..=end.row).contains(&part.row))
        {
            if previous.is_some_and(|source| source != part.source) {
                result.push('\n');
            }
            let len = part.text.chars().count();
            let from = if part.row == start.row {
                start.col.saturating_sub(part.column).min(len)
            } else {
                0
            };
            let to = if part.row == end.row {
                end.col.saturating_sub(part.column).min(len)
            } else {
                len
            };
            result.push_str(&char_slice(&part.text, from.min(to), to));
            previous = Some(part.source);
        }
        Some(result)
    }

    pub(crate) fn selected_line(&self, line: &Line<'static>, row: usize) -> Line<'static> {
        let range = self.selection_range();
        if let Some(side) = self.selection_side() {
            let Some((start, end)) = range else {
                return line.clone();
            };
            let Some(part) = self.split.as_ref().and_then(|split| {
                split
                    .fragments
                    .iter()
                    .find(|part| part.side == side && part.row == row)
            }) else {
                return line.clone();
            };
            if row < start.row || row > end.row {
                return line.clone();
            }
            let from = if row == start.row {
                start.col.max(part.column)
            } else {
                part.column
            };
            let to = if row == end.row {
                end.col.min(part.column + part.text.chars().count())
            } else {
                part.column + part.text.chars().count()
            };
            return selected_row(
                line,
                0,
                row,
                (from < to).then_some((RenderPos { row, col: from }, RenderPos { row, col: to })),
            );
        }
        selected_row(line, self.gutter(), row, range)
    }
}
