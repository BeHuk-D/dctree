use ratatui::style::{Color, Modifier, Style};

const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        (hex >> 16) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub slug: &'static str,
    pub name: &'static str,
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub muted: Color,
    pub branch: Color,
    pub folder: Color,
    pub sel_bg: Color,
    pub sel_fg: Color,
}

impl Theme {
    pub fn bg_style(&self) -> Style {
        Style::default().bg(self.bg)
    }

    pub fn muted_style(&self) -> Style {
        self.bg_style().fg(self.muted)
    }

    pub fn accent_style(&self) -> Style {
        self.bg_style().fg(self.accent)
    }

    pub fn branch_style(&self) -> Style {
        self.bg_style().fg(self.branch)
    }

    pub fn folder_style(&self) -> Style {
        self.bg_style().fg(self.folder)
    }

    pub fn selected_style(&self) -> Style {
        Style::default()
            .bg(self.sel_bg)
            .fg(self.sel_fg)
            .add_modifier(Modifier::BOLD)
    }
}

pub static THEMES: [Theme; 6] = [
    Theme {
        slug: "tokyonight",
        name: "Tokyo Night",
        bg: rgb(0x1a1b26),
        fg: rgb(0xc0caf5),
        accent: rgb(0x7aa2f7),
        muted: rgb(0x565f89),
        branch: rgb(0x3b4261),
        folder: rgb(0x7dcfff),
        sel_bg: rgb(0x33467c),
        sel_fg: rgb(0xffffff),
    },
    Theme {
        slug: "catppuccin",
        name: "Catppuccin",
        bg: rgb(0x1e1e2e),
        fg: rgb(0xcdd6f4),
        accent: rgb(0xcba6f7),
        muted: rgb(0x7f849c),
        branch: rgb(0x45475a),
        folder: rgb(0x89b4fa),
        sel_bg: rgb(0x45475a),
        sel_fg: rgb(0xf5e0dc),
    },
    Theme {
        slug: "nord",
        name: "Nord",
        bg: rgb(0x2e3440),
        fg: rgb(0xd8dee9),
        accent: rgb(0x88c0d0),
        muted: rgb(0x616e88),
        branch: rgb(0x434c5e),
        folder: rgb(0x81a1c1),
        sel_bg: rgb(0x434c5e),
        sel_fg: rgb(0xeceff4),
    },
    Theme {
        slug: "gruvbox",
        name: "Gruvbox",
        bg: rgb(0x282828),
        fg: rgb(0xebdbb2),
        accent: rgb(0xfabd2f),
        muted: rgb(0x928374),
        branch: rgb(0x504945),
        folder: rgb(0x83a598),
        sel_bg: rgb(0x45403d),
        sel_fg: rgb(0xfbf1c7),
    },
    Theme {
        slug: "rosepine",
        name: "Rose Pine",
        bg: rgb(0x191724),
        fg: rgb(0xe0def4),
        accent: rgb(0xc4a7e7),
        muted: rgb(0x6e6a86),
        branch: rgb(0x403d52),
        folder: rgb(0x9ccfd8),
        sel_bg: rgb(0x403d52),
        sel_fg: rgb(0xe0def4),
    },
    Theme {
        slug: "dawn",
        name: "Rose Pine Dawn",
        bg: rgb(0xfaf4ed),
        fg: rgb(0x575279),
        accent: rgb(0xd7827e),
        muted: rgb(0x9893a5),
        branch: rgb(0xd6ccc3),
        folder: rgb(0x56949f),
        sel_bg: rgb(0xeaddd5),
        sel_fg: rgb(0x575279),
    },
];

pub fn by_slug(slug: &str) -> Option<&'static Theme> {
    THEMES
        .iter()
        .find(|theme| theme.slug.eq_ignore_ascii_case(slug))
}
