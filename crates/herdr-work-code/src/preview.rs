//! Read-only document model, loading and rendering moved from the Sidebar viewer.
//! No host panes, plugin state or terminal lifecycle are owned by this module.
use crate::{ansi, ui::palette};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::{
    io::Read as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use unicode_width::UnicodeWidthChar;
/// Preview size guards: don't slurp huge files into a pane.
pub(crate) const MAX_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_LINES: usize = 5000;

/// Body width from the most recent `draw_doc` call. Markdown loads happen on
/// a worker before layout is available, so glow uses this snapshot and the
/// event loop reloads a glow document if its actual body width differs.
pub(crate) static LAST_BODY_WIDTH: std::sync::atomic::AtomicU16 =
    std::sync::atomic::AtomicU16::new(80);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ViewState {
    pub line: usize,
    pub wrap: bool,
}

pub struct Doc {
    pub(crate) name: String,
    pub(crate) context: String,
    pub(crate) lines: Vec<Line<'static>>,
    /// File previews get a line-number gutter; diffs carry their own +/-.
    pub(crate) numbered: bool,
    /// Decoded raster media, rendered portably with true-color half blocks.
    /// Video files carry the poster frame extracted by ffmpeg.
    pub(crate) media: Option<MediaPreview>,
    /// Offset into [`Doc::rows`] — RENDERED rows, not source lines, so a
    /// wrapped line's continuations are scrolled to like anything else.
    pub(crate) scroll: usize,
    /// Long lines wrap unless the user toggles them off (`w`). Per document:
    /// a newly loaded one starts wrapped again.
    pub(crate) wrap: bool,
    /// Width glow formatted this markdown document for. A mismatch with the
    /// live body triggers one background reload instead of generic rewrap.
    pub(crate) glow_width: Option<u16>,
    /// `lines` laid out for the pane, rebuilt only when the width or the
    /// wrap toggle changes (see [`Doc::relayout`]).
    pub(crate) rows: Vec<Row>,
    /// The (width, height, wrap) `rows` was built for; `None` until first draw.
    pub(crate) rows_key: Option<(u16, u16, bool)>,
    /// Source line to scroll back to once `rows` is rebuilt — how a wrap
    /// toggle and a diff refresh keep the reader's place even though the
    /// row index underneath them changed.
    pub(crate) pending_src: Option<usize>,
    pub(crate) selection: PreviewSelection,
    /// A file preview's modified-time/size stamp, taken on the load worker
    /// BEFORE the content was read: the baseline later polls compare against.
    pub(crate) stamp: Option<FileStamp>,
    pub(crate) source_text: Option<String>,
}

pub(crate) struct MediaPreview {
    pub(crate) pixels: image::RgbaImage,
    pub(crate) source_width: u32,
    pub(crate) source_height: u32,
    pub(crate) video_poster: bool,
}

/// One rendered row of the body: the source line it came from (so scroll
/// position survives a relayout) and the styled row itself.
pub(crate) struct Row {
    pub(crate) src: usize,
    pub(crate) line: Line<'static>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RenderPos {
    pub(crate) row: usize,
    pub(crate) col: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PreviewSelection {
    pub(crate) anchor: Option<RenderPos>,
    pub(crate) cursor: Option<RenderPos>,
    pub(crate) mouse_anchor: Option<RenderPos>,
}

impl Doc {
    /// Rebuild [`Doc::rows`] for the viewport when the layout inputs changed,
    /// then honour any pending source-line scroll request.
    pub(crate) fn relayout(&mut self, width: u16, height: u16) {
        if self.rows_key != Some((width, height, self.wrap)) {
            self.rows = if let Some(media) = &self.media {
                render_media_rows(media, width, height)
            } else {
                build_rows(&self.lines, self.numbered, self.wrap, width)
            };
            self.rows_key = Some((width, height, self.wrap));
            self.selection = PreviewSelection::default();
        }
        if let Some(src) = self.pending_src.take() {
            self.scroll = self
                .rows
                .iter()
                .position(|r| r.src >= src)
                .unwrap_or(self.rows.len().saturating_sub(1));
        }
    }

    /// The source line at the top of the body — the anchor a relayout
    /// scrolls back to.
    pub(crate) fn top_src(&self) -> usize {
        self.pending_src
            .unwrap_or_else(|| self.rows.get(self.scroll).map_or(0, |r| r.src))
    }

    pub(crate) fn gutter(&self) -> usize {
        if self.numbered {
            self.lines.len().to_string().len() + 1
        } else {
            0
        }
    }

    /// True when a mouse release finishes a non-empty selection.
    pub(crate) fn on_mouse(&mut self, mouse: &MouseEvent, body: Rect) -> bool {
        if self.media.is_some() {
            return false;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(position) = self.text_position_at(body, mouse.column, mouse.row) else {
                    return false;
                };
                let extending = mouse.modifiers.contains(KeyModifiers::SHIFT);
                let anchor = if extending {
                    self.selection.anchor.unwrap_or(position)
                } else {
                    position
                };
                self.selection.anchor = Some(anchor);
                self.selection.cursor = Some(position);
                self.selection.mouse_anchor = Some(anchor);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some(anchor) = self.selection.mouse_anchor else {
                    return false;
                };
                let Some(position) = self.text_position_at(body, mouse.column, mouse.row) else {
                    return false;
                };
                self.selection.anchor = Some(anchor);
                self.selection.cursor = Some(position);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.selection.mouse_anchor.take().is_none() {
                    return false;
                }
                if let Some(position) = self.text_position_at(body, mouse.column, mouse.row) {
                    self.selection.cursor = Some(position);
                }
                return self.selection_range().is_some();
            }
            _ => {}
        }
        false
    }

    pub(crate) fn text_position_at(&self, body: Rect, column: u16, row: u16) -> Option<RenderPos> {
        if column < body.x || column >= body.right() || row < body.y || row >= body.bottom() {
            return None;
        }
        let row = self.scroll + usize::from(row - body.y);
        let rendered = self.rows.get(row)?;
        let text = row_text_without_gutter(rendered, self.gutter());
        let x = usize::from(column - body.x).saturating_sub(self.gutter());
        Some(RenderPos {
            row,
            col: col_at_display_x(&text, x),
        })
    }

    pub(crate) fn selection_range(&self) -> Option<(RenderPos, RenderPos)> {
        let anchor = self.selection.anchor?;
        let cursor = self.selection.cursor?;
        (anchor != cursor).then(|| {
            if anchor < cursor {
                (anchor, cursor)
            } else {
                (cursor, anchor)
            }
        })
    }

    pub(crate) fn select_all(&mut self) {
        if self.media.is_some() {
            return;
        }
        let Some(last) = self.rows.last() else { return };
        let last_col = row_text_without_gutter(last, self.gutter()).chars().count();
        self.selection.anchor = Some(RenderPos::default());
        self.selection.cursor = Some(RenderPos {
            row: self.rows.len() - 1,
            col: last_col,
        });
        self.selection.mouse_anchor = None;
    }

    pub(crate) fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let mut selected = String::new();
        for row_index in start.row..=end.row {
            let row = self.rows.get(row_index)?;
            let text = row_text_without_gutter(row, self.gutter());
            let from = if row_index == start.row { start.col } else { 0 };
            let to = if row_index == end.row {
                end.col
            } else {
                text.chars().count()
            };
            selected.push_str(&char_slice(&text, from, to));
            if row_index < end.row
                && self
                    .rows
                    .get(row_index + 1)
                    .is_some_and(|next| next.src != row.src)
            {
                selected.push('\n');
            }
        }
        Some(selected)
    }
}

