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
        Mode::Search => search_lines(app, width),
        Mode::Picker => picker_lines(app, width),
    }
}

fn tree_lines(app: &mut App, width: u16) -> Vec<Line<'static>> {
    let theme = *app.theme();
    let total = app.tree.visible().len();
    if total == 0 {
        return vec![Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                fit_end("no subdirectories", width.saturating_sub(2) as usize),
                theme.muted_style(),
            ),
        ])];
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
    let marker_style = if is_selected {
        theme.accent_style().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let guide_style = theme.branch_style();

    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        if is_selected { "› " } else { "  " },
        marker_style,
    )];
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
        theme.accent_style(),
    ));

    let suffix_width = usize::from(node.is_symlink) * 2;
    let used = 2 + node.guides.len() * 3 + 3 + 2 + suffix_width;
    let name_width = (width as usize).saturating_sub(used).max(1);
    spans.push(Span::styled(
        fit_end(&node.name, name_width),
        if is_selected {
            theme.selected_style()
        } else {
            theme.folder_style()
        },
    ));
    if node.is_symlink {
        spans.push(Span::styled(" ↗", theme.muted_style()));
    }

    Line::from(spans)
}

fn search_lines(app: &mut App, width: u16) -> Vec<Line<'static>> {
    let theme = *app.theme();
    let width = width as usize;
    let mut lines = vec![Line::from(vec![
        Span::styled("/", theme.accent_style().add_modifier(Modifier::BOLD)),
        Span::styled(
            fit_end(&sanitize(&app.query), width.saturating_sub(1).max(1)),
            theme.selected_style(),
        ),
    ])];

    if app.matches.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                fit_end("no matches", width.saturating_sub(2)),
                theme.muted_style(),
            ),
        ]));
        return lines;
    }

    let height = (MAX_HEIGHT as usize - 1).max(1);
    let total = app.matches.len();
    let position = app.search_sel.min(total - 1);
    if position < app.search_scroll {
        app.search_scroll = position;
    }
    if position >= app.search_scroll + height {
        app.search_scroll = position + 1 - height;
    }
    if app.search_scroll + height > total {
        app.search_scroll = total.saturating_sub(height);
    }

    let root = app.tree.root().to_path_buf();
    let end = (app.search_scroll + height).min(total);
    for index in app.search_scroll..end {
        let path = &app.matches[index];
        let selected = index == position;
        let marker = if selected { "› " } else { "  " };
        let marker_style = if selected {
            theme.accent_style().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let name = fit_start(&relative(path, &root), width.saturating_sub(2));
        let name_style = if selected {
            theme.selected_style()
        } else {
            theme.folder_style()
        };
        lines.push(Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled(name, name_style),
        ]));
    }
    lines
}

fn picker_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let width = width as usize;
    let capacity = (MAX_HEIGHT as usize).min(THEMES.len());
    let offset = app.picker_idx.saturating_sub(capacity.saturating_sub(1));

    let mut lines = Vec::with_capacity(capacity);
    for (index, item) in THEMES.iter().enumerate().skip(offset).take(capacity) {
        let current = index == app.picker_idx;
        let dots = [item.accent, item.folder, item.branch, item.fg];
        let dots_width = dots.len() * 2 - 1;
        let name_width = width.saturating_sub(2 + 2 + dots_width).max(1);
        let name = fit_end(item.name, name_width);

        let mut spans = vec![
            Span::styled(
                if current { "› " } else { "  " },
                if current {
                    item.accent_style().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                name,
                if current {
                    item.selected_style()
                } else {
                    Style::default().fg(item.fg)
                },
            ),
            Span::styled("  ", Style::default()),
        ];
        for (position, color) in dots.iter().enumerate() {
            if position > 0 {
                spans.push(Span::styled(" ", Style::default()));
            }
            spans.push(Span::styled("●", Style::default().fg(*color)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn relative(path: &Path, root: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    sanitize(
        &rel.to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/"),
    )
}

fn sanitize(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { '?' } else { ch })
        .collect()
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

fn fit_start(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let mut out = String::from("…");
    out.extend(text.chars().skip(count - (width - 1)));
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;
    use ratatui::style::Color;
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

    fn type_query(app: &mut App, text: &str) {
        for ch in text.chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
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
    fn selection_is_marked_without_a_background() {
        let dir = sandbox("colors");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        let width = 60u16;
        let (buffer, _) = buffer_for(&mut app, width);
        assert!(buffer.content.iter().all(|cell| cell.bg == Color::Reset));
        let row = (0..MAX_HEIGHT)
            .find(|&y| {
                let text: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
                text.contains("alpha")
            })
            .unwrap();
        let text: String = (0..width).map(|x| buffer[(x, row)].symbol()).collect();
        assert!(text.starts_with("› "));
        let byte = text.find("alpha").unwrap();
        let start = text[..byte].chars().count() as u16;
        assert_eq!(buffer[(start, row)].fg, app.theme().fg);
        assert!(buffer[(start, row)].modifier.contains(Modifier::BOLD));
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

    #[test]
    fn typing_filters_directories() {
        let dir = sandbox("search");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        type_query(&mut app, "bet");
        assert_eq!(app.mode, Mode::Search);
        let (screen, used) = render(&mut app, 80);
        println!("{screen}");
        assert!(screen.contains("/bet"));
        assert!(screen.contains("beta"));
        assert!(!screen.contains("alpha"));
        assert_eq!(used, 2);
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match app.outcome {
            Some(Outcome::Select(path)) => assert_eq!(path, dir.join("beta")),
            _ => panic!("expected a selection"),
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn recursive_search_finds_nested_directories_only_with_all() {
        let dir = sandbox("recursive");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        type_query(&mut app, "nested");
        let (screen, used) = render(&mut app, 80);
        println!("{screen}");
        assert!(screen.contains("no matches"));
        assert_eq!(used, 2);

        app.search_all = true;
        for _ in 0.."nested".len() {
            app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        }
        type_query(&mut app, "nested");
        let (screen, _) = render(&mut app, 80);
        println!("{screen}");
        assert!(screen.contains("alpha/nested"));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match app.outcome {
            Some(Outcome::Select(path)) => assert_eq!(path, dir.join("alpha/nested")),
            _ => panic!("expected a selection"),
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn backspace_edits_then_closes_search() {
        let dir = sandbox("backspace");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        type_query(&mut app, "be");
        app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(app.query, "b");
        assert_eq!(app.mode, Mode::Search);
        app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(app.mode, Mode::Tree);
        assert!(app.query.is_empty());
        let (screen, _) = render(&mut app, 80);
        assert!(screen.contains("alpha"));
        assert!(screen.contains("beta"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn escape_closes_search() {
        let dir = sandbox("escape");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        type_query(&mut app, "al");
        assert_eq!(app.mode, Mode::Search);
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.mode, Mode::Tree);
        let (screen, _) = render(&mut app, 80);
        assert!(!screen.contains("/al"));
        assert!(screen.contains("alpha"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn arrows_move_through_matches() {
        let dir = sandbox("arrows");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        type_query(&mut app, "a");
        assert_eq!(app.matches.len(), 2);
        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.search_sel, 1);
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match app.outcome {
            Some(Outcome::Select(path)) => assert_eq!(path, dir.join("beta")),
            _ => panic!("expected a selection"),
        }
        let _ = fs::remove_dir_all(dir);
    }
}
