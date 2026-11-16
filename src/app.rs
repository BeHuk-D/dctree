use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::Config;
use crate::theme::{THEMES, Theme};
use crate::tree::Tree;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Tree,
    Picker,
}

pub enum Outcome {
    Select(PathBuf),
    Cancel,
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
    config: Config,
    return_theme: usize,
    theme_locked: bool,
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
            config,
            return_theme: theme_idx,
            theme_locked: theme_override.is_some(),
        }
    }

    pub fn theme(&self) -> &'static Theme {
        match self.mode {
            Mode::Picker => &THEMES[self.picker_idx],
            Mode::Tree => &THEMES[self.theme_idx],
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.outcome = Some(Outcome::Cancel);
            return;
        }
        match self.mode {
            Mode::Tree => self.tree_key(key.code),
            Mode::Picker => self.picker_key(key.code),
        }
    }

    fn tree_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.outcome = Some(Outcome::Cancel),
            KeyCode::Up | KeyCode::Char('k') => self.tree.move_by(-1),
            KeyCode::Down | KeyCode::Char('j') => self.tree.move_by(1),
            KeyCode::PageUp => self.tree.move_by(-10),
            KeyCode::PageDown => self.tree.move_by(10),
            KeyCode::Home | KeyCode::Char('g') => self.tree.select_first(),
            KeyCode::End | KeyCode::Char('G') => self.tree.select_last(),
            KeyCode::Right | KeyCode::Char('l') => self.tree.expand(),
            KeyCode::Left | KeyCode::Char('h') => self.tree.collapse(),
            KeyCode::Backspace => self.tree.go_up(),
            KeyCode::Char('~') => self.tree.home(),
            KeyCode::Char('.') => {
                self.tree.toggle_hidden();
                self.sync_config();
            }
            KeyCode::Char('t') => self.open_picker(),
            KeyCode::Enter => {
                if let Some(path) = self.tree.selected_path() {
                    self.outcome = Some(Outcome::Select(path.to_path_buf()));
                }
            }
            _ => {}
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
