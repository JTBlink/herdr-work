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
impl HerdrWork {
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
            let (files, truncated) =
                search::collect_quick_files(&root, show_hidden, search::QUICK_OPEN_FILE_LIMIT);
            let rows = search::quick_matches(&files, query)
                .into_iter()
                .take(1000)
                .map(|i| SearchRow {
                    path: files[i].path.clone(),
                    label: files[i].label.clone(),
                    line: 1,
                    context: String::new(),
                })
                .collect();
            Ok(SearchView { rows, truncated })
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
