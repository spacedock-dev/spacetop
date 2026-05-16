use ratatui::prelude::{Line, Modifier, Span, Style};
use ratatui::style::Color;
use similar::{ChangeTag, TextDiff};

use super::markdown::render_markdown_termimad;

/// Gutter glyph + style returned by [`gutter_for`]. Each produced markdown
/// `Line` is prefixed with this single-cell span so adds/removes/context stay
/// visually distinct even after termimad styling is applied to the line.
fn gutter_for(tag: ChangeTag) -> (&'static str, Style) {
    match tag {
        ChangeTag::Insert => ("+", Style::default().fg(Color::Green)),
        ChangeTag::Delete => ("-", Style::default().fg(Color::Red)),
        ChangeTag::Equal => (" ", Style::default().add_modifier(Modifier::DIM)),
    }
}

/// Per-line code-fence context for one side of the diff.
///
/// For each source line we record whether it sits inside a fenced code
/// block (`in_fence`) and, if so, the language token from the opening
/// fence (so a chunk extracted mid-fence can be re-wrapped with a fence
/// of the same flavour before being handed to `termimad`).
///
/// The fence opener / closer lines themselves are *not* treated as
/// in-fence: they are the markdown delimiters that produce the slab.
#[derive(Clone, Debug)]
struct LineFenceCtx {
    in_fence: bool,
    fence_lang: String,
}

fn fence_context(source: &str) -> Vec<LineFenceCtx> {
    let mut ctx = Vec::new();
    let mut in_fence = false;
    let mut lang = String::new();
    for line in source.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_fence {
                // Closing fence — the closer line itself is not in-fence.
                ctx.push(LineFenceCtx {
                    in_fence: false,
                    fence_lang: String::new(),
                });
                in_fence = false;
                lang.clear();
            } else {
                // Opening fence — also not in-fence; capture the lang for
                // subsequent in-fence lines.
                lang = trimmed.trim_start_matches('`').trim().to_string();
                ctx.push(LineFenceCtx {
                    in_fence: false,
                    fence_lang: String::new(),
                });
                in_fence = true;
            }
        } else {
            ctx.push(LineFenceCtx {
                in_fence,
                fence_lang: lang.clone(),
            });
        }
    }
    ctx
}

/// One contiguous chunk of source lines that share a diff tag. We keep the
/// raw line strings (not joined) so the chunk can be re-wrapped with fence
/// markers if it sits inside a code block.
struct DiffChunk {
    tag: ChangeTag,
    lines: Vec<String>,
    /// `Some(lang)` if every line in this chunk is inside a fenced code
    /// block on its source side; `None` otherwise. Mixed chunks fall back
    /// to `None` and render as plain markdown.
    fence_lang: Option<String>,
}