pub(crate) fn row_text_without_gutter(row: &Row, gutter: usize) -> String {
    row.line
        .spans
        .iter()
        .flat_map(|span| span.content.chars())
        .skip(gutter)
        .collect()
}

pub(crate) fn col_at_display_x(text: &str, wanted: usize) -> usize {
    let mut x = 0;
    for (col, ch) in text.chars().enumerate() {
        let width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if x + width > wanted {
            return col;
        }
        x += width;
    }
    text.chars().count()
}

pub(crate) fn char_slice(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

pub(crate) fn selected_row(
    line: &Line<'static>,
    gutter: usize,
    row: usize,
    range: Option<(RenderPos, RenderPos)>,
) -> Line<'static> {
    let Some((start, end)) = range else {
        return line.clone();
    };
    if row < start.row || row > end.row {
        return line.clone();
    }
    let mut rendered = Line {
        style: line.style,
        ..Line::default()
    };
    let mut global_col: usize = 0;
    for span in &line.spans {
        for ch in span.content.chars() {
            let content_col = global_col.checked_sub(gutter);
            let selected = content_col.is_some_and(|col| {
                let after_start = row > start.row || col >= start.col;
                let before_end = row < end.row || col < end.col;
                after_start && before_end
            });
            let style = if selected {
                span.style.bg(palette().text_selection_bg)
            } else {
                span.style
            };
            rendered.spans.push(Span::styled(ch.to_string(), style));
            global_col += 1;
        }
    }
    rendered
}

pub(crate) fn media_pixel(pixel: image::Rgba<u8>) -> Option<(u8, u8, u8)> {
    let alpha = u16::from(pixel[3]);
    if alpha < 8 {
        return None;
    }
    let base = if crate::ui::is_light() { 255 } else { 0 };
    let blend = |channel: u8| ((u16::from(channel) * alpha + base * (255 - alpha)) / 255) as u8;
    Some((blend(pixel[0]), blend(pixel[1]), blend(pixel[2])))
}

