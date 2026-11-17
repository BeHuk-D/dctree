use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::Config;
use crate::theme::{THEMES, Theme};
use crate::tree::Tree;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Tree,
    Search,
    Picker,
}

pub enum Outcome {
    Select(PathBuf),
    Cancel,
}

pub struct SearchRow {
    pub path: PathBuf,
    pub name: String,
    pub guides: Vec<bool>,
    pub is_last: bool,
    pub is_match: bool,
    pub has_children: bool,
}

struct TrieNode {
    path: PathBuf,
    name: String,
    is_match: bool,
    children: Vec<TrieNode>,
}

pub struct App {
    pub tree: Tree,
    pub mode: Mode,
    pub theme_idx: usize,
    pub picker_idx: usize,
    pub first_run: bool,
    pub scroll: usize,
    pub outcome: Option<Outcome>,
    pub save_error: Option<String>,
    pub query: String,
    pub search_rows: Vec<SearchRow>,
    pub search_sel: usize,
    pub search_scroll: usize,
    pub search_all: bool,
    config: Config,
    return_theme: usize,
    theme_locked: bool,
    search_index: Option<(PathBuf, bool, Vec<PathBuf>)>,
}

impl App {
    pub fn new(
        tree: Tree,
        config: Config,
        first_run: bool,
        open_picker: bool,
        theme_override: Option<&'static Theme>,
    ) -> Self {
        let theme_idx = match theme_override {
            Some(theme) => THEMES
                .iter()
                .position(|item| item.slug == theme.slug)
                .unwrap_or(0),
            None => THEMES
                .iter()
                .position(|theme| theme.slug == config.theme)
                .unwrap_or(0),
        };
        let mode = if first_run || open_picker {
            Mode::Picker
        } else {
            Mode::Tree
        };
        Self {
            tree,
            mode,
            theme_idx,
            picker_idx: theme_idx,
            first_run,
            scroll: 0,
            outcome: None,
            save_error: None,
            query: String::new(),
            search_rows: Vec::new(),
            search_sel: 0,
            search_scroll: 0,
            search_all: false,
            config,
            return_theme: theme_idx,
            theme_locked: theme_override.is_some(),
            search_index: None,
        }
    }

