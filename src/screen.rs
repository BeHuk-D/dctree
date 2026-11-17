use std::io::{self, Write};

use ratatui::buffer::Buffer;
use ratatui::crossterm::terminal;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::ui;

pub struct Screen {
    reserved: u16,
}

impl Screen {
    pub fn new() -> Self {
        Self { reserved: 0 }
    }

    pub fn draw(&mut self, app: &mut App) -> io::Result<()> {
        let width = terminal::size().map_or(80, |(cols, _)| cols).max(1);
        let lines = ui::lines(app, width);
        let used = (lines.len() as u16).clamp(1, ui::MAX_HEIGHT);
        let area = Rect::new(0, 0, width, used);
        let mut buffer = Buffer::empty(area);
        Paragraph::new(lines).render(area, &mut buffer);

        let mut out = String::new();
        let first = self.reserved == 0;
        if used > self.reserved {
            for _ in 0..used - self.reserved {
                out.push_str("\r\n");
            }
            self.reserved = used;
        }
        let back = self.reserved.saturating_sub(1) + u16::from(first);
        if back > 0 {
            out.push_str(&format!("\x1b[{back}A"));
        }
        for row in 0..self.reserved {
            out.push('\r');
            out.push_str("\x1b[2K");
            if row < used {
                emit_row(&mut out, &buffer, row, width);
            }
            if row + 1 < self.reserved {
                out.push_str("\r\n");
            }
        }
        out.push_str("\x1b[0m\r");
        write_stderr(&out)
    }

    pub fn finish(&mut self) -> io::Result<()> {
        let mut out = clear_sequence(self.reserved);
        out.push_str("\x1b[?25h");
        write_stderr(&out)
    }
}

fn clear_sequence(reserved: u16) -> String {
    let mut out = String::from("\x1b[0m\r");
    for row in 0..reserved {
        out.push_str("\x1b[2K");
        if row + 1 < reserved {
            out.push_str("\x1b[1A\r");
        }
    }
    out
}

fn emit_row(out: &mut String, buffer: &Buffer, row: u16, width: u16) {
    let Some(end) = last_used(buffer, row, width) else {
        return;
    };
    let mut column = 0;
    let mut current = None;
    while column <= end && column < width {
        let cell = &buffer[(column, row)];
        let style = (cell.fg, cell.bg, cell.modifier);
        if current != Some(style) {
            push_style(out, style);
            current = Some(style);
        }
        let symbol = cell.symbol();
        out.push_str(symbol);
        column += (UnicodeWidthStr::width(symbol).max(1)) as u16;
    }
}

fn last_used(buffer: &Buffer, row: u16, width: u16) -> Option<u16> {
    let mut last = None;
    let mut column = 0;
    while column < width {
        let cell = &buffer[(column, row)];
        let symbol_width = UnicodeWidthStr::width(cell.symbol()).max(1) as u16;
        if cell.fg != Color::Reset
            || cell.bg != Color::Reset
            || !cell.modifier.is_empty()
            || cell.symbol() != " "
        {
            last = Some(column + symbol_width - 1);
        }
        column += symbol_width;
    }
    last
}

fn push_style(out: &mut String, (fg, bg, modifier): (Color, Color, Modifier)) {
    out.push_str("\x1b[0m");
    if modifier.contains(Modifier::BOLD) {
        out.push_str("\x1b[1m");
    }
    if modifier.contains(Modifier::DIM) {
        out.push_str("\x1b[2m");
    }
    if let Some(code) = color_sequence(fg, 38) {
        out.push_str(&code);
    }
    if let Some(code) = color_sequence(bg, 48) {
        out.push_str(&code);
    }
}

fn color_sequence(color: Color, base: u8) -> Option<String> {
    match color {
        Color::Rgb(r, g, b) => Some(format!("\x1b[{base};2;{r};{g};{b}m")),
        Color::Indexed(index) => Some(format!("\x1b[{base};5;{index}m")),
        _ => None,
    }
}

fn write_stderr(text: &str) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    stderr.write_all(text.as_bytes())?;
    stderr.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Style;

    #[test]
    fn emits_styles_and_text() {
        let area = Rect::new(0, 0, 4, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_stringn(
            0,
            0,
            "ab",
            4,
            Style::default()
                .fg(Color::Rgb(1, 2, 3))
                .bg(Color::Rgb(4, 5, 6))
                .add_modifier(Modifier::BOLD),
        );
        let mut out = String::new();
        emit_row(&mut out, &buffer, 0, 4);
        assert!(out.contains("ab"));
        assert!(out.contains("\x1b[1m"));
        assert!(out.contains("38;2;1;2;3"));
        assert!(out.contains("48;2;4;5;6"));
    }

    #[test]
    fn skips_trailing_empty_cells() {
        let area = Rect::new(0, 0, 6, 1);
        let mut buffer = Buffer::empty(area);
        buffer[(0, 0)]
            .set_symbol("a")
            .set_style(Style::default().fg(Color::Rgb(1, 2, 3)));
        let mut out = String::new();
        emit_row(&mut out, &buffer, 0, 6);
        assert!(out.contains('a'));
        assert_eq!(out.matches(' ').count(), 0);
    }

    #[test]
    fn clears_the_whole_inline_region() {
        assert_eq!(
            clear_sequence(3),
            "\x1b[0m\r\x1b[2K\x1b[1A\r\x1b[2K\x1b[1A\r\x1b[2K"
        );
    }

    #[test]
    fn clears_nothing_without_reserved_rows() {
        assert_eq!(clear_sequence(0), "\x1b[0m\r");
    }
}