/// Render raster media without relying on terminal-specific image protocols.
/// One `▀` cell carries an upper pixel in its foreground and a lower pixel in
/// its background, giving the pane two vertical pixels per terminal row.
pub(crate) fn render_media_rows(media: &MediaPreview, width: u16, height: u16) -> Vec<Row> {
    if width == 0 || height == 0 || media.source_width == 0 || media.source_height == 0 {
        return Vec::new();
    }
    let max_pixel_height = u32::from(height).saturating_mul(2);
    let scale = (f64::from(width) / f64::from(media.source_width))
        .min(f64::from(max_pixel_height) / f64::from(media.source_height));
    let target_width = (f64::from(media.source_width) * scale).round().max(1.0) as u32;
    let target_height = (f64::from(media.source_height) * scale).round().max(1.0) as u32;
    let pixels = image::imageops::resize(
        &media.pixels,
        target_width,
        target_height,
        image::imageops::FilterType::Triangle,
    );
    let rendered_height = target_height.div_ceil(2) as u16;
    let top_pad = height.saturating_sub(rendered_height) / 2;
    let left_pad = usize::from(width.saturating_sub(target_width as u16) / 2);
    let mut rows = Vec::with_capacity(usize::from(top_pad + rendered_height));
    for src in 0..usize::from(top_pad) {
        rows.push(Row {
            src,
            line: Line::default(),
        });
    }
    for pixel_y in (0..target_height).step_by(2) {
        let mut spans = Vec::with_capacity(target_width as usize + 1);
        if left_pad > 0 {
            spans.push(Span::raw(" ".repeat(left_pad)));
        }
        for pixel_x in 0..target_width {
            let upper = media_pixel(*pixels.get_pixel(pixel_x, pixel_y));
            let lower = (pixel_y + 1 < target_height)
                .then(|| media_pixel(*pixels.get_pixel(pixel_x, pixel_y + 1)))
                .flatten();
            let span = match (upper, lower) {
                (Some(top), Some(bottom)) => Span::styled(
                    "▀",
                    Style::default()
                        .fg(Color::Rgb(top.0, top.1, top.2))
                        .bg(Color::Rgb(bottom.0, bottom.1, bottom.2)),
                ),
                (Some(top), None) => {
                    Span::styled("▀", Style::default().fg(Color::Rgb(top.0, top.1, top.2)))
                }
                (None, Some(bottom)) => Span::styled(
                    "▄",
                    Style::default().fg(Color::Rgb(bottom.0, bottom.1, bottom.2)),
                ),
                (None, None) => Span::raw(" "),
            };
            spans.push(span);
        }
        rows.push(Row {
            src: rows.len(),
            line: Line::from(spans),
        });
    }
    rows
}

/// Lay `lines` out for a `width`-wide body: wrap each source line (when
/// wrapping is on), prefix the line-number gutter (blank on continuation
/// rows, like an editor), and pad tinted diff rows to the full width.
pub(crate) fn build_rows(
    lines: &[Line<'static>],
    numbered: bool,
    wrap: bool,
    width: u16,
) -> Vec<Row> {
    let number_width = lines.len().to_string().len();
    let gutter = if numbered { number_width + 1 } else { 0 };
    let content = usize::from(width).saturating_sub(gutter);
    let mut rows = Vec::with_capacity(lines.len());
    for (src, line) in lines.iter().enumerate() {
        let contains_tab = line.spans.iter().any(|span| span.content.contains('\t'));
        let pieces = if content > 0 && contains_tab {
            crate::wrap::wrap_line(line, if wrap { content } else { usize::MAX })
        } else if wrap && content > 0 && line.width() > content {
            crate::wrap::wrap_line(line, content)
        } else {
            vec![line.clone()]
        };
        for (i, piece) in pieces.into_iter().enumerate() {
            let mut row = piece;
            if numbered {
                // The number sits on the first row only; continuations
                // indent to the same column so the code stays aligned.
                let label = if i == 0 {
                    format!("{:>number_width$} ", src + 1)
                } else {
                    " ".repeat(gutter)
                };
                let style = row.style;
                let mut spans = vec![Span::styled(label, Style::default().dim())];
                spans.append(&mut row.spans);
                row = Line::from(spans);
                row.style = style;
            }
            // Tinted diff rows fill the full row, like an editor — every
            // row, so a wrapped change keeps its colour to the pane edge.
            if row.style.bg.is_some() {
                let pad = usize::from(width).saturating_sub(row.width());
                if pad > 0 {
                    row.spans.push(Span::raw(" ".repeat(pad)));
                }
            }
            rows.push(Row { src, line: row });
        }
    }
    rows
}

pub(crate) type FileStamp = (std::time::SystemTime, u64);

pub(crate) fn file_stamp(path: &Path) -> Option<FileStamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

pub(crate) fn apply_diff_refresh(doc: &mut Doc, mut refreshed: Doc) {
    if doc.media.is_none()
        && doc.name == refreshed.name
        && doc.context == refreshed.context
        && doc.numbered == refreshed.numbered
        && doc.lines == refreshed.lines
    {
        doc.stamp = refreshed.stamp;
        doc.source_text = refreshed.source_text;
        return;
    }
    refreshed.wrap = doc.wrap;
    refreshed.pending_src = Some(doc.top_src());
    *doc = refreshed;
}

/// `git show` with stat + patch, colored — what a click on a commit, stash,
/// tag, or branch line renders. Immutable content: no refresh loop needed.
#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub(crate) fn load_show(root: &Path, spec: &str, path: Option<&str>) -> Doc {
    let display_spec = spec.strip_prefix("refs/tags/").unwrap_or(spec);
    let args = crate::scm::show_args(spec, path);
    let output = std::process::Command::new("git")
        .args(&args)
        .current_dir(root)
        .output();
    let lines = match output {
        Err(e) => vec![Line::raw(format!("(git failed: {e})"))],
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            if text.trim().is_empty() {
                let err = String::from_utf8_lossy(&out.stderr);
                if err.trim().is_empty() {
                    vec![Line::raw("(nothing to show)")]
                } else {
                    vec![Line::raw(format!("({})", err.trim()))]
                }
            } else {
                ansi::to_lines(&text)
            }
        }
    };
    Doc {
        name: display_spec.to_string(),
        context: format!("git show {display_spec} — {}", root.display()),
        lines,
        numbered: false,
        media: None,
        scroll: 0,
        wrap: true,
        glow_width: None,
        rows: Vec::new(),
        rows_key: None,
        pending_src: None,
        selection: PreviewSelection::default(),
        stamp: None,
        source_text: None,
    }
}