    pub fn theme(&self) -> &'static Theme {
        match self.mode {
            Mode::Picker => &THEMES[self.picker_idx],
            Mode::Tree | Mode::Search => &THEMES[self.theme_idx],
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('c')) {
            self.outcome = Some(Outcome::Cancel);
            return;
        }
        match self.mode {
            Mode::Tree => self.tree_key(key),
            Mode::Search => self.search_key(key),
            Mode::Picker => self.picker_key(key.code),
        }
    }

    fn tree_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::ALT) {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('t') => self.open_picker(),
                KeyCode::Char('h') => self.toggle_hidden(),
                KeyCode::Char('a') => {
                    self.tree.home();
                    self.search_index = None;
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => self.outcome = Some(Outcome::Cancel),
            KeyCode::Up => self.tree.move_by(-1),
            KeyCode::Down => self.tree.move_by(1),
            KeyCode::PageUp => self.tree.move_by(-10),
            KeyCode::PageDown => self.tree.move_by(10),
            KeyCode::Home => self.tree.select_first(),
            KeyCode::End => self.tree.select_last(),
            KeyCode::Right => {
                self.tree.expand();
                if let Some(position) = self.tree.selected_pos() {
                    self.scroll = position;
                }
            }
            KeyCode::Left => self.tree.collapse(),
            KeyCode::Backspace => {
                self.tree.go_up();
                self.search_index = None;
            }
            KeyCode::Enter => {
                if let Some(path) = self.tree.selected_path() {
                    self.outcome = Some(Outcome::Select(path.to_path_buf()));
                }
            }
            KeyCode::Char(ch) => self.start_search(ch),
            _ => {}
        }
    }

    fn search_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::ALT) {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('u') => self.close_search(),
                KeyCode::Char('h') => self.toggle_hidden(),
                KeyCode::Char('a') => {
                    self.tree.home();
                    self.search_index = None;
                    self.refresh_search();
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => self.close_search(),
            KeyCode::Right => self.search_open(),
            KeyCode::Left => self.search_parent(),
            KeyCode::Up => self.search_move(-1),
            KeyCode::Down => self.search_move(1),
            KeyCode::PageUp => self.search_move(-10),
            KeyCode::PageDown => self.search_move(10),
            KeyCode::Home => self.search_select(0),
            KeyCode::End => {
                let last = self.search_rows.len().saturating_sub(1);
                self.search_select(last);
            }
            KeyCode::Backspace => {
                self.query.pop();
                if self.query.is_empty() {
                    self.close_search();
                } else {
                    self.refresh_search();
                }
            }
            KeyCode::Enter => {
                if let Some(row) = self.search_rows.get(self.search_sel) {
                    self.outcome = Some(Outcome::Select(row.path.clone()));
                }
            }
            KeyCode::Char(ch) => {
                self.query.push(ch);
                self.refresh_search();
            }
            _ => {}
        }
    }

    fn search_parent(&mut self) {
        let Some(row) = self.search_rows.get(self.search_sel) else {
            return;
        };
        let Some(parent_guides) = row.guides.len().checked_sub(1) else {
            return;
        };
        if let Some(index) = (0..self.search_sel)
            .rev()
            .find(|&index| self.search_rows[index].guides.len() == parent_guides)
        {
            self.search_sel = index;
        }
    }

    fn search_open(&mut self) {
        let Some(row) = self.search_rows.get(self.search_sel) else {
            return;
        };
        if row.has_children {
            self.search_sel += 1;
        } else {
            self.close_search();
        }
    }

    fn picker_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.cancel_picker(),
            KeyCode::Up | KeyCode::Char('k') => self.picker_move(-1),
            KeyCode::Down | KeyCode::Char('j') => self.picker_move(1),
            KeyCode::Home | KeyCode::Char('g') => self.picker_idx = 0,
            KeyCode::End | KeyCode::Char('G') => self.picker_idx = THEMES.len() - 1,
            KeyCode::Enter => self.confirm_picker(),
            _ => {}
        }
    }

    fn start_search(&mut self, ch: char) {
        self.query.clear();
        self.query.push(ch);
        self.mode = Mode::Search;
        self.refresh_search();
    }

    fn close_search(&mut self) {
        let reveal = self
            .search_rows
            .get(self.search_sel)
            .map(|row| row.path.clone());
        self.query.clear();
        self.search_rows.clear();
        self.search_sel = 0;
        self.search_scroll = 0;
        self.mode = Mode::Tree;
        if let Some(path) = reveal {
            self.tree.reveal(&path);
            if let Some(position) = self.tree.selected_pos() {
                self.scroll = position;
            }
        }
    }

    fn refresh_search(&mut self) {
        let query = self.query.to_lowercase();
        let root = self.tree.root().to_path_buf();
        let paths: Vec<PathBuf> = if self.search_all {
            self.ensure_index();
            match &self.search_index {
                Some((_, _, dirs)) => dirs
                    .iter()
                    .filter(|path| name_matches(path, &query))
                    .cloned()
                    .collect(),
                None => Vec::new(),
            }
        } else {
            self.tree
                .child_directories()
                .into_iter()
                .filter(|path| name_matches(path, &query))
                .collect()
        };

        let mut trie = TrieNode {
            path: root,
            name: String::new(),
            is_match: false,
            children: Vec::new(),
        };
        for path in paths {
            insert_path(&mut trie, &path);
        }
        sort_trie(&mut trie);

        self.search_rows.clear();
        let mut guides = Vec::new();
        flatten_trie(&trie, &mut guides, &mut self.search_rows);
        self.search_sel = self
            .search_rows
            .iter()
            .position(|row| row.is_match)
            .unwrap_or(0);
        self.search_scroll = 0;
    }

    fn ensure_index(&mut self) {
        let root = self.tree.root().to_path_buf();
        let hidden = self.tree.show_hidden();
        let stale = match &self.search_index {
            Some((cached_root, cached_hidden, _)) => {
                *cached_root != root || *cached_hidden != hidden
            }
            None => true,
        };
        if stale {
            let dirs = Tree::collect_directories(&root, hidden);
            self.search_index = Some((root, hidden, dirs));
        }
    }

    fn search_move(&mut self, delta: isize) {
        if self.search_rows.is_empty() {
            return;
        }
        let last = self.search_rows.len() as isize - 1;
        self.search_sel = (self.search_sel as isize + delta).clamp(0, last) as usize;
    }

    fn search_select(&mut self, index: usize) {
        if !self.search_rows.is_empty() {
            self.search_sel = index.min(self.search_rows.len() - 1);
        }
    }

    fn picker_move(&mut self, delta: isize) {
        let last = THEMES.len() as isize - 1;
        self.picker_idx = (self.picker_idx as isize + delta).clamp(0, last) as usize;
    }

    fn confirm_picker(&mut self) {
        self.theme_idx = self.picker_idx;
        self.theme_locked = false;
        self.sync_config();
        self.mode = Mode::Tree;
        self.first_run = false;
    }

    fn cancel_picker(&mut self) {
        if self.first_run {
            self.outcome = Some(Outcome::Cancel);
            return;
        }
        self.theme_idx = self.return_theme;
        self.mode = Mode::Tree;
    }

    fn open_picker(&mut self) {
        self.picker_idx = self.theme_idx;
        self.return_theme = self.theme_idx;
        self.mode = Mode::Picker;
    }

    fn toggle_hidden(&mut self) {
        self.tree.toggle_hidden();
        self.sync_config();
        if self.mode == Mode::Search {
            self.refresh_search();
        }
    }

    fn sync_config(&mut self) {
        if !self.theme_locked {
            self.config.theme = THEMES[self.theme_idx].slug.to_string();
        }
        self.config.show_hidden = self.tree.show_hidden();
        if let Err(err) = self.config.save()
            && self.save_error.is_none()
        {
            self.save_error = Some(err.to_string());
        }
    }
}