/// Render a markdown-aware unified diff between `old` and `new` as a flat
/// `Vec<Line<'static>>` wrapped to `width`.
///
/// Algorithm (approach B from the design notes):
///   1. Run `similar::TextDiff::from_lines` over the raw markdown sources
///      and pre-compute per-line fence context for each side.
///   2. Group adjacent changes of the same kind (insert / delete / equal)
///      into a single contiguous source chunk. For each chunk, decide
///      whether *every* line is inside a code fence on its source side;
///      if so, capture the fence language so the chunk can be re-wrapped
///      with `\`\`\`lang ... \`\`\`` before going through termimad.
///   3. Feed each chunk through `render_markdown_termimad` at the gutter-
///      adjusted width (`width - 1`).
///   4. Prefix every produced `Line` with the gutter span from
///      [`gutter_for`].
///
/// Width is reserved by one column for the gutter so wrapped lines stay
/// inside the pane. If `width <= 1`, the markdown render falls back to
/// width = 1 (matching the contract of `render_markdown_termimad`).
pub fn render_diff_lines_with_width(old: &str, new: &str, width: u16) -> Vec<Line<'static>> {
    let inner_width = width.saturating_sub(1).max(1);
    let old_ctx = fence_context(old);
    let new_ctx = fence_context(new);
    let diff = TextDiff::from_lines(old, new);

    let mut chunks: Vec<DiffChunk> = Vec::new();
    for change in diff.iter_all_changes() {
        let tag = change.tag();
        let raw = change.value();
        let text = raw.strip_suffix('\n').unwrap_or(raw).to_string();

        // Look up this line's fence context on whichever side it came from.
        let (idx_opt, ctx_side): (Option<usize>, &[LineFenceCtx]) = match tag {
            ChangeTag::Insert => (change.new_index(), new_ctx.as_slice()),
            ChangeTag::Delete => (change.old_index(), old_ctx.as_slice()),
            // For equal lines, both sides agree on fence-ness; pick `new`.
            ChangeTag::Equal => (change.new_index(), new_ctx.as_slice()),
        };
        let per_line = idx_opt
            .and_then(|i| ctx_side.get(i))
            .cloned()
            .unwrap_or(LineFenceCtx {
                in_fence: false,
                fence_lang: String::new(),
            });

        match chunks.last_mut() {
            Some(last) if last.tag == tag => {
                last.lines.push(text);
                if per_line.in_fence {
                    // Same fence language across the whole chunk → keep
                    // the lang; otherwise drop to None.
                    last.fence_lang = match last.fence_lang.take() {
                        Some(prev) if prev == per_line.fence_lang => Some(prev),
                        _ => None,
                    };
                } else {
                    last.fence_lang = None;
                }
            }
            _ => chunks.push(DiffChunk {
                tag,
                lines: vec![text],
                fence_lang: if per_line.in_fence {
                    Some(per_line.fence_lang.clone())
                } else {
                    None
                },
            }),
        }
    }

    let mut out: Vec<Line<'static>> = Vec::new();
    for chunk in chunks {
        let (prefix, gutter_style) = gutter_for(chunk.tag);
        let source = if let Some(lang) = chunk.fence_lang.as_ref() {
            // Re-wrap this chunk with a fence so termimad applies the
            // code-block slab styling even though the original fence
            // delimiters live outside the chunk.
            let body = chunk.lines.join("\n");
            format!("```{lang}\n{body}\n```")
        } else {
            chunk.lines.join("\n")
        };
        let rendered = render_markdown_termimad(&source, inner_width);
        if rendered.is_empty() {
            // Preserve blank diff lines (an empty source line should still
            // show a gutter so the user sees the change at that position).
            out.push(Line::from(vec![Span::styled(prefix, gutter_style)]));
            continue;
        }
        for line in rendered {
            let mut spans: Vec<Span<'static>> = Vec::with_capacity(line.spans.len() + 1);
            spans.push(Span::styled(prefix, gutter_style));
            spans.extend(line.spans);
            let mut prefixed = Line::from(spans).style(line.style);
            if let Some(alignment) = line.alignment {
                prefixed = prefixed.alignment(alignment);
            }
            out.push(prefixed);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// Strip trailing whitespace from a rendered line's text. Termimad may
    /// emit padding spans that survive into the line text on plain
    /// paragraphs; the diff payload itself is what we want to assert
    /// against.
    fn line_text_trim_end(line: &Line<'_>) -> String {
        let s: String = line_text(line);
        s.trim_end().to_string()
    }

    #[test]
    fn render_diff_lines_emits_unified_hunks_with_add_remove_styling() {
        let old = "alpha\nbeta\ngamma\n";
        let new = "alpha\nBETA\ngamma\n";
        let lines = render_diff_lines_with_width(old, new, 80);
        let texts: Vec<String> = lines.iter().map(line_text_trim_end).collect();
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
            .find(|l| line_text_trim_end(l) == "-beta")
            .expect("minus line");
        assert_eq!(minus_line.spans[0].style.fg, Some(Color::Red));
        let plus_line = lines
            .iter()
            .find(|l| line_text_trim_end(l) == "+BETA")
            .expect("plus line");
        assert_eq!(plus_line.spans[0].style.fg, Some(Color::Green));
    }

    /// Locks in the call-site contract that the diff renderer treats `new`
    /// as additions (`+`) and `old` as removals (`-`). The preview renderer
    /// passes `main_body` as `old` and the worktree-divergent body as
    /// `new`, so worktree-unique content must render with `+` and
    /// main-unique content with `-`.
    #[test]
    fn render_diff_lines_treats_new_as_plus_old_as_minus() {
        let old = "shared\nONLY_IN_MAIN\nshared2\n";
        let new = "shared\nONLY_IN_WORKTREE\nshared2\n";
        let lines = render_diff_lines_with_width(old, new, 80);
        let texts: Vec<String> = lines.iter().map(line_text_trim_end).collect();
        assert!(
            texts.iter().any(|t| t == "-ONLY_IN_MAIN"),
            "main-only content must render with '-' prefix, got: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "+ONLY_IN_WORKTREE"),
            "worktree-only content must render with '+' prefix, got: {texts:?}"
        );
    }

    #[test]
    fn render_diff_lines_identical_bodies_emits_only_context() {
        let body = "same\nlines\n";
        let lines = render_diff_lines_with_width(body, body, 80);
        assert!(
            lines
                .iter()
                .map(line_text)
                .all(|t| t.starts_with(' ')),
            "all lines should be context for identical inputs"
        );
    }

    /// AC-1 + AC-2: the diff renderer must (a) carry termimad markdown
    /// styling on every line — headings, inline code, and fenced code
    /// blocks alike — and (b) prepend a single-character styled gutter
    /// (`+` green / `-` red / ` ` dim) to every produced `Line`. This test
    /// constructs `old` and `new` bodies that put each of those three
    /// markdown features into a context region, a removed region, and an
    /// added region and asserts both invariants hold.
    #[test]
    fn render_diff_lines_styles_markdown_across_context_add_remove() {
        let old = "\
# Shared heading

Paragraph with `old_code` inline.

```rust
let removed = 1;
```
";
        let new = "\
# Shared heading

Paragraph with `new_code` inline.

```rust
let added = 2;
```
";
        let lines = render_diff_lines_with_width(old, new, 80);

        // AC-2: every produced line starts with a gutter span whose
        // content is `+`, `-`, or ` ` and whose style matches the
        // expected palette.
        for (idx, line) in lines.iter().enumerate() {
            let gutter = line.spans.first().unwrap_or_else(|| {
                panic!("line {idx} has no spans at all: {:?}", lines)
            });
            let glyph: &str = gutter.content.as_ref();
            assert!(
                matches!(glyph, "+" | "-" | " "),
                "line {idx} gutter must be one of '+'/'-'/' '; got {glyph:?}"
            );
            match glyph {
                "+" => assert_eq!(
                    gutter.style.fg,
                    Some(Color::Green),
                    "'+' gutter must be green on line {idx}",
                ),
                "-" => assert_eq!(
                    gutter.style.fg,
                    Some(Color::Red),
                    "'-' gutter must be red on line {idx}",
                ),
                " " => assert!(
                    gutter.style.add_modifier.contains(Modifier::DIM),
                    "' ' gutter must be dim on line {idx}",
                ),
                _ => unreachable!(),
            }
        }

        // AC-1: heading text is bold and appears on a context line (the
        // heading is shared between old and new).
        let heading_line = lines
            .iter()
            .find(|l| {
                l.spans
                    .iter()
                    .any(|s| s.content.contains("Shared heading"))
            })
            .expect("heading line must exist");
        assert_eq!(
            heading_line.spans[0].content.as_ref(),
            " ",
            "heading is shared content so its gutter must be ' '",
        );
        let heading_bold = heading_line.spans.iter().any(|s| {
            s.content.contains("Shared heading")
                && s.style.add_modifier.contains(Modifier::BOLD)
        });
        assert!(
            heading_bold,
            "heading text must keep termimad's bold modifier in the diff path",
        );

        // AC-1: inline `old_code` appears on a removed line with the
        // DarkGray inline-code background.
        let removed_inline = lines
            .iter()
            .find(|l| {
                l.spans.first().map(|s| s.content.as_ref()) == Some("-")
                    && l.spans
                        .iter()
                        .any(|s| s.content.contains("old_code"))
            })
            .expect("removed line containing 'old_code' must exist");
        let removed_inline_styled = removed_inline.spans.iter().any(|s| {
            s.content.contains("old_code") && s.style.bg == Some(Color::DarkGray)
        });
        assert!(
            removed_inline_styled,
            "inline `old_code` on the removed line must carry DarkGray bg",
        );

        // AC-1: inline `new_code` appears on an added line with the
        // DarkGray inline-code background.
        let added_inline = lines
            .iter()
            .find(|l| {
                l.spans.first().map(|s| s.content.as_ref()) == Some("+")
                    && l.spans
                        .iter()
                        .any(|s| s.content.contains("new_code"))
            })
            .expect("added line containing 'new_code' must exist");
        let added_inline_styled = added_inline.spans.iter().any(|s| {
            s.content.contains("new_code") && s.style.bg == Some(Color::DarkGray)
        });
        assert!(
            added_inline_styled,
            "inline `new_code` on the added line must carry DarkGray bg",
        );

        // AC-1: the fenced code block lines on each side keep
        // Cyan-on-DarkGray slab styling.
        let removed_code = lines.iter().any(|l| {
            l.spans.first().map(|s| s.content.as_ref()) == Some("-")
                && l.spans.iter().any(|s| {
                    s.content.contains("let removed = 1;")
                        && s.style.fg == Some(Color::Cyan)
                        && s.style.bg == Some(Color::DarkGray)
                })
        });
        assert!(
            removed_code,
            "removed code block line must be Cyan-on-DarkGray",
        );
        let added_code = lines.iter().any(|l| {
            l.spans.first().map(|s| s.content.as_ref()) == Some("+")
                && l.spans.iter().any(|s| {
                    s.content.contains("let added = 2;")
                        && s.style.fg == Some(Color::Cyan)
                        && s.style.bg == Some(Color::DarkGray)
                })
        });
        assert!(
            added_code,
            "added code block line must be Cyan-on-DarkGray",
        );

        // Raw markdown markers must not survive — even on diff lines.
        for line in &lines {
            let text: String = line_text(line);
            assert!(
                !text.contains("```rust"),
                "fence marker leaked into diff output: {text:?}"
            );
            assert!(
                !text.starts_with("-# ") && !text.starts_with("+# ") && !text.starts_with(" # "),
                "heading hash marker leaked into diff output: {text:?}"
            );
        }
    }

    /// AC-4: a divergent body wider than the preview area must produce
    /// more lines than `body_inner.height` so the scrollbar branch in
    /// `src/ui/mod.rs` engages.
    #[test]
    fn render_diff_lines_wraps_wide_content_for_scrollbar() {
        // `new` has one very long paragraph that, once wrapped to a
        // narrow width and prefixed with the gutter, must exceed a
        // realistic preview height.
        let old = "shared\n";
        let new = format!("shared\n\n{}\n", "X".repeat(800));
        let lines = render_diff_lines_with_width(old, &new, 40);
        assert!(
            lines.len() > 10,
            "800 chars wrapped to width 40 should exceed a typical short \
             preview height; got {} lines",
            lines.len()
        );
    }
}