/// Render markdown text via `glow`. Returns `None` when glow is not installed
/// or exits non-zero (caller falls back to syntax highlight).
///
/// Receives the already-read `text` buffer so the MAX_BYTES guard in
/// `load_file` is honoured — glow would otherwise re-read the full file.
/// Pipes via stdin (`-`) to avoid treating filenames starting with `-` as
/// flags. The first render uses the most recently drawn body width; the event
/// loop schedules one background reload if the actual body width differs.
#[cfg(test)]
pub(crate) fn glow_markdown(text: &str, width: u16) -> Option<Vec<Line<'static>>> {
    glow_markdown_outside(text, width, None)
}
fn glow_markdown_outside(
    text: &str,
    width: u16,
    root: Option<&Path>,
) -> Option<Vec<Line<'static>>> {
    let program = safe_program("glow", root)?;
    let mut command = std::process::Command::new(program);
    command
        .args(glow_args(width, crate::ui::is_light()))
        .env("CLICOLOR_FORCE", "1");
    let output = capture_preview(
        &mut command,
        Some(text.as_bytes().to_vec()),
        (MAX_BYTES * 4) as u64,
    )
    .ok()?;
    let rendered = String::from_utf8_lossy(&output);
    if rendered.trim().is_empty() {
        return None;
    }
    let mut lines = ansi::to_lines(&rendered);
    lines.truncate(MAX_LINES);
    if lines.is_empty() {
        return None;
    }
    Some(lines)
}

pub(crate) fn glow_args(width: u16, light: bool) -> Vec<String> {
    vec![
        "--style".into(),
        if light { "light" } else { "dark" }.into(),
        "--width".into(),
        width.to_string(),
        "-".into(),
    ]
}

pub(crate) fn choose_text_render<F>(
    glow_rendered: Option<Vec<Line<'static>>>,
    fallback: F,
    glow_width: u16,
) -> (Vec<Line<'static>>, bool, Option<u16>)
where
    F: FnOnce() -> Vec<Line<'static>>,
{
    match glow_rendered {
        Some(rendered) => (rendered, false, Some(glow_width)),
        None => (fallback(), true, None),
    }
}

pub(crate) const MAX_MEDIA_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) const MAX_MEDIA_PIXELS: u64 = 12_000_000;
pub(crate) const MAX_MEDIA_DIMENSION: u32 = 8192;
pub(crate) const MAX_MEDIA_ALLOC_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_VIDEO_FRAME_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const VIDEO_FRAME_TIMEOUT: Duration = Duration::from_secs(4);

pub(crate) fn has_extension(target: &Path, extensions: &[&str]) -> bool {
    target
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

pub(crate) fn is_image_file(target: &Path) -> bool {
    has_extension(
        target,
        &[
            "bmp", "gif", "ico", "jpeg", "jpg", "png", "tif", "tiff", "webp",
        ],
    )
}

pub(crate) fn is_video_file(target: &Path) -> bool {
    has_extension(
        target,
        &[
            "avi", "flv", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "webm", "wmv",
        ],
    )
}

pub(crate) fn decode_image_file(target: &Path) -> Result<image::RgbaImage, String> {
    let file_bytes = std::fs::metadata(target)
        .map_err(|error| error.to_string())?
        .len();
    if file_bytes > MAX_MEDIA_FILE_BYTES {
        return Err(format!(
            "image file is too large ({} MiB limit)",
            MAX_MEDIA_FILE_BYTES / 1024 / 1024
        ));
    }
    let (width, height) = image::image_dimensions(target).map_err(|error| error.to_string())?;
    if width > MAX_MEDIA_DIMENSION
        || height > MAX_MEDIA_DIMENSION
        || u64::from(width).saturating_mul(u64::from(height)) > MAX_MEDIA_PIXELS
    {
        return Err(format!("image is too large ({width}×{height})"));
    }
    let mut reader = image::ImageReader::open(target)
        .map_err(|error| error.to_string())?
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_MEDIA_DIMENSION);
    limits.max_image_height = Some(MAX_MEDIA_DIMENSION);
    limits.max_alloc = Some(MAX_MEDIA_ALLOC_BYTES);
    reader.limits(limits);
    reader
        .decode()
        .map(|image| image.to_rgba8())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
pub(crate) fn ffmpeg_in_path(path: &std::ffi::OsStr, cwd: Option<&Path>) -> Option<PathBuf> {
    program_in_path(path, cwd, "ffmpeg")
}
fn program_in_path(path: &std::ffi::OsStr, cwd: Option<&Path>, name: &str) -> Option<PathBuf> {
    let executable = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::split_paths(path)
        .filter(|directory| directory.is_absolute())
        .filter_map(|directory| directory.canonicalize().ok())
        .filter(|directory| !cwd.is_some_and(|cwd| directory.starts_with(cwd)))
        .map(|directory| directory.join(&executable))
        .find(|candidate| {
            if !candidate.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                candidate
                    .metadata()
                    .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
            }
            #[cfg(not(unix))]
            true
        })
}

