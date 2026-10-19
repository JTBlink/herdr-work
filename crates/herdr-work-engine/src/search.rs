//! Adapts the original Sidebar search results without another search algorithm.
use crate::HerdrWork;
use anyhow::Result;
use herdr_work_code::search::{self, SearchOptions};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct SearchRow {
    pub path: PathBuf,
    pub label: String,
    pub line: usize,
    pub context: String,
    pub matches: Vec<(usize, usize)>,
}
pub struct SearchView {
    pub rows: Vec<SearchRow>,
    pub truncated: bool,
}
pub struct QuickIndex {
    pub root: PathBuf,
    pub show_hidden: bool,
    files: Vec<search::QuickFile>,
    truncated: bool,
}
impl QuickIndex {
    pub fn matches(&self, query: &str) -> SearchView {
        SearchView {
            rows: search::quick_matches(&self.files, query)
                .into_iter()
                .take(1000)
                .map(|i| SearchRow {
                    path: self.files[i].path.clone(),
                    label: self.files[i].label.clone(),
                    line: 1,
                    context: String::new(),
                    matches: vec![],
                })
                .collect(),
            truncated: self.truncated,
        }
    }
}
impl HerdrWork {
    pub fn quick_index(&self, root: &Path, show_hidden: bool) -> Result<QuickIndex> {
        let root = root.canonicalize()?;
        let (files, truncated) =
            search::collect_quick_files(&root, show_hidden, search::QUICK_OPEN_FILE_LIMIT);
        Ok(QuickIndex {
            root,
            show_hidden,
            files,
            truncated,
        })
    }

    pub fn search_files(
        &self,
        root: &Path,
        query: &str,
        show_hidden: bool,
        options: SearchOptions,
        include: &str,
        exclude: &str,
    ) -> Result<SearchView> {
        let root = root.canonicalize()?;
        let (hits, truncated) = search::collect_content_hits(
            &root,
            show_hidden,
            query,
            include,
            exclude,
            options,
            search::CONTENT_SEARCH_MATCH_LIMIT,
        )
        .map_err(anyhow::Error::msg)?;
        Ok(SearchView {
            rows: hits
                .into_iter()
                .map(|h| SearchRow {
                    path: h.path,
                    label: h.label,
                    line: h.line,
                    context: h.context,
                    matches: h.matches,
                })
                .collect(),
            truncated,
        })
    }
}

