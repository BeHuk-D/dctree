use std::path::Path;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{App, Mode};
use crate::theme::{THEMES, Theme};
use crate::tree::Tree;

pub const MAX_HEIGHT: u16 = 12;

pub fn lines(app: &mut App, width: u16) -> Vec<Line<'static>> {
    let width = width.max(1);
    match app.mode {
        Mode::Tree => tree_lines(app, width),
        Mode::Picker => picker_lines(app, width),
    }
}

fn tree_lines(app: &mut App, width: u16) -> Vec<Line<'static>> {
    let theme = *app.theme();
    let total = app.tree.visible().len();
    if total == 0 {
        let line = Line::from(vec![
            Span::styled("  ", theme.bg_style()),
            Span::styled(
                fit_end("no subdirectories", width.saturating_sub(2) as usize),
                theme.muted_style(),
            ),
        ]);
        return vec![pad(line, width, theme.bg_style())];
    }

    let height = total.min(MAX_HEIGHT as usize);
    let position = app.tree.selected_pos().unwrap_or(0);
    if position < app.scroll {
        app.scroll = position;
    }
    if position >= app.scroll + height {
        app.scroll = position + 1 - height;
    }
    if app.scroll + height > total {
        app.scroll = total.saturating_sub(height);
    }

    let selected = app.tree.selected_path().map(Path::to_path_buf);
    let end = (app.scroll + height).min(total);
    let mut lines = Vec::with_capacity(end - app.scroll);
    for &idx in &app.tree.visible()[app.scroll..end] {
        lines.push(tree_line(
            &app.tree,
            idx,
            width,
            selected.as_deref(),
            &theme,
        ));
    }
    lines
}

fn tree_line(
    tree: &Tree,
    idx: usize,
    width: u16,
    selected: Option<&Path>,
    theme: &Theme,
) -> Line<'static> {
    let node = &tree.nodes()[idx];
    let is_selected = selected.is_some_and(|path| path == node.path);
    let base = if is_selected {
        theme.selected_style()
    } else {
        theme.bg_style()
    };
    let guide_style = if is_selected {
        base.add_modifier(Modifier::DIM)
    } else {
        theme.branch_style()
    };

    let mut spans: Vec<Span<'static>> = vec![Span::styled("  ", base)];
    for &continues in &node.guides {
        spans.push(Span::styled(
            if continues { "│  " } else { "   " },
            guide_style,
        ));
    }
    spans.push(Span::styled(
        if node.is_last { "╰─ " } else { "├─ " },
        guide_style,
    ));
    spans.push(Span::styled(
        if node.expanded { "▾ " } else { "▸ " },
        if is_selected {
            base
        } else {
            theme.accent_style()
        },
    ));

    let suffix_width = usize::from(node.is_symlink) * 2;
    let used = 2 + node.guides.len() * 3 + 3 + 2 + suffix_width;
    let name_width = (width as usize).saturating_sub(used).max(1);
    spans.push(Span::styled(
        fit_end(&node.name, name_width),
        if is_selected {
            base
        } else {
            theme.folder_style()
        },
    ));
    if node.is_symlink {
        spans.push(Span::styled(
            " ↗",
            if is_selected {
                base
            } else {
                theme.muted_style()
            },
        ));
    }

    pad(Line::from(spans), width, base)
}