fn safe_program(name: &str, root: Option<&Path>) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|path| path.canonicalize().ok());
    program_in_path(&path, root.or(cwd.as_deref()), name)
        .filter(|program| !cwd.as_ref().is_some_and(|cwd| program.starts_with(cwd)))
}

/// The original poster worker's bounded pipes/deadline, also used for glow.
/// Reader threads report through channels so inherited pipes cannot stall a join.
fn capture_preview(
    command: &mut std::process::Command,
    input: Option<Vec<u8>>,
    max: u64,
) -> Result<Vec<u8>, String> {
    use std::io::Write as _;
    command
        .stdin(if input.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    if let (Some(mut stdin), Some(input)) = (child.stdin.take(), input) {
        std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
    }
    let stdout = child.stdout.take().ok_or("preview stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("preview stderr unavailable")?;
    let (out_tx, out_rx) = std::sync::mpsc::channel();
    let (err_tx, err_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.take(max + 1).read_to_end(&mut bytes);
        let _ = out_tx.send(bytes);
    });
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.take(16 * 1024).read_to_end(&mut bytes);
        let _ = err_tx.send(bytes);
    });
    let deadline = Instant::now() + VIDEO_FRAME_TIMEOUT;
    let mut output = None;
    let mut errors = None;
    let mut status = None;
    loop {
        if output.is_none() {
            output = out_rx.try_recv().ok();
        }
        if errors.is_none() {
            errors = err_rx.try_recv().ok();
        }
        if status.is_none() {
            status = child.try_wait().map_err(|error| error.to_string())?;
        }
        if status.is_some() && output.is_some() && errors.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("preview tool timed out after 4 seconds".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = output.unwrap();
    if output.len() as u64 > max {
        return Err("preview output exceeded size limit".into());
    }
    if !status.unwrap().success() {
        return Err(format!(
            "preview tool failed: {}",
            String::from_utf8_lossy(&errors.unwrap())
                .lines()
                .next()
                .unwrap_or("unknown error")
        ));
    }
    Ok(output)
}

fn decode_video_poster(target: &Path, root: Option<&Path>) -> Result<image::RgbaImage, String> {
    let ffmpeg = safe_program("ffmpeg", root)
        .ok_or_else(|| "video preview needs ffmpeg on PATH".to_string())?;
    let mut command = std::process::Command::new(ffmpeg);
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-ss",
        "0",
        "-i",
    ]);
    command.arg(target);
    command.args([
        "-frames:v",
        "1",
        "-vf",
        "scale=1280:720:force_original_aspect_ratio=decrease",
        "-f",
        "image2pipe",
        "-vcodec",
        "png",
        "pipe:1",
    ]);
    let stdout = capture_preview(&mut command, None, MAX_VIDEO_FRAME_BYTES)?;
    image::load_from_memory_with_format(&stdout, image::ImageFormat::Png)
        .map(|image| image.to_rgba8())
        .map_err(|error| format!("could not decode video frame: {error}"))
}

fn load_media_file(target: &Path, name: String, video_poster: bool, root: Option<&Path>) -> Doc {
    let result = if video_poster {
        decode_video_poster(target, root)
    } else {
        decode_image_file(target)
    };
    let (lines, media, context) = match result {
        Ok(pixels) => {
            let (source_width, source_height) = pixels.dimensions();
            let kind = if video_poster {
                "video poster"
            } else {
                "image"
            };
            (
                Vec::new(),
                Some(MediaPreview {
                    pixels,
                    source_width,
                    source_height,
                    video_poster,
                }),
                format!(
                    "{} — {source_width}×{source_height} {kind}",
                    target.display()
                ),
            )
        }
        Err(error) => (
            vec![Line::raw(format!("({error})"))],
            None,
            target.display().to_string(),
        ),
    };
    Doc {
        name,
        context,
        lines,
        numbered: false,
        media,
        scroll: 0,
        wrap: false,
        glow_width: None,
        rows: Vec::new(),
        rows_key: None,
        pending_src: None,
        selection: PreviewSelection::default(),
        stamp: None,
        source_text: None,
    }
}

