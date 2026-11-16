use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::app::{App, Mode};
use crate::config::display_path;
use crate::theme::{THEMES, Theme};
use crate::tree::Tree;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let theme = *app.theme();
    frame.render_widget(Block::default().style(theme.base()), frame.area());

    let area = frame.area();
    let too_small = match app.mode {
        Mode::Tree => area.width < 30 || area.height < 9,
        Mode::Picker => area.width < 50 || area.height < 18,
    };
    if too_small {
        let rows = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(area);
        frame.render_widget(
            Paragraph::new("terminal too small")
                .style(theme.muted_style())
                .alignment(Alignment::Center),
            rows[1],
        );
        return;
    }

    match app.mode {
        Mode::Tree => draw_tree(frame, app, &theme),
        Mode::Picker => draw_picker(frame, app, &theme),
    }
}

fn draw_tree(frame: &mut Frame, app: &mut App, theme: &Theme) {
    let area = frame.area().inner(Margin::new(2, 1));
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(frame, app, theme, rows[0]);

    let panel = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style())
        .style(theme.bg_style());
    let inner = panel.inner(rows[2]);
    frame.render_widget(panel, rows[2]);
    let content = Rect {
        x: inner.x + 2,
        width: inner.width.saturating_sub(3),
        ..inner
    };
    draw_rows(frame, app, theme, content, inner);

    draw_target(frame, app, theme, rows[3]);
    draw_hints(frame, theme, rows[4]);
}

fn draw_header(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let count = app.tree.visible().len();
    let right = if app.tree.show_hidden() {
        format!("{count} dirs · hidden")
    } else {
        format!("{count} dirs")
    };
    let right_width = right.chars().count() as u16;
    let cols =
        Layout::horizontal([Constraint::Min(12), Constraint::Length(right_width)]).split(area);

    let path_width = (cols[0].width as usize).saturating_sub("box2cell  ".len());
    let path = fit_end(&display_path(app.tree.root()), path_width);
    let line = Line::from(vec![
        Span::styled(
            "box2cell",
            theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", theme.bg_style()),
        Span::styled(path, theme.muted_style()),
    ]);
    frame.render_widget(Paragraph::new(line), cols[0]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(right, theme.muted_style())))
            .alignment(Alignment::Right),
        cols[1],
    );
}

fn draw_rows(frame: &mut Frame, app: &mut App, theme: &Theme, content: Rect, inner: Rect) {
    if content.width == 0 || content.height == 0 {
        return;
    }
    let total = app.tree.visible().len();
    if total == 0 {
        let rows = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(content);
        let message = if app.tree.show_hidden() {
            "no subdirectories · ⌫ to go up"
        } else {
            "no subdirectories · . shows hidden · ⌫ to go up"
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(theme.muted_style())
                .alignment(Alignment::Center),
            rows[1],
        );
        return;
    }

    let height = content.height as usize;
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

    let width = content.width;
    let selected = app.tree.selected_path().map(Path::to_path_buf);

    let end = (app.scroll + height).min(total);
    let mut lines: Vec<Line> = Vec::with_capacity(end - app.scroll);
    for &idx in &app.tree.visible()[app.scroll..end] {
        lines.push(tree_line(&app.tree, idx, width, selected.as_deref(), theme));
    }
    frame.render_widget(Paragraph::new(lines), content);

    if let Some(position) = app.tree.selected_pos()
        && content.x > 0
        && position >= app.scroll
        && position < app.scroll + height
    {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("▌", theme.accent_style()))),
            Rect::new(
                content.x - 1,
                content.y + (position - app.scroll) as u16,
                1,
                1,
            ),
        );
    }

    if total > height && inner.width >= 2 {
        draw_scrollbar(frame, theme, inner, app.scroll, total, height);
    }
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
    let faint = if is_selected {
        base.add_modifier(Modifier::DIM)
    } else {
        theme.muted_style()
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
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

    let empty = node.expanded && node.loaded && !node.has_children;
    let suffix_width = usize::from(node.is_symlink) * 2 + if empty { " (empty)".len() } else { 0 };
    let used = node.guides.len() * 3 + 3 + 2 + suffix_width;
    let name_width = (width as usize).saturating_sub(used).max(1);
    let name = fit_end(&node.name, name_width);
    spans.push(Span::styled(
        name.clone(),
        if is_selected {
            base
        } else {
            theme.folder_style()
        },
    ));
    if node.is_symlink {
        spans.push(Span::styled(" ↗", faint));
    }
    if empty {
        spans.push(Span::styled(" (empty)", faint));
    }

    let mut line = Line::from(spans);
    let used_width = line.width();
    let width = width as usize;
    if used_width < width {
        line.spans
            .push(Span::styled(" ".repeat(width - used_width), base));
    }
    line
}

