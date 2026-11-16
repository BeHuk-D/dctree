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
    used: u16,
}

impl Screen {
    pub fn new() -> Self {
        Self {
            reserved: 0,
            used: 0,
        }
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
        self.used = used;
        write_stderr(&out)
    }

    pub fn finish(&mut self) -> io::Result<()> {
        let mut out = finish_sequence(self.used, self.reserved);
        out.push_str("\x1b[0m\x1b[?25h");
        write_stderr(&out)
    }
}

fn finish_sequence(used: u16, reserved: u16) -> String {
    let mut out = String::new();
    if used < reserved {
        let up = reserved - 1 - used;
        if up > 0 {
            out.push_str(&format!("\x1b[{up}A"));
        }
        out.push('\r');
    } else {
        out.push_str("\r\n");
    }
    out
}

fn emit_row(out: &mut String, buffer: &Buffer, row: u16, width: u16) {
    let mut column = 0;
    let mut current = None;
    while column < width {
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
    fn parks_cursor_below_a_full_region() {
        assert_eq!(finish_sequence(5, 5), "\r\n");
    }

    #[test]
    fn parks_cursor_at_the_first_free_row() {
        assert_eq!(finish_sequence(2, 5), "\x1b[2A\r");
        assert_eq!(finish_sequence(4, 5), "\r");
    }
}