#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub(crate) fn load_file(target: &Path, target_line: Option<usize>) -> Doc {
    load_file_width(
        target,
        target_line,
        LAST_BODY_WIDTH.load(std::sync::atomic::Ordering::Relaxed),
        None,
    )
}
fn load_file_width(
    target: &Path,
    target_line: Option<usize>,
    width: u16,
    root: Option<&Path>,
) -> Doc {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| target.display().to_string());
    let lower = name.to_lowercase();
    if is_image_file(target) {
        return load_media_file(target, name, false, root);
    }
    if is_video_file(target) {
        return load_media_file(target, name, true, root);
    }
    let is_markdown = lower.ends_with(".md") || lower.ends_with(".markdown");
    let mut source_text = None;
    let (lines, numbered, glow_width) = match std::fs::File::open(target).and_then(|file| {
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
        Ok(bytes)
    }) {
        Err(e) => (vec![Line::raw(format!("(unreadable: {e})"))], true, None),
        Ok(bytes) => {
            if bytes.contains(&0) {
                (
                    vec![Line::raw(format!("(binary file — {} bytes)", bytes.len()))],
                    false,
                    None,
                )
            } else {
                let truncated = bytes.len() > MAX_BYTES;
                let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_BYTES)]);
                source_text = Some(if truncated || text.lines().count() > MAX_LINES {
                    format!(
                        "{}\n… (truncated)",
                        text.lines().take(MAX_LINES).collect::<Vec<_>>().join("\n")
                    )
                } else {
                    text.to_string()
                });
                // Markdown: render via glow; fall back to syntax highlight on failure.
                let requested_glow_width = width.max(20);
                let glow_rendered = (is_markdown && target_line.is_none())
                    .then(|| glow_markdown_outside(&text, requested_glow_width, root))
                    .flatten();
                let fallback = || {
                    crate::syntax::highlight(&name, &text, MAX_LINES).unwrap_or_else(|| {
                        text.lines()
                            .take(MAX_LINES)
                            .map(|l| Line::raw(l.to_string()))
                            .collect()
                    })
                };
                let (mut lines, numbered, glow_width) =
                    choose_text_render(glow_rendered, fallback, requested_glow_width);
                if truncated || text.lines().count() > MAX_LINES {
                    lines.push(Line::raw("… (truncated)"));
                }
                if lines.is_empty() {
                    lines.push(Line::raw("(empty file)"));
                }
                (lines, numbered, glow_width)
            }
        }
    };
    Doc {
        name,
        context: target.display().to_string(),
        lines,
        numbered,
        media: None,
        scroll: 0,
        // Glow already laid out tables and paragraphs for `glow_width`.
        wrap: glow_width.is_none(),
        glow_width,
        rows: Vec::new(),
        rows_key: None,
        pending_src: target_line.map(|line| line.saturating_sub(1)),
        selection: PreviewSelection::default(),
        stamp: None,
        source_text,
    }
}

pub(crate) fn glow_needs_reload(doc: &Doc, body_width: u16) -> bool {
    let body_width = body_width.max(20);
    doc.glow_width.is_some_and(|width| width != body_width)
}

#[cfg(any(feature = "sidebar-ui", feature = "reference-ui", test))]
pub(crate) fn load_diff(root: &Path, rel: &str, kind: &str) -> Doc {
    let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
    // Plain (uncolored) diff: crate::diffview parses it and renders the
    // VS Code look — dual gutters, tinted rows, syntax-highlighted code.
    let mut args: Vec<String> = vec!["diff".into(), "--no-ext-diff".into()];
    match kind {
        "staged" => args.push("--cached".into()),
        // An untracked file has no diff; --no-index against the null device
        // renders it as one big addition, like VS Code does.
        "untracked" => {
            args.push("--no-index".into());
            args.push(if cfg!(windows) {
                "NUL".into()
            } else {
                "/dev/null".into()
            });
        }
        _ => {}
    }
    args.push("--".into());
    args.push(rel.replace('/', std::path::MAIN_SEPARATOR_STR));

    let output = std::process::Command::new("git")
        .args(&args)
        .current_dir(root)
        .output();
    let lines = match output {
        Err(e) => vec![Line::raw(format!("(git failed: {e})"))],
        Ok(out) => {
            // --no-index exits 1 when the files differ; that's not an error.
            let text = String::from_utf8_lossy(&out.stdout);
            if text.trim().is_empty() {
                let err = String::from_utf8_lossy(&out.stderr);
                if err.trim().is_empty() {
                    vec![Line::raw("(no changes)")]
                } else {
                    vec![Line::raw(format!("({})", err.trim()))]
                }
            } else {
                crate::diffview::render(rel, &text)
            }
        }
    };
    let what = match kind {
        "staged" => "staged",
        "untracked" => "untracked",
        _ => "working tree",
    };
    Doc {
        name: name.clone(),
        context: format!("{} — {what} diff", root.join(rel).display()),
        lines,
        numbered: false,
        media: None,
        scroll: 0,
        wrap: true,
        glow_width: None,
        rows: Vec::new(),
        rows_key: None,
        pending_src: None,
        selection: PreviewSelection::default(),
        stamp: None,
        source_text: None,
    }
}