fn draw_scrollbar(
    frame: &mut Frame,
    theme: &Theme,
    area: Rect,
    scroll: usize,
    total: usize,
    height: usize,
) {
    let x = area.x + area.width - 1;
    let thumb = ((height * height) / total).clamp(1, height);
    let max_scroll = total.saturating_sub(height);
    let travel = height.saturating_sub(thumb);
    let start = (scroll * travel).checked_div(max_scroll).unwrap_or(0);
    for offset in 0..height {
        let active = offset >= start && offset < start + thumb;
        let (symbol, style) = if active {
            ("▌", theme.accent_style())
        } else {
            ("│", theme.bg_style().fg(theme.border))
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(symbol, style))),
            Rect::new(x, area.y + offset as u16, 1, 1),
        );
    }
}

fn draw_target(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let arrow_style = theme.accent_style().add_modifier(Modifier::BOLD);
    let line = match app.tree.selected_path() {
        Some(path) => {
            let width = (area.width as usize).saturating_sub(2);
            let path = fit_start(&display_path(path), width);
            Line::from(vec![
                Span::styled("→ ", arrow_style),
                Span::styled(path, theme.bg_style().fg(theme.fg)),
            ])
        }
        None => Line::from(vec![
            Span::styled("→ ", arrow_style),
            Span::styled("nothing selected", theme.muted_style()),
        ]),
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_hints(frame: &mut Frame, theme: &Theme, area: Rect) {
    let hints = [
        ("↑↓", "move"),
        ("→", "expand"),
        ("←", "collapse"),
        ("⏎", "cd"),
        ("⌫", "up"),
        (".", "hidden"),
        ("t", "theme"),
        ("esc", "quit"),
    ];
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    let limit = area.width as usize;
    for (index, (key, label)) in hints.iter().enumerate() {
        let pair = key.chars().count() + 1 + label.chars().count();
        let needed = pair + if index == 0 { 0 } else { 2 };
        if used + needed > limit {
            break;
        }
        if index > 0 {
            spans.push(Span::styled("  ", theme.bg_style()));
            used += 2;
        }
        spans.push(Span::styled(
            *key,
            theme.accent_style().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(" ", theme.bg_style()));
        spans.push(Span::styled(*label, theme.muted_style()));
        used += pair;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_picker(frame: &mut Frame, app: &mut App, theme: &Theme) {
    let area = frame.area().inner(Margin::new(2, 1));
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(6),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let title = Line::from(vec![
        Span::styled(
            "box2cell",
            theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  ", theme.muted_style()),
        Span::styled("choose a theme", theme.bg_style().fg(theme.fg)),
    ]);
    frame.render_widget(Paragraph::new(title).alignment(Alignment::Center), rows[0]);
    frame.render_widget(
        Paragraph::new("you can change it any time with t")
            .style(theme.muted_style())
            .alignment(Alignment::Center),
        rows[1],
    );

    let max_width = area.width.min(84);
    let available = rows[3];
    let body_height = available.height.min(20);
    let body = Rect {
        x: area.x + area.width.saturating_sub(max_width) / 2,
        y: available.y + available.height.saturating_sub(body_height) / 2,
        width: max_width,
        height: body_height,
    };

    if body.width >= 58 {
        let columns = Layout::horizontal([
            Constraint::Length(30),
            Constraint::Length(2),
            Constraint::Min(24),
        ])
        .split(body);
        draw_theme_list(frame, app, theme, columns[0]);
        draw_preview(frame, theme, columns[2]);
    } else {
        draw_theme_list(frame, app, theme, body);
    }

    let footer = if app.first_run {
        "↑↓ choose   ⏎ save and continue   esc quit"
    } else {
        "↑↓ choose   ⏎ apply   esc back"
    };
    frame.render_widget(
        Paragraph::new(footer)
            .style(theme.muted_style())
            .alignment(Alignment::Center),
        rows[5],
    );
}

fn draw_theme_list(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.accent_style())
        .style(theme.bg_style())
        .title(Line::from(Span::styled(" themes ", theme.accent_style())));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let width = inner.width as usize;
    let capacity = inner.height as usize;
    let count = THEMES.len();
    let (offset, pad) = if capacity >= count {
        (0, (capacity - count) / 2)
    } else {
        (app.picker_idx.saturating_sub(capacity.saturating_sub(1)), 0)
    };
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(capacity);
    for _ in 0..pad {
        lines.push(Line::from(""));
    }

    for (index, item) in THEMES.iter().enumerate().skip(offset).take(capacity) {
        if lines.len() >= capacity {
            break;
        }
        let is_current = index == app.picker_idx;
        let base = if is_current {
            theme.selected_style()
        } else {
            theme.bg_style()
        };
        let marker = if is_current { " › " } else { "   " };
        let kind = if item.light { "light" } else { "dark" };
        let kind_width = kind.chars().count();
        let name_width = width.saturating_sub(3 + kind_width + 1);
        let name = fit_end(item.name, name_width);

        let mut spans = vec![
            Span::styled(
                marker,
                if is_current {
                    base.fg(theme.accent)
                } else {
                    theme.bg_style()
                },
            ),
            Span::styled(
                name.clone(),
                if is_current {
                    base
                } else {
                    theme.bg_style().fg(theme.fg)
                },
            ),
        ];
        let used = 3 + name.chars().count();
        let padding = width.saturating_sub(used + kind_width);
        if padding > 0 {
            spans.push(Span::styled(" ".repeat(padding), base));
        }
        spans.push(Span::styled(
            kind,
            if is_current {
                base.add_modifier(Modifier::DIM)
            } else {
                theme.muted_style()
            },
        ));
        lines.push(Line::from(spans));
    }

    while lines.len() < capacity {
        lines.push(Line::from(""));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_preview(frame: &mut Frame, theme: &Theme, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_style())
        .style(theme.bg_style())
        .title(Line::from(Span::styled(
            format!(" {} ", theme.name),
            theme.muted_style(),
        )));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let width = inner.width as usize;
    let mut lines = preview_lines(theme, width);
    let mut pad = (inner.height as usize).saturating_sub(lines.len()) / 2;
    let palette_height = 2;
    if (inner.height as usize) >= lines.len() + palette_height {
        let mut palette = vec![Span::styled("palette  ", theme.muted_style())];
        for color in [
            theme.accent,
            theme.folder,
            theme.branch,
            theme.fg,
            theme.sel_bg,
        ] {
            palette.push(Span::styled("●", theme.bg_style().fg(color)));
            palette.push(Span::styled(" ", theme.bg_style()));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(palette));
        pad = (inner.height as usize).saturating_sub(lines.len()) / 2;
    }

    let mut all: Vec<Line<'static>> = Vec::with_capacity(inner.height as usize);
    for _ in 0..pad {
        all.push(Line::from(""));
    }
    all.extend(lines);
    frame.render_widget(Paragraph::new(all), inner);
}

fn preview_lines(theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let rows: [(&[bool], bool, bool, &str, bool); 8] = [
        (&[], false, true, "projects", false),
        (&[true], false, true, "box2cell", true),
        (&[true, true], false, false, "src", false),
        (&[true, true], false, false, "tests", false),
        (&[true, true], true, false, "docs", false),
        (&[true], false, false, "dotfiles", false),
        (&[true], true, false, "notes", false),
        (&[], true, false, "archive", false),
    ];

    let mut lines = Vec::with_capacity(rows.len());
    for (guides, last, expanded, name, selected) in rows {
        let base = if selected {
            theme.selected_style()
        } else {
            theme.bg_style()
        };
        let guide_style = if selected {
            base.add_modifier(Modifier::DIM)
        } else {
            theme.branch_style()
        };
        let mut spans: Vec<Span<'static>> = Vec::new();
        for &continues in guides {
            spans.push(Span::styled(
                if continues { "│  " } else { "   " },
                guide_style,
            ));
        }
        spans.push(Span::styled(if last { "╰─ " } else { "├─ " }, guide_style));
        spans.push(Span::styled(
            if expanded { "▾ " } else { "▸ " },
            if selected { base } else { theme.accent_style() },
        ));
        spans.push(Span::styled(
            name,
            if selected { base } else { theme.folder_style() },
        ));
        let mut line = Line::from(spans);
        if selected {
            let used = line.width();
            if used < width {
                line.spans
                    .push(Span::styled(" ".repeat(width - used), base));
            }
        }
        lines.push(line);
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
    let skip = count - (width - 1);
    let mut out = String::from("…");
    out.extend(text.chars().skip(skip));
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

    fn render(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn tree_screen() {
        let dir = sandbox("tree");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        let screen = render(&mut app, 80, 20);
        println!("{screen}");
        assert!(screen.contains("box2cell"));
        assert!(screen.contains("alpha"));
        assert!(screen.contains("beta"));
        assert!(screen.contains("→ "));
        assert!(screen.contains("⏎ cd"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn tree_screen_after_expanding() {
        let dir = sandbox("expand");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let screen = render(&mut app, 80, 20);
        println!("{screen}");
        assert!(screen.contains("nested"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn picker_screen_on_first_run() {
        let dir = sandbox("picker");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), true, false, None);
        let screen = render(&mut app, 90, 24);
        println!("{screen}");
        assert!(screen.contains("choose a theme"));
        assert!(screen.contains("Tokyo Night"));
        assert!(screen.contains("Rose Pine Dawn"));
        assert!(screen.contains("palette"));
        assert!(screen.contains("save and continue"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn long_tree_scrolls_with_bar() {
        let dir = std::env::temp_dir().join(format!("box2cell-ui-scroll-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for index in 0..30 {
            fs::create_dir_all(dir.join(format!("dir-{index:02}"))).unwrap();
        }
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        let screen = render(&mut app, 60, 14);
        assert!(screen.contains("▌"));
        assert!(screen.contains("dir-00"));
        app.on_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        let screen = render(&mut app, 60, 14);
        assert!(screen.contains("dir-29"));
        assert!(!screen.contains("dir-00"));
        println!("{screen}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn selection_uses_theme_colors() {
        let dir = sandbox("colors");
        let tree = Tree::new(dir.clone(), false);
        let mut app = App::new(tree, Config::default(), false, false, None);
        let width = 60u16;
        let mut terminal = Terminal::new(TestBackend::new(width, 16)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let sel_bg = app.theme().sel_bg;
        let rows: Vec<u16> = buffer
            .content
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.bg == sel_bg)
            .map(|(index, _)| (index as u16) / width)
            .collect();
        assert!(rows.len() > (width as usize) / 2);
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
