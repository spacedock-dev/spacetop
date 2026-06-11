use ratatui::style::Color;
use spacetop_core::config::SpacetopConfig;
use spacetop_core::domain::Rgb;

const DEFAULT_SELECTION_BG: Color = Color::Rgb(40, 52, 84);
const DEFAULT_FOOTER_BG: Color = Color::Rgb(59, 66, 82);

/// Convert a core `Rgb` into a ratatui `Color` at the UI boundary.
pub(crate) fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}

pub(crate) fn color_from_hex(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

pub(crate) fn selection_bg(config: &SpacetopConfig) -> Color {
    color_from_hex(&config.theme.selection_bg).unwrap_or(DEFAULT_SELECTION_BG)
}

pub(crate) fn footer_bg(config: &SpacetopConfig) -> Color {
    color_from_hex(&config.theme.footer_bg).unwrap_or(DEFAULT_FOOTER_BG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_rgb_color() {
        assert_eq!(color_from_hex("#283454"), Some(Color::Rgb(40, 52, 84)));
    }

    #[test]
    fn invalid_hex_color_returns_none() {
        assert_eq!(color_from_hex("blue"), None);
        assert_eq!(color_from_hex("#12"), None);
        assert_eq!(color_from_hex("#gggggg"), None);
    }
}