impl Doc {
    pub fn view_state(&self) -> ViewState {
        ViewState {
            line: self.top_src() + 1,
            wrap: self.wrap,
        }
    }
    pub fn restore_view_state(&mut self, state: ViewState) {
        self.wrap = state.wrap;
        self.goto_line(state.line);
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn context(&self) -> &str {
        &self.context
    }
    pub fn numbered(&self) -> bool {
        self.numbered
    }
    pub fn lines(&self) -> &[Line<'static>] {
        &self.lines
    }
    pub fn plain_text(&self) -> String {
        if let Some(text) = &self.source_text {
            return text.clone();
        }
        self.lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn media_hint(&self) -> Option<&'static str> {
        self.media.as_ref().map(|media| {
            if media.video_poster {
                "视频封面"
            } else {
                "图片预览"
            }
        })
    }
    pub fn is_media(&self) -> bool {
        self.media.is_some()
    }
    pub fn scroll(&self) -> usize {
        self.scroll
    }
    pub fn stamp(&self) -> Option<(std::time::SystemTime, u64)> {
        self.stamp
    }
    pub fn set_scroll(&mut self, scroll: usize) {
        self.scroll = scroll;
    }
    pub fn goto_line(&mut self, line: usize) {
        self.pending_src = Some(line.saturating_sub(1));
    }
    pub fn toggle_wrap(&mut self) {
        self.pending_src = Some(self.top_src());
        self.wrap = !self.wrap;
    }
    pub fn wrap(&self) -> bool {
        self.wrap
    }
    pub fn refresh(&mut self, refreshed: Doc) {
        apply_diff_refresh(self, refreshed);
    }
    pub fn needs_width_reload(&self, width: u16) -> bool {
        glow_needs_reload(self, width)
    }
    pub fn prepare(&mut self, body: Rect) {
        self.relayout(body.width, body.height);
    }
    pub fn handle_mouse(&mut self, mouse: &MouseEvent, body: Rect) -> bool {
        self.relayout(body.width, body.height);
        self.on_mouse(mouse, body)
    }
    pub fn select_all_text(&mut self) {
        self.select_all();
    }
    pub fn selection_text(&self) -> Option<String> {
        self.selected_text()
    }
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    pub fn draw_body(&mut self, frame: &mut Frame, body: Rect) {
        LAST_BODY_WIDTH.store(body.width, std::sync::atomic::Ordering::Relaxed);
        self.relayout(body.width, body.height);
        self.scroll = self.scroll.min(
            self.rows
                .len()
                .saturating_sub(usize::from(body.height).max(1)),
        );
        let range = self.selection_range();
        let gutter = self.gutter();
        let text: Vec<Line> = self
            .rows
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(usize::from(body.height))
            .map(|(row, rendered)| selected_row(&rendered.line, gutter, row, range))
            .collect();
        frame.render_widget(Paragraph::new(text), body);
    }
}
/// Load on a worker; source line targets show Markdown source so anchors remain exact.
pub fn file(root: &Path, target: &Path, target_line: Option<usize>, width: u16) -> Doc {
    LAST_BODY_WIDTH.store(width.max(20), std::sync::atomic::Ordering::Relaxed);
    let stamp = file_stamp(target);
    let mut doc = load_file_width(target, target_line, width, Some(root));
    doc.stamp = stamp;
    doc
}
/// Build a styled document from the already bounded Engine Git query.
pub fn git_show(name: String, context: String, text: &str) -> Doc {
    Doc {
        name,
        context,
        lines: ansi::to_lines(text),
        numbered: false,
        media: None,
        scroll: 0,
        wrap: true,
        glow_width: None,
        rows: vec![],
        rows_key: None,
        pending_src: None,
        selection: PreviewSelection::default(),
        stamp: None,
        source_text: None,
    }
}
/// What the copy worker reports back to the event loop.
#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseCopy {
    /// herdr's `copy_on_select` is off: nothing was copied.
    Disabled,
    Copied,
    /// OSC 52 is wanted; the event loop writes it (it owns stdout).
    Osc52(String),
    Failed(String),
}

/// Decide and perform one copy. Pure apart from its arguments, so the
/// routing is testable without a clipboard.
pub(crate) fn release_copy(
    text: String,
    enabled: bool,
    prefer_osc52: bool,
    native: impl FnOnce(&str) -> std::io::Result<()>,
    fallback_osc52: bool,
) -> ReleaseCopy {
    if !enabled {
        return ReleaseCopy::Disabled;
    }
    if prefer_osc52 {
        return ReleaseCopy::Osc52(text);
    }
    match native(&text) {
        Ok(()) => ReleaseCopy::Copied,
        Err(_) if fallback_osc52 => ReleaseCopy::Osc52(text),
        Err(error) => ReleaseCopy::Failed(error.to_string()),
    }
}

