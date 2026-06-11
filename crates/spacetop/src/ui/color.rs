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
    let bytes = hex.as_bytes();
    if bytes.len() != 6 {
        return None;
    }
    let r = parse_hex_pair(&bytes[0..2])?;
    let g = parse_hex_pair(&bytes[2..4])?;
    let b = parse_hex_pair(&bytes[4..6])?;
    Some(Color::Rgb(r, g, b))
}

fn parse_hex_pair(bytes: &[u8]) -> Option<u8> {
    Some(hex_value(bytes[0])? << 4 | hex_value(bytes[1])?)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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

    #[test]
    fn non_ascii_hex_color_returns_none() {
        assert_eq!(color_from_hex("#€€"), None);
    }
}
