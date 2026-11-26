pub mod watcher;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use self::watcher::ProjectWatcher;

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub loaded: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new(path: PathBuf, is_dir: bool) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.display().to_string());
        Self {
            name,
            path,
            is_dir,
            expanded: false,
            loaded: false,
            children: Vec::new(),
        }
    }
}

/// Lazy-loaded project scaffolding explorer.
/// Only reads directories that are explicitly expanded, respecting `.gitignore`.
pub struct ProjectTree {
    pub root: PathBuf,
    pub root_name: String,
    pub children: Vec<TreeNode>,
    pub show_hidden: bool,
    pub filter_query: String,
    pub watcher: ProjectWatcher,
}

impl ProjectTree {
    pub fn new(root: PathBuf) -> Self {
        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| root.display().to_string());
        let mut tree = Self {
            root: root.clone(),
            root_name,
            children: Vec::new(),
            show_hidden: false,
            filter_query: String::new(),
            watcher: ProjectWatcher::new(),
        };
        tree.watcher.watch_dir(&root);
        tree.children = read_dir_entries(&root, tree.show_hidden);
        tree
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.watcher.clear();
        self.root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| root.display().to_string());
        self.root = root.clone();
        self.watcher.watch_dir(&root);
        self.children = read_dir_entries(&root, self.show_hidden);
    }

    pub fn refresh(&mut self) {
        let show_hidden = self.show_hidden;
        refresh_level(&self.root, &mut self.children, show_hidden, &mut self.watcher);
    }

    pub fn toggle_node_at(
        node: &mut TreeNode,
        show_hidden: bool,
        watcher: &mut ProjectWatcher,
    ) {
        if !node.is_dir {
            return;
        }
        node.expanded = !node.expanded;
        if node.expanded {
            if !node.loaded {
                node.children = read_dir_entries(&node.path, show_hidden);
                node.loaded = true;
            }
            watcher.watch_dir(&node.path);
        } else {
            watcher.unwatch_dir(&node.path);
        }
    }

    pub fn create_file(&mut self, relative_or_name: &str) -> io::Result<PathBuf> {
        let target = self.root.join(relative_or_name.trim());
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if !target.exists() {
            fs::File::create(&target)?;
        }
        self.refresh();
        Ok(target)
    }

    pub fn create_dir(&mut self, relative_or_name: &str) -> io::Result<PathBuf> {
        let target = self.root.join(relative_or_name.trim());
        fs::create_dir_all(&target)?;
        self.refresh();
        Ok(target)
    }

    pub fn rename_path(&mut self, old_path: &Path, new_name: &str) -> io::Result<PathBuf> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "El nuevo nombre no puede estar vacío",
            ));
        }
        let parent = old_path.parent().unwrap_or(&self.root);
        let new_path = parent.join(trimmed);
        fs::rename(old_path, &new_path)?;
        self.refresh();
        Ok(new_path)
    }

    /// Collects matching file paths for Quick Open (Ctrl+P), bounded to keep RAM low.
    pub fn find_files(&self, query: &str, max_results: usize) -> Vec<PathBuf> {
        let q = query.trim().to_lowercase();
        let mut results = Vec::new();
        let walker = WalkBuilder::new(&self.root)
            .hidden(!self.show_hidden)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .max_depth(Some(8))
            .build();

        for entry in walker.flatten() {
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                let path = entry.path();
                if q.is_empty() {
                    results.push(path.to_path_buf());
                } else {
                    let rel = path
                        .strip_prefix(&self.root)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_lowercase();
                    if fuzzy_match(&rel, &q) {
                        results.push(path.to_path_buf());
                    }
                }
                if results.len() >= max_results {
                    break;
                }
            }
        }
        results
    }
}

fn read_dir_entries(dir: &Path, show_hidden: bool) -> Vec<TreeNode> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    let walker = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(!show_hidden)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path == dir {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let node = TreeNode::new(path.to_path_buf(), is_dir);
        if is_dir {
            dirs.push(node);
        } else {
            files.push(node);
        }
    }

    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(files);
    dirs
}

fn refresh_level(
    dir: &Path,
    existing: &mut Vec<TreeNode>,
    show_hidden: bool,
    watcher: &mut ProjectWatcher,
) {
    let expanded_paths: std::collections::HashSet<PathBuf> = existing
        .iter()
        .filter(|n| n.is_dir && n.expanded)
        .map(|n| n.path.clone())
        .collect();

    let mut fresh = read_dir_entries(dir, show_hidden);
    for node in &mut fresh {
        if node.is_dir && expanded_paths.contains(&node.path) {
            node.expanded = true;
            node.loaded = true;
            watcher.watch_dir(&node.path);
            if let Some(old) = existing.iter_mut().find(|o| o.path == node.path) {
                node.children = std::mem::take(&mut old.children);
            }
            refresh_level(&node.path, &mut node.children, show_hidden, watcher);
        }
    }
    *existing = fresh;
}

fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    let haystack = haystack.to_lowercase();
    let needle = needle.to_lowercase();
    if haystack.contains(&needle) {
        return true;
    }
    let mut chars = needle.chars();
    let mut current = match chars.next() {
        Some(c) => c,
        None => return true,
    };
    for h in haystack.chars() {
        if h == current {
            match chars.next() {
                Some(c) => current = c,
                None => return true,
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_works() {
        assert!(fuzzy_match("src/editor/buffer.rs", "ebuf"));
        assert!(fuzzy_match("Cargo.toml", "cargo"));
        assert!(!fuzzy_match("src/main.rs", "xyz"));
    }
}