fn name_matches(path: &Path, query: &str) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase().contains(query))
        .unwrap_or(false)
}

fn insert_path(root: &mut TrieNode, path: &Path) {
    let Ok(relative) = path.strip_prefix(&root.path) else {
        return;
    };
    let mut node = root;
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            continue;
        };
        let label = name.to_string_lossy().into_owned();
        let position = match node.children.iter().position(|child| child.name == label) {
            Some(position) => position,
            None => {
                node.children.push(TrieNode {
                    path: node.path.join(name),
                    name: label,
                    is_match: false,
                    children: Vec::new(),
                });
                node.children.len() - 1
            }
        };
        node = &mut node.children[position];
    }
    node.is_match = true;
}

fn sort_trie(node: &mut TrieNode) {
    node.children
        .sort_by_cached_key(|child| child.name.to_lowercase());
    for child in &mut node.children {
        sort_trie(child);
    }
}

fn flatten_trie(node: &TrieNode, guides: &mut Vec<bool>, out: &mut Vec<SearchRow>) {
    let count = node.children.len();
    for (index, child) in node.children.iter().enumerate() {
        let is_last = index + 1 == count;
        out.push(SearchRow {
            path: child.path.clone(),
            name: child.name.clone(),
            guides: guides.clone(),
            is_last,
            is_match: child.is_match,
            has_children: !child.children.is_empty(),
        });
        guides.push(!is_last);
        flatten_trie(child, guides, out);
        guides.pop();
    }
}
