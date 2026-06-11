use ratatui::style::Color;
use spacetop_core::domain::Rgb;

/// Convert a core `Rgb` into a ratatui `Color` at the UI boundary.
pub(crate) fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}
