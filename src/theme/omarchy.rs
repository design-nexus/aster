use std::path::{Path, PathBuf};

use ratatui::style::Color;
use toml_edit::DocumentMut;

use super::Theme;

pub fn load() -> Option<Theme> {
    for dir in candidate_dirs() {
        if let Some(theme) = load_from(&dir) {
            return Some(theme);
        }
    }
    None
}

fn candidate_dirs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    vec![
        home.join(".local/state/omarchy/current"),
        home.join(".config/omarchy/current"),
    ]
}

fn load_from(dir: &Path) -> Option<Theme> {
    let text = std::fs::read_to_string(dir.join("theme").join("colors.toml")).ok()?;
    let name = std::fs::read_to_string(dir.join("theme.name"))
        .unwrap_or_default()
        .trim()
        .to_string();
    let name = if name.is_empty() {
        "Omarchy".to_string()
    } else {
        format!("Omarchy · {name}")
    };
    theme_from_str(&text, &name).ok()
}

fn theme_from_str(text: &str, name: &str) -> Result<Theme, String> {
    let doc: DocumentMut = text
        .parse()
        .map_err(|err: toml_edit::TomlError| err.to_string())?;
    let background = required(&doc, "background")?;
    let foreground = required(&doc, "foreground")?;
    let accent = first(&doc, &["accent", "blue", "cyan"]);
    let selection = first(&doc, &["selection", "lighter_background", "muted"]);
    Ok(Theme {
        name: name.to_string(),
        bg: background,
        bg_deep: first(
            &doc,
            &["dark_background", "darker_background", "background"],
        ),
        border: first(&doc, &["muted", "selection", "dark_foreground"]),
        selection,
        selection_fg: readable(selection, foreground, background),
        fg: foreground,
        accent,
        gradient: palette(
            &doc,
            &["blue", "cyan", "green", "yellow", "orange", "red", "magenta"],
            accent,
        ),
    })
}

fn palette(doc: &DocumentMut, keys: &[&str], fallback: Color) -> Vec<Color> {
    let colors: Vec<Color> = keys
        .iter()
        .filter_map(|key| doc.get(*key).and_then(|item| item.as_str()).and_then(parse_hex))
        .collect();
    if colors.len() >= 2 {
        colors
    } else {
        vec![fallback]
    }
}

fn required(doc: &DocumentMut, key: &str) -> Result<Color, String> {
    let raw = doc
        .get(key)
        .and_then(|item| item.as_str())
        .ok_or_else(|| format!("missing {key}"))?;
    parse_hex(raw).ok_or_else(|| format!("bad color for {key}"))
}

fn first(doc: &DocumentMut, keys: &[&str]) -> Color {
    for key in keys {
        if let Some(text) = doc.get(key).and_then(|item| item.as_str()) {
            if let Some(color) = parse_hex(text) {
                return color;
            }
        }
    }
    Color::Rgb(0x88, 0x88, 0x88)
}

fn parse_hex(text: &str) -> Option<Color> {
    let text = text.trim().trim_start_matches('#');
    if text.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    Some(Color::Rgb(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ))
}

fn luminance(color: Color) -> f32 {
    match color {
        Color::Rgb(r, g, b) => 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32,
        _ => 40.0,
    }
}

fn readable(bg: Color, prefer: Color, alt: Color) -> Color {
    let prefer_gap = (luminance(bg) - luminance(prefer)).abs();
    let alt_gap = (luminance(bg) - luminance(alt)).abs();
    if prefer_gap >= 90.0 || prefer_gap >= alt_gap {
        prefer
    } else {
        alt
    }
}
