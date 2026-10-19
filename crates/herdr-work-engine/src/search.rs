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
        quick: bool,
        show_hidden: bool,
        options: SearchOptions,
        include: &str,
        exclude: &str,
    ) -> Result<SearchView> {
        let root = root.canonicalize()?;
        if quick {
            Ok(self.quick_index(&root, show_hidden)?.matches(query))
        } else {
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
                    })
                    .collect(),
                truncated,
            })
        }
    }
}
