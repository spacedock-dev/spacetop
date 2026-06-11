use crate::domain::Rgb;
use ratatui::style::Color;

/// Convert a core `Rgb` into a ratatui `Color` at the UI boundary.
pub(crate) fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}
