//! Shared Sidebar SCM tree model, moved from scm_app without a second algorithm.
use crate::git::FileEntry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub expanded: bool,
    pub count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreeItem {
    Dir(usize),
    File(usize),
}

#[derive(Clone, Debug)]
struct ScmTreeNode {
    name: String,
    path: String,
    is_dir: bool,
    file_index: Option<usize>,
    children: Vec<ScmTreeNode>,
}

impl ScmTreeNode {
    fn new_dir(name: String, path: String) -> Self {
        Self {
            name,
            path,
            is_dir: true,
            file_index: None,
            children: Vec::new(),
        }
    }

    fn new_file(name: String, path: String, file_index: usize) -> Self {
        Self {
            name,
            path,
            is_dir: false,
            file_index: Some(file_index),
            children: Vec::new(),
        }
    }

    fn add_path(&mut self, components: &[&str], full_path: &str, file_index: usize) {
        if components.is_empty() {
            return;
        }
        if components.len() == 1 {
            self.children.push(ScmTreeNode::new_file(
                components[0].to_string(),
                full_path.to_string(),
                file_index,
            ));
            return;
        }
        let dir_name = components[0];
        let sub_path = if self.path.is_empty() {
            dir_name.to_string()
        } else {
            format!("{}/{}", self.path, dir_name)
        };
        if let Some(pos) = self
            .children
            .iter()
            .position(|c| c.is_dir && c.name == dir_name)
        {
            self.children[pos].add_path(&components[1..], full_path, file_index);
        } else {
            let mut new_dir = ScmTreeNode::new_dir(dir_name.to_string(), sub_path);
            new_dir.add_path(&components[1..], full_path, file_index);
            self.children.push(new_dir);
        }
    }

    fn compact(&mut self) {
        for child in &mut self.children {
            child.compact();
        }
        if self.is_dir && self.children.len() == 1 && self.children[0].is_dir {
            let only_child = self.children.remove(0);
            self.name = format!("{}/{}", self.name, only_child.name);
            self.path = only_child.path;
            self.children = only_child.children;
        }
    }

    fn sort(&mut self) {
        for child in &mut self.children {
            child.sort();
        }
        self.children.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
    }

    fn file_count(&self) -> usize {
        if !self.is_dir {
            1
        } else {
            self.children.iter().map(|c| c.file_count()).sum()
        }
    }
}

/// Drop collapsed folders that no longer exist in `files`. Existence is
/// judged against EVERY current (compacted) folder path, not the visible
/// rows: a folder hidden inside a collapsed parent still exists, and must
/// come back collapsed when the parent is expanded again.
pub fn prune_collapsed(collapsed: &mut std::collections::BTreeSet<String>, files: &[FileEntry]) {
    if files.is_empty() {
        return;
    }
    let (all, _, _) = build_tree_model(files, &std::collections::BTreeSet::new());
    let existing: std::collections::BTreeSet<&str> =
        all.iter().map(|dir| dir.path.as_str()).collect();
    collapsed.retain(|path| existing.contains(path.as_str()));
}

pub fn build_tree_model(
    files: &[FileEntry],
    collapsed_dirs: &std::collections::BTreeSet<String>,
) -> (Vec<DirEntry>, Vec<usize>, Vec<TreeItem>) {
    let mut root = ScmTreeNode::new_dir(String::new(), String::new());
    for (i, entry) in files.iter().enumerate() {
        let parts: Vec<&str> = entry.path.split('/').filter(|s| !s.is_empty()).collect();
        root.add_path(&parts, &entry.path, i);
    }
    root.sort();
    for child in &mut root.children {
        child.compact();
    }
    let mut dirs = Vec::new();
    let mut depths = vec![0; files.len()];
    let mut rows = Vec::new();
    flatten_scm_tree(
        &root.children,
        0,
        collapsed_dirs,
        &mut dirs,
        &mut depths,
        &mut rows,
    );
    (dirs, depths, rows)
}

fn flatten_scm_tree(
    nodes: &[ScmTreeNode],
    depth: usize,
    collapsed_dirs: &std::collections::BTreeSet<String>,
    dirs_out: &mut Vec<DirEntry>,
    file_depths_out: &mut [usize],
    rows_out: &mut Vec<TreeItem>,
) {
    for node in nodes {
        if node.is_dir {
            let expanded = !collapsed_dirs.contains(&node.path);
            let dir_idx = dirs_out.len();
            dirs_out.push(DirEntry {
                path: node.path.clone(),
                name: node.name.clone(),
                depth,
                expanded,
                count: node.file_count(),
            });
            rows_out.push(TreeItem::Dir(dir_idx));
            if expanded {
                flatten_scm_tree(
                    &node.children,
                    depth + 1,
                    collapsed_dirs,
                    dirs_out,
                    file_depths_out,
                    rows_out,
                );
            }
        } else if let Some(idx) = node.file_index {
            if idx < file_depths_out.len() {
                file_depths_out[idx] = depth;
            }
            rows_out.push(TreeItem::File(idx));
        }
    }
}

/// Source SCM Left navigation: the nearest preceding directory above this depth.
pub fn parent_directory(rows: &[(usize, bool)], selected: usize) -> Option<usize> {
    let depth = rows.get(selected)?.0;
    if depth == usize::MAX {
        return None;
    }
    (0..selected)
        .rev()
        .find(|&index| rows[index].1 && rows[index].0 < depth)
}