/// A copy request. Automatic (mouse-release) copies honor `copy_on_select`;
/// explicit Ctrl/Cmd+C always copies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CopyRequest {
    pub(crate) text: String,
    pub(crate) automatic: bool,
}

/// Queue a request in order. A newer automatic copy replaces any automatic
/// copy still waiting — only the latest drag matters — so a stalled
/// clipboard can never build an unbounded backlog; explicit copies are kept
/// and stay ordered against automatic ones, so an older release can never
/// overwrite a newer Ctrl+C.
pub(crate) fn enqueue_copy(
    queue: &mut std::collections::VecDeque<CopyRequest>,
    request: CopyRequest,
) {
    if request.automatic {
        queue.retain(|queued| !queued.automatic);
    }
    queue.push_back(request);
}

type CopyQueue = std::sync::Arc<(
    std::sync::Mutex<std::collections::VecDeque<CopyRequest>>,
    std::sync::Condvar,
)>;

/// One worker for every preview copy, so copies stay in order and a hung
/// clipboard tool (or a config on a stalled drive) can never freeze the
/// viewer's event loop. The helper itself is time-limited in `actions`.
pub struct ReleaseCopier {
    pub(crate) queue: CopyQueue,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    automatic_setting: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub(crate) results: std::sync::mpsc::Receiver<ReleaseCopy>,
    /// Released by the event loop after it wrote an OSC 52 copy, so a later
    /// native copy cannot finish first and then be overwritten by it.
    pub(crate) written: std::sync::mpsc::Sender<()>,
}

impl ReleaseCopier {
    pub(crate) fn start_with_enabled(
        mut automatic_enabled: impl FnMut() -> bool + Send + 'static,
    ) -> Option<Self> {
        let queue: CopyQueue = std::sync::Arc::default();
        let (outbox, results) = std::sync::mpsc::channel();
        let (written, osc52_done) = std::sync::mpsc::channel::<()>();
        let worker_queue = queue.clone();
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        std::thread::Builder::new()
            .name("preview-copy".into())
            .spawn(move || {
                loop {
                    let request = {
                        let (lock, ready) = &*worker_queue;
                        let Ok(mut pending) = lock.lock() else { return };
                        loop {
                            if worker_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                                return;
                            }
                            if let Some(request) = pending.pop_front() {
                                break request;
                            }
                            let Ok(next) = ready.wait(pending) else {
                                return;
                            };
                            pending = next;
                        }
                    };
                    let enabled = !request.automatic || automatic_enabled();
                    let result = release_copy(
                        request.text,
                        enabled,
                        crate::actions::osc52_preferred(),
                        crate::actions::copy_native,
                        crate::actions::osc52_fallback_available(),
                    );
                    let wait_for_terminal = matches!(result, ReleaseCopy::Osc52(_));
                    if outbox.send(result).is_err() {
                        return;
                    }
                    if wait_for_terminal && osc52_done.recv().is_err() {
                        return;
                    }
                }
            })
            .ok()?;
        Some(Self {
            shutdown,
            automatic_setting: None,
            queue,
            results,
            written,
        })
    }

    /// Forget drags still waiting: entering edit mode starts explicit,
    /// synchronous editor copies that a late read-only copy must not follow.
    pub(crate) fn drop_waiting_automatic(&self) {
        let (lock, _) = &*self.queue;
        if let Ok(mut pending) = lock.lock() {
            pending.retain(|queued| !queued.automatic);
        }
    }

    pub(crate) fn submit(&self, request: CopyRequest) {
        let (lock, ready) = &*self.queue;
        if let Ok(mut pending) = lock.lock() {
            enqueue_copy(&mut pending, request);
            ready.notify_one();
        }
    }
}

impl ReleaseCopier {
    pub fn product(copy_on_select: bool) -> Option<Self> {
        let enabled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(copy_on_select));
        let worker_enabled = enabled.clone();
        let mut copier = Self::start_with_enabled(move || {
            worker_enabled.load(std::sync::atomic::Ordering::Relaxed)
        })?;
        copier.automatic_setting = Some(enabled);
        Some(copier)
    }
    pub fn set_copy_on_select(&self, enabled: bool) {
        if let Some(setting) = &self.automatic_setting {
            setting.store(enabled, std::sync::atomic::Ordering::Relaxed);
        }
    }
    pub fn copy(&self, text: String, automatic: bool) {
        self.submit(CopyRequest { text, automatic });
    }
    pub fn next_result(&self) -> Option<ReleaseCopy> {
        self.results.try_recv().ok()
    }
    pub fn acknowledge_terminal(&self) {
        let _ = self.written.send(());
    }
    pub fn cancel_automatic(&self) {
        self.drop_waiting_automatic();
    }
}

impl Drop for ReleaseCopier {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.queue.0.lock() {
            self.shutdown
                .store(true, std::sync::atomic::Ordering::Relaxed);
            pending.clear();
            self.queue.1.notify_all();
        }
    }
}
