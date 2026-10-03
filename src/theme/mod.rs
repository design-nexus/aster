mod omarchy;

use ratatui::style::{Color, Style};

pub use omarchy::load as load_omarchy;

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub bg: Color,
    pub bg_deep: Color,
    pub border: Color,
    pub selection: Color,
    pub selection_fg: Color,
    pub fg: Color,
    pub accent: Color,
    /// Left-to-right logo colors, taken from the Omarchy palette.
    pub gradient: Vec<Color>,
}

impl Theme {
    pub fn terminal() -> Self {
        Self {
            name: "Terminal".into(),
            bg: Color::Reset,
            bg_deep: Color::Reset,
            border: Color::DarkGray,
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

    /// Color at `t` along the theme gradient. `t` is 0 at the left and 1 at the right.
    pub fn gradient_at(&self, t: f32) -> Color {
        let stops: Vec<(u8, u8, u8)> = self
            .gradient
            .iter()
            .filter_map(|color| match color {
                Color::Rgb(red, green, blue) => Some((*red, *green, *blue)),
                _ => None,
            })
            .collect();
        if stops.is_empty() {
            return self.accent;
        }
        if stops.len() == 1 || t <= 0.0 {
            let (red, green, blue) = stops[0];
            return Color::Rgb(red, green, blue);
        }
        if t >= 1.0 {
            let (red, green, blue) = stops[stops.len() - 1];
            return Color::Rgb(red, green, blue);
        }
        let scaled = t * (stops.len() - 1) as f32;
        let index = scaled.floor() as usize;
        let next = (index + 1).min(stops.len() - 1);
        let blend = scaled - index as f32;
        let (r1, g1, b1) = stops[index];
        let (r2, g2, b2) = stops[next];
        Color::Rgb(
            mix(r1, r2, blend),
            mix(g1, g2, blend),
            mix(b1, b2, blend),
        )
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

fn mix(from: u8, to: u8, t: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * t).round() as u8
}