#[derive(Clone)]
pub struct ReplaceRequest {
    pub root: PathBuf,
    pub query: String,
    pub replacement: String,
    pub options: SearchOptions,
    pub preserve_case: bool,
    pub show_hidden: bool,
    pub include: String,
    pub exclude: String,
    /// None replaces every match; Some replaces the first match on the selected result line.
    pub selected: Option<(PathBuf, usize)>,
}
struct ReplacementFile {
    session: crate::EditorSession,
    original: Vec<u8>,
    path: PathBuf,
    after: String,
}
pub struct ReplacePlan {
    pub root: PathBuf,
    pub query: String,
    pub replacement: String,
    pub occurrences: usize,
    pub preserve_case: bool,
    identity: String,
    files: Vec<ReplacementFile>,
}
impl ReplacePlan {
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
    pub fn description(&self) -> String {
        format!(
            "{}\n将 {:?} 替换为 {:?}\n{} 个匹配，{} 个文件\n{}",
            self.root.display(),
            self.query,
            self.replacement,
            self.occurrences,
            self.files.len(),
            self.files
                .iter()
                .take(8)
                .map(|f| f
                    .path
                    .strip_prefix(&self.root)
                    .unwrap_or(&f.path)
                    .display()
                    .to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}
impl HerdrWork {
    pub fn prepare_replacement(&self, request: ReplaceRequest) -> Result<ReplacePlan> {
        use anyhow::ensure;
        let root = request.root.canonicalize()?;
        ensure!(!request.query.is_empty(), "请输入搜索内容");
        let selected = request
            .selected
            .as_ref()
            .map(|(path, line)| {
                let path = path.canonicalize()?;
                ensure!(
                    path.starts_with(&root) && *line > 0,
                    "替换目标必须属于当前目录"
                );
                Ok::<_, anyhow::Error>((path, *line))
            })
            .transpose()?;
        let view = self.search_files(
            &root,
            &request.query,
            request.show_hidden,
            request.options,
            &request.include,
            &request.exclude,
        )?;
        ensure!(!view.truncated, "结果已截断，请缩小搜索范围后再替换");
        let paths: std::collections::BTreeSet<_> = view
            .rows
            .iter()
            .filter(|row| {
                selected
                    .as_ref()
                    .is_none_or(|(path, line)| path == &row.path && *line == row.line)
            })
            .map(|row| row.path.clone())
            .collect();
        let mut plan = ReplacePlan {
            root: root.clone(),
            query: request.query.clone(),
            replacement: request.replacement.clone(),
            occurrences: 0,
            preserve_case: request.preserve_case,
            identity: Self::worktree_identity(&root)?,
            files: vec![],
        };
        let mut total = 0;
        for path in paths {
            ensure!(path.canonicalize()? == path, "文件路径已改变");
            let original = std::fs::read(&path)?;
            total += original.len();
            ensure!(
                total <= 64 * 1024 * 1024,
                "替换预览超过 64 MiB，请缩小搜索范围"
            );
            let session = self.edit_file(&root, &path.strip_prefix(&root)?.to_string_lossy())?;
            // Editor owns encoding, BOM and external-change protection.
            let (after, count) = search::replace_content(
                &session.editor.text(),
                &request.query,
                &request.replacement,
                request.options,
                request.preserve_case,
                selected.as_ref().map(|(_, line)| *line),
            )
            .map_err(anyhow::Error::msg)?;
            ensure!(
                std::fs::read(&path)? == original,
                "文件在生成预览时改变，请重试"
            );
            if count > 0 {
                plan.occurrences += count;
                plan.files.push(ReplacementFile {
                    session,
                    original,
                    path,
                    after,
                });
            }
        }
        ensure!(plan.occurrences > 0, "没有可替换的匹配");
        ensure!(
            Self::worktree_identity(&root)? == plan.identity,
            "工作目录实例已改变"
        );
        Ok(plan)
    }
    pub fn apply_replacement(&self, plan: &ReplacePlan) -> Result<String> {
        use anyhow::{Context, ensure};
        ensure!(
            Self::worktree_identity(&plan.root)? == plan.identity,
            "工作目录实例已改变，未替换"
        );
        // Validate every file before the first write. Save checks again per file;
        // a later failure reports partial progress rather than claiming rollback.
        for file in &plan.files {
            ensure!(
                file.path.canonicalize()? == file.path
                    && std::fs::read(&file.path)? == file.original,
                "文件已改变，未替换：{}",
                file.path.display()
            );
        }
        for (done, file) in plan.files.iter().enumerate() {
            let mut session = file.session.snapshot();
            session.editor.replace_text(&file.after);
            session.save().with_context(|| {
                format!(
                    "替换中止：已保存 {done}/{} 个文件，未完成 {}",
                    plan.files.len(),
                    file.path.display()
                )
            })?;
        }
        Ok(format!(
            "已替换 {} 个匹配，涉及 {} 个文件",
            plan.occurrences,
            plan.files.len()
        ))
    }
}

#[cfg(test)]
mod replacement_tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, HerdrWork, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        let desk = HerdrWork::open(
            &temp.path().join("profile"),
            Some(temp.path().join("missing-backend")),
        )
        .unwrap();
        (temp, desk, root)
    }
    fn request(root: PathBuf) -> ReplaceRequest {
        ReplaceRequest {
            root,
            query: "foo".into(),
            replacement: "bar".into(),
            options: SearchOptions::default(),
            preserve_case: true,
            show_hidden: false,
            include: "*.txt".into(),
            exclude: "skip.txt".into(),
            selected: None,
        }
    }
    #[test]
    fn preview_then_apply_preserves_bom_crlf_and_filters() {
        let (_temp, desk, root) = fixture();
        let bytes = b"\xef\xbb\xbfFoo FOO foo\r\nfoo\r\n";
        std::fs::write(root.join("a.txt"), bytes).unwrap();
        std::fs::write(root.join("skip.txt"), "foo").unwrap();
        std::fs::write(root.join("skip.rs"), "foo").unwrap();
        let plan = desk.prepare_replacement(request(root.clone())).unwrap();
        assert_eq!(plan.occurrences, 4);
        assert_eq!(plan.file_count(), 1);
        assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), bytes);
        desk.apply_replacement(&plan).unwrap();
        assert_eq!(
            std::fs::read(root.join("a.txt")).unwrap(),
            b"\xef\xbb\xbfBar BAR bar\r\nbar\r\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("skip.txt")).unwrap(),
            "foo"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("skip.rs")).unwrap(),
            "foo"
        );
    }
    #[test]
    fn external_change_in_last_file_prevents_every_write() {
        let (_temp, desk, root) = fixture();
        for name in ["a.txt", "b.txt"] {
            std::fs::write(root.join(name), "foo").unwrap();
        }
        let plan = desk.prepare_replacement(request(root.clone())).unwrap();
        std::fs::write(root.join("b.txt"), "external change").unwrap();
        assert!(desk.apply_replacement(&plan).is_err());
        assert_eq!(std::fs::read_to_string(root.join("a.txt")).unwrap(), "foo");
        assert_eq!(
            std::fs::read_to_string(root.join("b.txt")).unwrap(),
            "external change"
        );
    }
    #[test]
    fn selected_result_replaces_one_match_and_rejects_truncation() {
        let (_temp, desk, root) = fixture();
        let path = root.join("a.txt");
        std::fs::write(&path, "foo foo\nfoo\n").unwrap();
        let mut req = request(root.clone());
        req.selected = Some((path.clone(), 1));
        let plan = desk.prepare_replacement(req).unwrap();
        assert_eq!(plan.occurrences, 1);
        desk.apply_replacement(&plan).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "bar foo\nfoo\n");
        std::fs::write(&path, "foo\n".repeat(1001)).unwrap();
        assert!(
            desk.prepare_replacement(request(root))
                .err()
                .unwrap()
                .to_string()
                .contains("截断")
        );
    }
}
