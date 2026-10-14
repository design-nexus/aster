mod omarchy;

use ratatui::style::{Color, Style};

pub use omarchy::load as load_omarchy;

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub bg: Color,
    pub bg_deep: Color,
    pub border: Color,
    pub muted: Color,
    pub error: Color,
    pub selection: Color,
    pub selection_fg: Color,
    pub fg: Color,
    pub accent: Color,
    /// Left-to-right logo colors, taken from the Omarchy palette.
    pub gradient: Vec<Color>,
}

impl Theme {
    pub fn aurora() -> Self {
        Self {
            name: "Aurora".into(),
            bg: Color::Rgb(23, 24, 32),
            bg_deep: Color::Rgb(20, 21, 29),
            border: Color::Rgb(61, 64, 84),
            muted: Color::Rgb(165, 169, 190),
            error: Color::Rgb(237, 135, 150),
            selection: Color::Rgb(34, 36, 50),
            selection_fg: Color::Rgb(181, 160, 255),
            fg: Color::Rgb(236, 236, 244),
            accent: Color::Rgb(181, 160, 255),
            gradient: vec![Color::Rgb(181, 160, 255), Color::Rgb(131, 217, 220)],
        }
    }
    pub fn terminal() -> Self {
        Self {
            name: "Terminal".into(),
            bg: Color::Reset,
            bg_deep: Color::Reset,
            border: Color::DarkGray,
            muted: Color::Gray,
            error: Color::Red,
            selection: Color::DarkGray,
            selection_fg: Color::White,
            fg: Color::Reset,
            accent: Color::Blue,
            gradient: vec![
                Color::Blue,
                Color::Cyan,
                Color::Green,
                Color::Yellow,
                Color::Magenta,
            ],
        }
    }

    pub fn base(&self) -> Style {
        Style::default().bg(self.bg).fg(self.fg)
    }

    pub fn selected(&self) -> Style {
        Style::default().bg(self.selection).fg(self.selection_fg)
    }

    pub fn footer(&self) -> Style {
        Style::default().bg(self.bg_deep).fg(self.fg)
    }
}