fn pad(mut line: Line<'static>, width: u16, style: Style) -> Line<'static> {
    let used = line.width();
    let width = width as usize;
    if used < width {
        line.spans
            .push(Span::styled(" ".repeat(width - used), style));
    }
    line
}

fn picker_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let width = width as usize;
    let capacity = (MAX_HEIGHT as usize).min(THEMES.len());
    let offset = app.picker_idx.saturating_sub(capacity.saturating_sub(1));

    let mut lines = Vec::with_capacity(capacity);
    for (index, item) in THEMES.iter().enumerate().skip(offset).take(capacity) {
        let current = index == app.picker_idx;
        let base = Style::default()
            .bg(if current { item.sel_bg } else { item.bg })
            .fg(if current { item.sel_fg } else { item.fg });
        let bold = if current {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        let dots = [item.accent, item.folder, item.branch, item.fg];
        let dots_width = dots.len() * 2 - 1 + 2;
        let name_width = width.saturating_sub(2 + dots_width).max(1);
        let name = fit_end(item.name, name_width);

        let mut spans = vec![
            Span::styled(if current { "› " } else { "  " }, base.fg(item.accent)),
            Span::styled(name.clone(), base.add_modifier(bold)),
        ];
        let used = 2 + name.chars().count();
        let gap = width.saturating_sub(used + dots_width);
        if gap > 0 {
            spans.push(Span::styled(" ".repeat(gap), base));
        }
        for (position, color) in dots.iter().enumerate() {
            if position > 0 {
                spans.push(Span::styled(" ", base));
            }
            spans.push(Span::styled("●", base.fg(*color)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn fit_end(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(width - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;
    use ratatui::widgets::{Paragraph, Widget};

    use super::*;
    use crate::app::Outcome;
    use crate::config::Config;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("box2cell-ui-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("alpha/nested")).unwrap();
        fs::create_dir_all(dir.join("beta")).unwrap();
        dir
    }

    fn buffer_for(app: &mut App, width: u16) -> (Buffer, u16) {
        let lines = lines(app, width);
        let used = lines.len() as u16;
        let area = Rect::new(0, 0, width, MAX_HEIGHT);
        let mut buffer = Buffer::empty(area);
        Paragraph::new(lines).render(area, &mut buffer);
        (buffer, used)
    }

    fn render(app: &mut App, width: u16) -> (String, u16) {
        let (buffer, used) = buffer_for(app, width);
        let mut out = String::new();
        for y in 0..MAX_HEIGHT {
            for x in 0..width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        (out, used)
    }

    #[test]
    fn tree_draws_only_the_directory_list() {
        let dir = sandbox("tree");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        let (screen, used) = render(&mut app, 80);
        println!("{screen}");
        assert!(screen.contains("alpha"));
        assert!(screen.contains("beta"));
        assert!(!screen.contains("box2cell"));
        assert!(!screen.contains('╭'));
        assert_eq!(used, 2);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn expanding_adds_a_row() {
        let dir = sandbox("expand");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let (screen, used) = render(&mut app, 80);
        println!("{screen}");
        assert!(screen.contains("nested"));
        assert_eq!(used, 3);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn picker_lists_themes_inline() {
        let dir = sandbox("picker");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), true, false, None);
        let (screen, used) = render(&mut app, 80);
        println!("{screen}");
        assert!(screen.contains("Tokyo Night"));
        assert!(screen.contains("Rose Pine Dawn"));
        assert!(screen.contains("› Tokyo Night"));
        assert!(!screen.contains("choose a theme"));
        assert!(!screen.contains("palette"));
        assert_eq!(used, 6);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn long_tree_scrolls_in_place() {
        let dir = std::env::temp_dir().join(format!("box2cell-ui-scroll-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for index in 0..30 {
            fs::create_dir_all(dir.join(format!("dir-{index:02}"))).unwrap();
        }
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        let (screen, used) = render(&mut app, 60);
        assert!(screen.contains("dir-00"));
        assert!(!screen.contains("dir-29"));
        assert_eq!(used, MAX_HEIGHT);
        app.on_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        let (screen, _) = render(&mut app, 60);
        assert!(screen.contains("dir-29"));
        assert!(!screen.contains("dir-00"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn selection_uses_theme_colors() {
        let dir = sandbox("colors");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        let width = 60u16;
        let (buffer, _) = buffer_for(&mut app, width);
        let sel_bg = app.theme().sel_bg;
        let rows: Vec<u16> = buffer
            .content
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.bg == sel_bg)
            .map(|(index, _)| (index as u16) / width)
            .collect();
        assert_eq!(rows.len(), width as usize);
        assert!(rows.iter().all(|&row| row == rows[0]));
        let text: String = (0..width).map(|x| buffer[(x, rows[0])].symbol()).collect();
        assert!(text.contains("alpha"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn enter_selects_directory() {
        let dir = sandbox("enter");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(app.outcome, Some(Outcome::Select(_))));
        let _ = fs::remove_dir_all(dir);
    }
}
