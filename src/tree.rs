use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::home_dir;

pub struct Node {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_last: bool,
    pub guides: Vec<bool>,
    pub expanded: bool,
    pub loaded: bool,
    pub has_children: bool,
    pub is_symlink: bool,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

pub struct Tree {
    root: PathBuf,
    nodes: Vec<Node>,
    visible: Vec<usize>,
    expanded: HashSet<PathBuf>,
    show_hidden: bool,
    selected: PathBuf,
}

impl Tree {
    pub fn new(root: PathBuf, show_hidden: bool) -> Self {
        let mut tree = Self {
            root,
            nodes: Vec::new(),
            visible: Vec::new(),
            expanded: HashSet::new(),
            show_hidden,
            selected: PathBuf::new(),
        };
        tree.rebuild();
        tree
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn visible(&self) -> &[usize] {
        &self.visible
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    pub fn child_directories(&self) -> Vec<PathBuf> {
        self.nodes
            .first()
            .map(|root| {
                root.children
                    .iter()
                    .map(|&idx| self.nodes[idx].path.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn collect_directories(root: &Path, show_hidden: bool) -> Vec<PathBuf> {
        let mut out = Vec::new();
        collect_into(root, show_hidden, &mut out);
        out
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.nodes
            .iter()
            .position(|node| node.depth > 0 && node.path == self.selected)
    }

    pub fn selected_node(&self) -> Option<&Node> {
        self.selected_index().map(|idx| &self.nodes[idx])
    }

    pub fn selected_path(&self) -> Option<&Path> {
        self.selected_node().map(|node| node.path.as_path())
    }

    pub fn selected_pos(&self) -> Option<usize> {
        self.visible
            .iter()
            .position(|&idx| self.nodes[idx].path == self.selected)
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.rebuild();
    }

    pub fn move_by(&mut self, delta: isize) {
        let Some(pos) = self.selected_pos() else {
            self.select_first();
            return;
        };
        if self.visible.is_empty() {
            return;
        }
        let last = self.visible.len() as isize - 1;
        let next = (pos as isize + delta).clamp(0, last) as usize;
        self.selected = self.nodes[self.visible[next]].path.clone();
    }

    pub fn select_first(&mut self) {
        if let Some(&idx) = self.visible.first() {
            self.selected = self.nodes[idx].path.clone();
        }
    }

    pub fn select_last(&mut self) {
        if let Some(&idx) = self.visible.last() {
            self.selected = self.nodes[idx].path.clone();
        }
    }

    pub fn home(&mut self) {
        let Some(home) = home_dir() else {
            return;
        };
        self.root = home;
        self.selected.clear();
        self.rebuild();
    }

    pub fn reveal(&mut self, path: &Path) {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return;
        };
        if relative.as_os_str().is_empty() {
            return;
        }
        let mut current = self.root.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                continue;
            };
            current.push(name);
            self.expanded.insert(current.clone());
        }
        self.selected = path.to_path_buf();
        self.rebuild();
    }

    pub fn expand(&mut self) {
        let Some(idx) = self.selected_index() else {
            return;
        };
        let path = self.nodes[idx].path.clone();
        if !self.nodes[idx].expanded {
            self.expanded.insert(path);
            self.rebuild();
        } else if let Some(&first) = self.nodes[idx].children.first() {
            self.selected = self.nodes[first].path.clone();
        }
    }

    pub fn collapse(&mut self) {
        let Some(idx) = self.selected_index() else {
            return;
        };
        if self.nodes[idx].expanded {
            let path = self.nodes[idx].path.clone();
            self.expanded.remove(&path);
            self.rebuild();
            return;
        }
        match self.nodes[idx].parent {
            Some(parent) if parent != 0 => {
                self.selected = self.nodes[parent].path.clone();
            }
            Some(_) => self.go_up(),
            None => {}
        }
    }

    pub fn go_up(&mut self) {
        let Some(parent) = self.root.parent().map(Path::to_path_buf) else {
            return;
        };
        let previous = std::mem::replace(&mut self.root, parent);
        self.expanded.insert(previous.clone());
        self.selected = previous;
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.nodes.clear();
        self.visible.clear();
        self.nodes.push(Node {
            path: self.root.clone(),
            name: display_name(&self.root),
            depth: 0,
            is_last: false,
            guides: Vec::new(),
            expanded: true,
            loaded: false,
            has_children: false,
            is_symlink: false,
            parent: None,
            children: Vec::new(),
        });
        self.walk(0);

        let still_visible = self
            .nodes
            .iter()
            .any(|node| node.depth > 0 && node.path == self.selected);
        if !still_visible {
            self.selected = self
                .visible
                .first()
                .map(|&idx| self.nodes[idx].path.clone())
                .unwrap_or_default();
        }
    }

    fn walk(&mut self, parent: usize) {
        let parent_path = self.nodes[parent].path.clone();
        let depth = self.nodes[parent].depth + 1;
        let guides = self.nodes[parent].guides.clone();
        let parent_is_last = self.nodes[parent].is_last;
        let entries = read_dirs(&parent_path, self.show_hidden);
        let count = entries.len();

        let mut ids = Vec::with_capacity(count);
        for (position, entry) in entries.into_iter().enumerate() {
            let mut child_guides = guides.clone();
            if depth > 1 {
                child_guides.push(!parent_is_last);
            }
            ids.push(self.nodes.len());
            self.nodes.push(Node {
                expanded: self.expanded.contains(&entry.path),
                path: entry.path,
                name: entry.name,
                depth,
                is_last: position + 1 == count,
                guides: child_guides,
                loaded: false,
                has_children: false,
                is_symlink: entry.is_symlink,
                parent: Some(parent),
                children: Vec::new(),
            });
        }

        self.nodes[parent].children = ids.clone();
        self.nodes[parent].loaded = true;
        self.nodes[parent].has_children = !ids.is_empty();

        for id in ids {
            let expanded = self.nodes[id].expanded;
            self.visible.push(id);
            if expanded {
                self.walk(id);
            }
        }
    }
}

fn collect_into(dir: &Path, show_hidden: bool, out: &mut Vec<PathBuf>) {
    for entry in read_dirs(dir, show_hidden) {
        out.push(entry.path.clone());
        if !entry.is_symlink {
            collect_into(&entry.path, show_hidden, out);
        }
    }
}

struct Entry {
    path: PathBuf,
    name: String,
    is_symlink: bool,
}

fn read_dirs(dir: &Path, show_hidden: bool) -> Vec<Entry> {
    let Ok(reader) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for dir_entry in reader.flatten() {
        let raw_name = dir_entry.file_name();
        let name: String = raw_name
            .to_string_lossy()
            .chars()
            .map(|ch| if ch.is_control() { '?' } else { ch })
            .collect();
        if name.is_empty() || (!show_hidden && name.starts_with('.')) {
            continue;
        }
        let path = dir_entry.path();
        let file_type = dir_entry.file_type().ok();
        let is_symlink = file_type.is_some_and(|kind| kind.is_symlink());
        let is_dir = match file_type {
            Some(kind) if kind.is_dir() => true,
            Some(kind) if kind.is_symlink() => fs::metadata(&path).is_ok_and(|meta| meta.is_dir()),
            _ => false,
        };
        if is_dir {
            entries.push(Entry {
                path,
                name,
                is_symlink,
            });
        }
    }
    entries.sort_by_cached_key(|entry| entry.name.to_lowercase());
    entries
}

fn display_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("box2cell-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("alpha/nested")).unwrap();
        fs::create_dir_all(dir.join("beta")).unwrap();
        fs::create_dir_all(dir.join(".hidden")).unwrap();
        fs::write(dir.join("file.txt"), "x").unwrap();
        dir
    }

    #[test]
    fn lists_only_directories_sorted() {
        let dir = sandbox("sort");
        let tree = Tree::new(dir.clone(), false);
        let names: Vec<&str> = tree
            .visible()
            .iter()
            .map(|&idx| tree.nodes()[idx].name.as_str())
            .collect();
        assert_eq!(names, ["alpha", "beta"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn expanding_reveals_children() {
        let dir = sandbox("expand");
        let mut tree = Tree::new(dir.clone(), false);
        tree.expand();
        let names: Vec<&str> = tree
            .visible()
            .iter()
            .map(|&idx| tree.nodes()[idx].name.as_str())
            .collect();
        assert_eq!(names, ["alpha", "nested", "beta"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reveal_expands_the_branch_and_selects_the_path() {
        let dir = sandbox("reveal");
        let mut tree = Tree::new(dir.clone(), false);
        tree.reveal(&dir.join("alpha/nested"));
        assert_eq!(
            tree.selected_path(),
            Some(dir.join("alpha/nested").as_path())
        );
        let names: Vec<&str> = tree
            .visible()
            .iter()
            .map(|&idx| tree.nodes()[idx].name.as_str())
            .collect();
        assert_eq!(names, ["alpha", "nested", "beta"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn going_up_reroots_and_selects_previous_root() {
        let dir = sandbox("up");
        let mut tree = Tree::new(dir.join("alpha"), false);
        tree.go_up();
        assert_eq!(tree.selected_path(), Some(dir.join("alpha").as_path()));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn collects_nested_directories_for_recursive_search() {
        let dir = sandbox("collect");
        let all = Tree::collect_directories(&dir, false);
        assert!(all.contains(&dir.join("alpha")));
        assert!(all.contains(&dir.join("alpha/nested")));
        assert!(all.contains(&dir.join("beta")));
        assert!(!all.contains(&dir.join(".hidden")));
        assert!(!all.contains(&dir.join("file.txt")));
        let hidden = Tree::collect_directories(&dir, true);
        assert!(hidden.contains(&dir.join(".hidden")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn child_directories_lists_only_the_first_level() {
        let dir = sandbox("children");
        let tree = Tree::new(dir.clone(), false);
        assert_eq!(
            tree.child_directories(),
            vec![dir.join("alpha"), dir.join("beta")]
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn hidden_toggle_adds_dot_directories() {
        let dir = sandbox("hidden");
        let mut tree = Tree::new(dir.clone(), false);
        tree.toggle_hidden();
        let names: Vec<&str> = tree
            .visible()
            .iter()
            .map(|&idx| tree.nodes()[idx].name.as_str())
            .collect();
        assert!(names.contains(&".hidden"));
        let _ = fs::remove_dir_all(dir);
    }
}
