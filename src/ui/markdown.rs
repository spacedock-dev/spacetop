//! Termimad-backed markdown renderer for the preview-pane body.
//!
//! We use [`ratskin`] — a thin wrapper around [`termimad`] that emits
//! ratatui `Line`s directly — so we do not have to write our own ANSI
//! decoder. The skin is tuned so code blocks and inline code keep the
//! Cyan-on-DarkGray look the rest of the TUI's tests already pin down.
//!
//! Keeping this module renderer-only (input `&str`, output
//! `Vec<Line<'static>>`) makes the seam testable without a terminal
//! backend — see the tests at the bottom of this file.

use ratatui::prelude::Line;
use ratskin::RatSkin;
use termimad::crossterm::style::Color as CtColor;

/// Render a markdown body to a flat `Vec<Line<'static>>` wrapped to
/// `width` cells.
///
/// The width is interpreted the same way the hand-rolled renderer used
/// it: it should be the inner width of the body area in the preview
/// pane (i.e. `body_inner.width`). `width = 0` is normalised to `1` so
/// callers do not have to special-case the zero-width edge.
pub fn render_markdown_termimad(body: &str, width: u16) -> Vec<Line<'static>> {
    let width = width.max(1);
    let skin = preview_skin();
    let parsed = RatSkin::parse_text(body);
    let lines = skin.parse(parsed, width);
    // ratskin returns `Vec<Line<'a>>` borrowed from the parsed `Text`.
    // We need owned `'static` lines so callers can keep them across
    // frames. Spans are mostly `Cow::Owned` already but force an
    // owning conversion to be safe.
    lines.into_iter().map(to_static_line).collect()
}

/// Drop trailing whitespace-only spans that carry no styling.
///
/// `ratskin` appends right-margin "completion" padding to every line so
/// terminal-style output stays aligned. Inside a ratatui `Paragraph`
/// with `Wrap { trim: false }` those trailing spaces would re-wrap onto
/// extra rows and dilute scrollbar metrics, so we strip them here.
fn trim_trailing_padding(mut spans: Vec<ratatui::text::Span<'static>>) -> Vec<ratatui::text::Span<'static>> {
    while let Some(last) = spans.last() {
        let is_padding = last.style == ratatui::prelude::Style::default()
            && last.content.chars().all(|c| c == ' ');
        if is_padding {
            spans.pop();
        } else {
            break;
        }
    }
    spans
}

fn preview_skin() -> RatSkin {
    let mut skin = RatSkin::default();
    // Align code styling with the rest of the TUI's tests (Cyan on
    // DarkGray). Termimad pads code lines to the outer width, which
    // gives the slab background look we already document.
    skin.skin
        .code_block
        .compound_style
        .set_fgbg(CtColor::DarkCyan, CtColor::DarkGrey);
    skin.skin
        .inline_code
        .set_fgbg(CtColor::DarkCyan, CtColor::DarkGrey);
    skin
}

fn to_static_line(line: Line<'_>) -> Line<'static> {
    use ratatui::text::Span;
    let spans: Vec<Span<'static>> = line
        .spans
        .into_iter()
        .map(|span| Span::styled(span.content.into_owned(), span.style))
        .collect();
    let spans = trim_trailing_padding(spans);
    let mut owned = Line::from(spans).style(line.style);
    if let Some(alignment) = line.alignment {
        owned = owned.alignment(alignment);
    }
    owned
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    fn flatten(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// AC-1: the new renderer produces termimad-distinctive styling for
    /// the canonical markdown features (heading, list, inline code,
    /// code block, emphasis).
    #[test]
    fn renders_termimad_distinctive_styling() {
        let body = "# Heading\n\nSome **bold** and `code` text.\n\n```rust\nlet x = 1;\n```\n\n- first\n- second";
        let lines = render_markdown_termimad(body, 60);
        let rendered = flatten(&lines);

        // No raw markdown markers leak through.
        assert!(rendered.contains("Heading"), "heading text must be present");
        assert!(
            !rendered.contains("# Heading"),
            "heading hash marker should not survive: {rendered}"
        );
        assert!(
            !rendered.contains("**bold**"),
            "bold markers should not survive: {rendered}"
        );

        // Heading text carries termimad's bold styling.
        let heading_has_bold = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|s| s.content.contains("Heading") && s.style.add_modifier.contains(Modifier::BOLD))
        });
        assert!(heading_has_bold, "heading span must be bold");

        // Inline `code` carries the slab style (DarkGray bg).
        let inline_code_styled = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|s| s.content.contains("code") && s.style.bg == Some(Color::DarkGray))
        });
        assert!(
            inline_code_styled,
            "inline code span must carry DarkGray background"
        );

        // Code block line keeps Cyan/DarkGray slab styling.
        let code_block_styled = lines.iter().any(|line| {
            line.spans.iter().any(|s| {
                s.content.contains("let x = 1;")
                    && s.style.bg == Some(Color::DarkGray)
                    && s.style.fg == Some(Color::Cyan)
            })
        });
        assert!(
            code_block_styled,
            "code block span must be Cyan-on-DarkGray"
        );

        // Bullet items render their text without the raw `-` marker.
        assert!(rendered.contains("first"), "list item text must render");
        assert!(rendered.contains("second"), "list item text must render");
    }

    /// AC-2: wide content produces enough lines that callers will trip
    /// the scrollbar overflow branch.
    #[test]
    fn wide_content_wraps_to_multiple_lines() {
        let body = "X".repeat(400);
        let narrow = render_markdown_termimad(&body, 40);
        assert!(
            narrow.len() > 4,
            "400 chars wrapped to width 40 should produce more than 4 lines, got {}",
            narrow.len()
        );
    }

    #[test]
    fn empty_body_does_not_panic() {
        let lines = render_markdown_termimad("", 40);
        // An empty body may produce zero lines — but it must not panic
        // and must return a Vec we can iterate over.
        let _ = lines.len();
    }
}
