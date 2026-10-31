//! Commit summary and historical file diffs reuse the ordinary document renderer.
use super::*;
use crate::history::CommitDetails;

pub fn load(root: &Path, spec: &str, parent: usize, path: Option<&str>) -> Doc {
    match crate::git::Git::discover(root).and_then(|git| CommitDetails::load(&git, spec, parent)) {
        Ok(details) => match path {
            None => summary(root, details),
            Some(path) => patch(root, &details, path),
        },
        Err(error) => git_show(
            "提交详情".into(),
            root.display().to_string(),
            &format!("读取失败：{error}\n按 r 重试"),
        ),
    }
}

pub fn summary(root: &Path, details: CommitDetails) -> Doc {
    let (lines, _) = details.lines();
    let mut doc = git_show(
        format!("Commit {}", details.short()),
        root.display().to_string(),
        "",
    );
    doc.lines = lines.into_iter().map(Line::raw).collect();
    doc.commit = Some(Box::new(details));
    doc
}

fn patch(root: &Path, details: &CommitDetails, path: &str) -> Doc {
    let mut doc = git_show(
        path.rsplit('/').next().unwrap_or(path).into(),
        format!(
            "{} › {} › {}",
            root.display(),
            details.short(),
            crate::history::display(path)
        ),
        "",
    );
    let Some(file) = details.files.iter().find(|file| file.path == path) else {
        doc.lines = vec![Line::raw("此文件不在该提交的变更中")];
        return doc;
    };
    let mut command = std::process::Command::new("git");
    command
        .current_dir(root)
        .args(details.diff_args(&["-p", "--unified=2147483647"]))
        .arg("--");
    if let Some(old) = &file.old_path {
        command.arg(old);
    }
    command.arg(path);
    let result =
        capture_preview_output(&mut command, None, (MAX_BYTES * 4) as u64).and_then(|out| {
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
            }
            let text = String::from_utf8_lossy(&out.stdout);
            if text.lines().take(MAX_LINES * 2 + 1).count() > MAX_LINES * 2 {
                return Err("文件过大，无法完整显示 Diff".into());
            }
            Ok(crate::diffview::render_full(path, &text))
        });
    match result {
        Ok((lines, starts, split)) => {
            doc.lines = if lines.is_empty() {
                vec![Line::raw(format!("{}（仅文件属性变化）", file.label()))]
            } else {
                lines
            };
            doc.changes = Some(ChangeNavigation::new(starts));
            doc.split = split;
            doc.step_change(false);
        }
        Err(e) => doc.lines = vec![Line::raw(format!("无法读取历史 Diff：{e}"))],
    }
    doc
}

impl Doc {
    pub fn commit_details(&self) -> Option<&CommitDetails> {
        self.commit.as_deref()
    }
    pub fn commit_file_source(&self, source: usize) -> Option<&crate::history::ChangedFile> {
        let detail = self.commit.as_ref()?;
        detail.files.get(source.checked_sub(detail.lines().1)?)
    }
    pub fn commit_file_at(
        &self,
        body: Rect,
        x: u16,
        y: u16,
    ) -> Option<&crate::history::ChangedFile> {
        let position = self.text_position_at(body, x, y)?;
        self.commit_file_source(self.rows.get(position.row)?.src)
    }
}
