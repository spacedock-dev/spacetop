use ratatui::prelude::{Line, Modifier, Span, Style};
use ratatui::style::Color;
use similar::{ChangeTag, TextDiff};

/// Render a unified-style line diff between `old` and `new` bodies as
/// ratatui `Line`s. Added lines are prefixed with `+` (green), removed lines
/// with `-` (red), and unchanged context lines with a leading space (dim).
///
/// This helper is intentionally terminal-agnostic so it can be unit-tested
/// without a `Frame`.
pub fn render_diff_lines(old: &str, new: &str) -> Vec<Line<'static>> {
    let diff = TextDiff::from_lines(old, new);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for change in diff.iter_all_changes() {
        let raw = change.value();
        let text = raw.strip_suffix('\n').unwrap_or(raw).to_string();
        let (prefix, style): (&'static str, Style) = match change.tag() {
            ChangeTag::Insert => ("+", Style::default().fg(Color::Green)),
            ChangeTag::Delete => ("-", Style::default().fg(Color::Red)),
            ChangeTag::Equal => (" ", Style::default().add_modifier(Modifier::DIM)),
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(text, style),
        ]));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn render_diff_lines_emits_unified_hunks_with_add_remove_styling() {
        let old = "alpha\nbeta\ngamma\n";
        let new = "alpha\nBETA\ngamma\n";
        let lines = render_diff_lines(old, new);
        let texts: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            texts.iter().any(|t| t == "-beta"),
            "expected '-beta' removal line, got: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "+BETA"),
            "expected '+BETA' insertion line, got: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == " alpha"),
            "expected ' alpha' context line, got: {texts:?}"
        );

        let minus_line = lines
            .iter()
            .find(|l| line_text(l) == "-beta")
            .expect("minus line");
        assert_eq!(minus_line.spans[0].style.fg, Some(Color::Red));
        let plus_line = lines
            .iter()
            .find(|l| line_text(l) == "+BETA")
            .expect("plus line");
        assert_eq!(plus_line.spans[0].style.fg, Some(Color::Green));
    }

    #[test]
    fn render_diff_lines_identical_bodies_emits_only_context() {
        let body = "same\nlines\n";
        let lines = render_diff_lines(body, body);
        assert!(
            lines
                .iter()
                .map(line_text)
                .all(|t| t.starts_with(' ')),
            "all lines should be context for identical inputs"
        );
    }
}
