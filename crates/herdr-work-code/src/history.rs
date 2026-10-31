//! Structured, read-only commit history shared by the sidebar and full graph.
use crate::{git::Git, git_graph::Graph};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    All,
    Current,
}
impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "全部引用",
            Self::Current => "当前分支",
        }
    }
    pub fn toggle(&mut self) {
        *self = match self {
            Self::All => Self::Current,
            Self::Current => Self::All,
        };
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: char,
    pub added: Option<usize>,
    pub removed: Option<usize>,
}
impl ChangedFile {
    pub fn compact_label(&self) -> String {
        let (directory, name) = self.path.rsplit_once('/').unwrap_or(("", &self.path));
        let name = match &self.old_path {
            Some(old) => format!("{} → {name}", old.rsplit('/').next().unwrap_or(old)),
            None => name.to_string(),
        };
        format!("{} {}  {}", self.status, display(&name), display(directory))
    }
    pub fn label(&self) -> String {
        match &self.old_path {
            Some(old) => format!("{} {} → {}", self.status, display(old), display(&self.path)),
            None => format!("{} {}", self.status, display(&self.path)),
        }
    }
    pub fn stats(&self) -> String {
        match (self.added, self.removed) {
            (Some(a), Some(r)) => format!("+{a} −{r}"),
            _ => "二进制".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitDetails {
    pub oid: String,
    pub parents: Vec<String>,
    pub parent: usize,
    pub author: String,
    pub authored: String,
    pub committer: String,
    pub committed: String,
    pub refs: String,
    pub message: String,
    pub files: Vec<ChangedFile>,
}
impl CommitDetails {
    pub fn load(git: &Git, spec: &str, parent: usize) -> Result<Self, String> {
        // Resolve first: every later command receives a full immutable object ID.
        let oid = git.raw(&[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{spec}^{{commit}}"),
        ])?;
        let oid = oid.trim();
        if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("无效提交号".into());
        }
        let metadata = git.raw(&[
            "show",
            "-s",
            "--no-show-signature",
            "--format=%H%x00%P%x00%an <%ae>%x00%aI%x00%cn <%ce>%x00%cI%x00%D%x00%B",
            oid,
            "--",
        ])?;
        let fields: Vec<_> = metadata.splitn(8, '\0').collect();
        if fields.len() != 8 {
            return Err("提交元数据不完整".into());
        }
        let parents: Vec<String> = fields[1].split_whitespace().map(str::to_owned).collect();
        if (!parents.is_empty() && parent >= parents.len()) || (parents.is_empty() && parent != 0) {
            return Err("父提交已失效".into());
        }
        let mut details = Self {
            oid: oid.into(),
            parents,
            parent,
            author: display(fields[2]),
            authored: display(fields[3]),
            committer: display(fields[4]),
            committed: display(fields[5]),
            refs: display(fields[6]),
            message: fields[7].trim_end().to_string(),
            files: vec![],
        };
        let names = details.diff(git, &["--name-status", "-z"])?;
        let mut parts = names.split('\0').filter(|part| !part.is_empty());
        while let Some(status) = parts.next() {
            let first = parts.next().ok_or("Git 文件列表不完整")?;
            let rename = matches!(status.chars().next(), Some('R' | 'C'));
            let path = if rename {
                parts.next().ok_or("Git 重命名列表不完整")?
            } else {
                first
            };
            details.files.push(ChangedFile {
                path: path.into(),
                old_path: rename.then(|| first.into()),
                status: status.chars().next().unwrap_or('M'),
                added: None,
                removed: None,
            });
        }
        let stats = details.diff(git, &["--numstat", "-z"])?;
        let mut parts = stats.split('\0').filter(|part| !part.is_empty());
        while let Some(record) = parts.next() {
            let mut columns = record.splitn(3, '\t');
            let added = columns.next().and_then(|n| n.parse().ok());
            let removed = columns.next().and_then(|n| n.parse().ok());
            let mut path = columns.next().ok_or("Git 文件统计不完整")?;
            if path.is_empty() {
                parts.next().ok_or("Git 重命名统计缺少旧路径")?;
                path = parts.next().ok_or("Git 重命名统计缺少新路径")?;
            }
            if let Some(file) = details.files.iter_mut().find(|file| file.path == path) {
                file.added = added;
                file.removed = removed;
            }
        }
        Ok(details)
    }
    fn diff(&self, git: &Git, options: &[&str]) -> Result<String, String> {
        let mut args = self.diff_args(options);
        args.push("--");
        git.raw(&args)
    }
    pub fn diff_args<'a>(&'a self, options: &[&'a str]) -> Vec<&'a str> {
        let mut args = if let Some(parent) = self.parents.get(self.parent) {
            vec![
                "--literal-pathspecs",
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-M",
                parent.as_str(),
                self.oid.as_str(),
            ]
        } else {
            vec![
                "--literal-pathspecs",
                "diff-tree",
                "--root",
                "--no-commit-id",
                "-r",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "-M",
                self.oid.as_str(),
            ]
        };
        args.extend_from_slice(options);
        args
    }
    pub fn short(&self) -> &str {
        &self.oid[..8.min(self.oid.len())]
    }
    pub fn lines(&self) -> (Vec<String>, usize) {
        let mut lines = vec![
            format!("Commit {}", self.oid),
            format!("作者：{}", self.author),
            format!("作者时间：{}", self.authored),
            format!("提交时间：{}", self.committed),
        ];
        if self.author != self.committer {
            lines.push(format!("提交者：{}", self.committer));
        }
        lines.push(format!(
            "父提交：{}",
            if self.parents.is_empty() {
                "无（首次提交）".into()
            } else {
                self.parents.join(" · ")
            }
        ));
        if self.parents.len() > 1 {
            lines.push(format!(
                "比较：第 {} 个父提交（按 P 切换）",
                self.parent + 1
            ));
        }
        if !self.refs.is_empty() {
            lines.push(format!("引用：{}", self.refs));
        }
        lines.push(String::new());
        lines.extend(self.message.lines().map(display));
        lines.push(String::new());
        let added: usize = self.files.iter().filter_map(|f| f.added).sum();
        let removed: usize = self.files.iter().filter_map(|f| f.removed).sum();
        lines.push(format!("{} 个文件  +{added} −{removed}", self.files.len()));
        if self.files.is_empty() {
            lines.push("此提交没有文件变更".into());
        }
        let first_file = lines.len();
        lines.extend(
            self.files
                .iter()
                .map(|file| format!("{}  {}", file.label(), file.stats())),
        );
        (lines, first_file)
    }
}

pub fn display(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Dropping a superseded receiver prevents A→B→A responses from changing selection.
#[derive(Default)]
pub struct Browser {
    pub root: PathBuf,
    pub graph: Graph,
    pub scope: Scope,
    pub limit: usize,
    pub selected: Option<String>,
    pub detail: Option<Result<CommitDetails, String>>,
    pub error: Option<String>,
    pub file_limit: usize,
    graph_job: Option<Receiver<Result<Graph, String>>>,
    detail_job: Option<Receiver<Result<CommitDetails, String>>>,
    detail_parent: usize,
}
impl Browser {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            limit: 30,
            file_limit: 100,
            ..Self::default()
        }
    }
    pub fn refresh(&mut self) {
        let root = self.root.clone();
        let scope = self.scope;
        let limit = self.limit;
        let (tx, rx) = mpsc::channel();
        self.error = None;
        self.graph_job = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(Git::discover(&root).and_then(|git| git.history_graph(limit, scope)));
        });
    }
    pub fn busy(&self) -> bool {
        self.graph_job.is_some() || self.detail_job.is_some()
    }
    pub fn loading(&self) -> bool {
        self.graph_job.is_some()
    }
    pub fn select(&mut self, oid: String, parent: usize) {
        if self.selected.as_ref() == Some(&oid)
            && ((self.detail_job.is_some() && self.detail_parent == parent)
                || self
                    .detail
                    .as_ref()
                    .is_some_and(|d| d.as_ref().is_ok_and(|d| d.parent == parent)))
        {
            return;
        }
        self.selected = Some(oid.clone());
        self.detail_parent = parent;
        self.detail = None;
        self.file_limit = 100;
        let root = self.root.clone();
        let (tx, rx) = mpsc::channel();
        self.detail_job = Some(rx);
        std::thread::spawn(move || {
            let _ = tx
                .send(Git::discover(&root).and_then(|git| CommitDetails::load(&git, &oid, parent)));
        });
    }
    pub fn details(&self) -> Option<&CommitDetails> {
        self.detail.as_ref()?.as_ref().ok()
    }
    pub fn continuation(&self) -> String {
        self.graph
            .rows
            .iter()
            .find(|row| row.commit.as_ref().map(|c| &c.oid) == self.selected.as_ref())
            .map(|row| {
                crate::ansi::to_lines(&row.graph)[0]
                    .to_string()
                    .replace('*', "│")
            })
            .unwrap_or_else(|| "│ ".into())
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(result) = self.graph_job.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.graph_job = None;
            changed = true;
            match result {
                Ok(graph) => {
                    if self.selected.as_ref().is_some_and(|oid| {
                        !graph
                            .rows
                            .iter()
                            .any(|r| r.commit.as_ref().is_some_and(|c| &c.oid == oid))
                    }) {
                        self.selected = None;
                        self.detail = None;
                        self.detail_job = None;
                    }
                    self.graph = graph;
                }
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(result) = self.detail_job.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.detail_job = None;
            self.detail = Some(result);
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
pub(crate) mod tests;
