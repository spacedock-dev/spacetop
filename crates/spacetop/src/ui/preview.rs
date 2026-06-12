use ratatui::{
    layout::Rect,
    prelude::{Frame, Line, Modifier, Span, Style},
    style::Color,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::app::{OverviewState, PreviewPlacement, SelectedRow, ViewScope};
use spacetop_core::domain::EntityParseError;

use super::{diff, markdown};

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

thread_local! {
    /// Per-render-thread memoization of the termimad markdown render. Lives in
    /// the ui layer (NOT on `OverviewState`) so app-state stays free of ratatui
    /// types and testable without a terminal backend (see CLAUDE.md). Keyed by
    /// content, so a live-reload edit to the same task file self-invalidates.
    static MARKDOWN_CACHE: RefCell<MarkdownCache> = RefCell::new(MarkdownCache::default());
}

/// Render `body` to wrapped lines, reusing the thread-local cache when the
/// (path, content, wrap, width) tuple is unchanged since the last frame.
fn cached_markdown(path: &Path, body: &str, wrap: bool, width: u16) -> Vec<Line<'static>> {
    MARKDOWN_CACHE.with(|cache| {
        cache.borrow_mut().get_or_render(
            path,
            body,
            wrap,
            width,
            markdown::render_markdown_termimad,
        )
    })
}

/// Single-item markdown render cache. Invalidates whole-key on a change to
/// path, body content (hash), or wrap; within a key it retains up to two
/// widths so the wrap+scrollbar two-pass render (full width then scrollbar-
/// narrowed width) both hit on the next frame instead of thrashing.
#[derive(Default)]
struct MarkdownCache {
    key: Option<MarkdownCacheKey>,
    renders: Vec<(u16, Vec<Line<'static>>)>,
}

#[derive(PartialEq)]
struct MarkdownCacheKey {
    path: PathBuf,
    body_hash: u64,
    wrap: bool,
}

impl MarkdownCache {
    fn get_or_render(
        &mut self,
        path: &Path,
        body: &str,
        wrap: bool,
        width: u16,
        render: impl FnOnce(&str, u16) -> Vec<Line<'static>>,
    ) -> Vec<Line<'static>> {
        let key = MarkdownCacheKey {
            path: path.to_path_buf(),
            body_hash: hash_str(body),
            wrap,
        };
        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.renders.clear();
        }
        if let Some((_, lines)) = self.renders.iter().find(|(w, _)| *w == width) {
            return lines.clone();
        }
        let lines = render(body, width);
        // Cap at the two widths a single frame uses (full pass + scrollbar pass);
        // evict oldest first so the cache never grows unbounded.
        if self.renders.len() >= 2 {
            self.renders.remove(0);
        }
        self.renders.push((width, lines.clone()));
        lines
    }
}

fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Stable user-facing hint string shown in the broken-entity preview to guide
/// the captain back to a parseable frontmatter. Pinned by a UI test so any
/// change here is intentional and visible to reviewers.
pub(crate) const BROKEN_ENTITY_HINT: &str =
    "Hint: wrap values containing ':' in quotes, or use '>-' for multi-line scalars";

const MARKDOWN_NO_WRAP_RENDER_WIDTH: u16 = 4096;

pub(super) fn render_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &OverviewState,
    placement: PreviewPlacement,
) {
    let borders = match placement {
        PreviewPlacement::Left => Borders::LEFT,
        PreviewPlacement::Bottom => Borders::TOP,
    };
    let block = Block::default().borders(borders);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let item = match state.selected_row() {
        Some(SelectedRow::Item(item)) => item,
        Some(SelectedRow::Broken(err)) => {
            render_broken_entity_preview(frame, inner, &err);
            return;
        }
        None => {
            let dim = Style::default().add_modifier(Modifier::DIM);
            let header = Line::from(Span::styled("Preview", dim));
            let mut lines = vec![header];
            if inner.height > 1 {
                lines.push(Line::from("Select a work item to inspect it."));
            }
            frame.render_widget(Paragraph::new(lines), inner);
            return;
        }
    };

    let mut header_lines = build_preview_header_lines(&item, state, inner.width, placement);
    let divider_line = header_lines.pop().unwrap_or_else(|| Line::from(""));
    let divider_height = wrapped_lines_height(std::slice::from_ref(&divider_line), inner.width);
    let metadata_height = wrapped_lines_height(&header_lines, inner.width)
        .min(inner.height.saturating_sub(divider_height));
    let header_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: metadata_height,
    };
    if metadata_height > 0 {
        frame.render_widget(
            Paragraph::new(header_lines).wrap(Wrap { trim: true }),
            header_area,
        );
    }

    let divider_y = inner.y + metadata_height;
    if divider_y >= inner.y + inner.height {
        return;
    }
    let divider_area = Rect {
        x: inner.x,
        y: divider_y,
        width: inner.width,
        height: divider_height.min(inner.height.saturating_sub(metadata_height)),
    };
    frame.render_widget(Paragraph::new(vec![divider_line]), divider_area);

    let body_inner = Rect {
        x: inner.x,
        y: divider_y + divider_area.height,
        width: inner.width,
        height: inner
            .height
            .saturating_sub(metadata_height + divider_area.height),
    };
    // When a worktree copy of this task has a body that diverges from the
    // root (main) copy, render a unified diff between the two bodies instead
    // of the plain markdown body. Otherwise fall back to the normal markdown
    // rendering path.
    let diff_lines: Option<Vec<Line<'static>>> = item
        .main_body
        .as_deref()
        .map(|main| diff::render_diff_lines_with_width(main, &item.body, body_inner.width));

    // First pass: determine content height for overflow detection.
    // In the diff path, derive height directly from `diff_lines.len()` to
    // avoid cloning the entire Vec<Line>. In the markdown path, render only
    // height+1 lines so we don't render the whole body twice for long previews.
    // When wrap is OFF, pre-wrapping the markdown to the pane width would
    // hide horizontally-scrollable overflow. Pass a wide render width so
    // paragraphs stay on a single Line; ratatui's no-wrap horizontal scroll
    // then exposes them.
    let render_width = if state.preview_wrap() {
        body_inner.width
    } else {
        body_inner.width.max(MARKDOWN_NO_WRAP_RENDER_WIDTH)
    };
    let (content_height_full, body_lines_full) = if let Some(lines) = diff_lines.as_ref() {
        (lines.len() as u16, None)
    } else {
        let lines = cached_markdown(&item.path, &item.body, state.preview_wrap(), render_width);
        (lines.len() as u16, Some(lines))
    };
    let show_scrollbar = content_height_full > body_inner.height && body_inner.width > 1;
    let body_area = if show_scrollbar {
        Rect {
            x: body_inner.x,
            y: body_inner.y,
            width: body_inner.width - 1,
            height: body_inner.height,
        }
    } else {
        body_inner
    };
    // Second pass: re-render only when the scrollbar narrows the render area.
    // This ensures code block lines are padded to body_area.width (not body_inner.width)
    // so they do not overflow the render area and leave background gaps in wrap mode.
    let render_width_second_pass = if state.preview_wrap() {
        body_area.width
    } else {
        body_area.width.max(MARKDOWN_NO_WRAP_RENDER_WIDTH)
    };
    let body_lines = if let Some(lines) = diff_lines {
        lines
    } else if show_scrollbar && state.preview_wrap() {
        // Re-render only when the wrap-mode scrollbar narrows the render
        // area, so code-block backgrounds still pad to the visible width.
        cached_markdown(
            &item.path,
            &item.body,
            state.preview_wrap(),
            render_width_second_pass,
        )
    } else {
        body_lines_full.expect("markdown path always produces body_lines_full")
    };
    let content_height = body_lines.len() as u16;

    let max_scroll = usize::from(content_height.saturating_sub(body_area.height));
    state.max_preview_scroll.set(max_scroll);
    // Feed the live body height back to the scroll state so page/half steps are
    // viewport-relative (mirrors the task_page_size Cell pattern for the list).
    state
        .preview_viewport_height
        .set(usize::from(body_area.height));
    let content_width = body_lines.iter().map(line_width).max().unwrap_or(0);
    let max_scroll_x = content_width.saturating_sub(body_area.width as usize);
    state.max_preview_scroll_x.set(max_scroll_x);
    let scroll_position = state.preview_scroll().min(max_scroll);
    let scroll_x = state.preview_scroll_x().min(max_scroll_x) as u16;
    let body_para = if state.preview_wrap() {
        state.max_preview_scroll_x.set(0);
        Paragraph::new(body_lines)
            .scroll((scroll_position as u16, 0))
            .wrap(Wrap { trim: false })
    } else {
        Paragraph::new(body_lines).scroll((scroll_position as u16, scroll_x))
    };
    frame.render_widget(body_para, body_area);

    if show_scrollbar {
        let mut scrollbar_state = ScrollbarState::new(max_scroll + 1).position(scroll_position);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some("\u{2502}"))
                .thumb_symbol("\u{2588}"),
            body_inner,
            &mut scrollbar_state,
        );
    }
}

fn build_preview_header_lines<'a>(
    item: &'a spacetop_core::domain::Entity,
    state: &OverviewState,
    inner_width: u16,
    placement: PreviewPlacement,
) -> Vec<Line<'a>> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let score = item
        .score
        .map(|score| format!("{score:.2}"))
        .unwrap_or_else(|| "n/a".to_string());
    let source = item.source.as_deref().unwrap_or("n/a");
    let worktree_segment: Vec<Span<'_>> = {
        let trimmed = item
            .worktree
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match trimmed {
            Some(path) => {
                let basename = std::path::Path::new(path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| path.to_string());
                vec![Span::styled("worktree: ", dim), Span::raw(basename)]
            }
            None => vec![
                Span::styled("worktree: ", dim),
                Span::styled("\u{2014}", dim),
            ],
        }
    };
    let mut lines: Vec<Line<'_>> = Vec::new();

    // Combined section header + title: "Preview  ·  #id  Title" so that both
    // "Preview" and the task title appear without consuming an extra row.
    // The section marker is dim, the title is bold.
    lines.push(Line::from(vec![
        Span::styled(
            format!("Preview  \u{00B7}  #{}  ", item.id),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(
            item.title.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));

    let status_color = crate::ui::color::to_color(state.definition().stage_color_for(&item.status));
    let status_spans = vec![
        Span::styled("status: ", dim),
        Span::styled("\u{25CF}", Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled(
            item.status.clone(),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if state.view_scope() == ViewScope::Archived {
        let verdict = item.verdict.as_deref().unwrap_or("n/a");
        let completed = item.completed.as_deref().unwrap_or("n/a");
        let verdict_style = match item.verdict.as_deref() {
            Some("PASSED") => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            Some(_) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            None => Style::default().add_modifier(Modifier::DIM),
        };
        match placement {
            PreviewPlacement::Bottom => {
                let mut spans = status_spans.clone();
                spans.push(Span::raw("  \u{00B7}  "));
                spans.push(Span::styled("score: ", dim));
                spans.push(Span::raw(score.clone()));
                spans.push(Span::raw("  \u{00B7}  "));
                spans.push(Span::styled("verdict: ", dim));
                spans.push(Span::styled(verdict.to_string(), verdict_style));
                lines.push(Line::from(spans));
                lines.push(Line::from(vec![
                    Span::styled("source: ", dim),
                    Span::raw(source.to_string()),
                ]));
                lines.push(Line::from(worktree_segment.clone()));
            }
            PreviewPlacement::Left => {
                lines.push(Line::from(status_spans));
                lines.push(Line::from(vec![
                    Span::styled("score: ", dim),
                    Span::raw(score.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("source: ", dim),
                    Span::raw(source.to_string()),
                ]));
                lines.push(Line::from(worktree_segment.clone()));
                lines.push(Line::from(vec![
                    Span::styled("verdict: ", dim),
                    Span::styled(verdict.to_string(), verdict_style),
                ]));
            }
        }
        lines.push(Line::from(format!("completed: {completed}")));
    } else {
        match placement {
            PreviewPlacement::Bottom => {
                let mut spans = status_spans;
                spans.push(Span::raw("  \u{00B7}  "));
                spans.push(Span::styled("score: ", dim));
                spans.push(Span::raw(score.clone()));
                lines.push(Line::from(spans));
                lines.push(Line::from(vec![
                    Span::styled("source: ", dim),
                    Span::raw(source.to_string()),
                ]));
                lines.push(Line::from(worktree_segment.clone()));
            }
            PreviewPlacement::Left => {
                lines.push(Line::from(status_spans));
                lines.push(Line::from(vec![
                    Span::styled("score: ", dim),
                    Span::raw(score.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("source: ", dim),
                    Span::raw(source.to_string()),
                ]));
                lines.push(Line::from(worktree_segment.clone()));
            }
        }
    }
    // Render the entity path relative to the workflow root so it fits the
    // preview header and stays Smart-Selection-clickable in terminals that
    // resolve relative paths against OSC 7. Fall back to the absolute path
    // for entities whose path sits outside the workflow root (e.g. worktree
    // copies), preserving the disambiguating context the absolute path
    // carries. Defensively reject an empty relative result (`strip_prefix`
    // returns `Ok("")` when the two paths are equal — render the absolute
    // path in that edge case so the value is never visibly empty).
    let path_full = match item.path.strip_prefix(state.workflow_dir()) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
        _ => item.path.display().to_string(),
    };
    // The header paragraph wraps with `Wrap { trim: true }`, which performs
    // word-wrapping at whitespace boundaries. A long path (no internal
    // whitespace) wraps at the single space after `path:` — leaving the
    // label alone on one row and the value on the next. To users, the label
    // appears EMPTY. Truncate the value with a leading ellipsis so the
    // basename stays visible and the line fits on one row.
    let path_text = fit_path_to_width(&path_full, inner_width as usize);
    lines.push(Line::from(format!("path: {path_text}")));

    // Body divider: "── body " + "─" repeated to fill pane width.
    // This replaces the previous blank separator line (same line count, but
    // now visually marks the boundary between metadata and body content).
    let prefix = "\u{2500}\u{2500} body "; // "── body " = 8 chars
    let fill_len = (inner_width as usize).saturating_sub(prefix.chars().count());
    let divider = format!("{}{}", prefix, "\u{2500}".repeat(fill_len));
    lines.push(Line::from(Span::styled(divider, dim)));

    lines
}

fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

/// Truncate `value` so that `"path: " + value` fits on a single row of the
/// given total `pane_width`. When truncation is required, drop characters from
/// the LEFT and replace them with a leading ellipsis (`…`) so the basename
/// stays visible. Returns the value unchanged when it already fits, and
/// returns just the ellipsis when even one character would not fit.
///
/// This exists because the preview header is rendered with
/// `Paragraph::new(...).wrap(Wrap { trim: true })`, which word-wraps at the
/// single space between the label and a long path — putting the label alone
/// on one row and the value on the next row, making the label appear empty.
/// See `path_line_stays_visible_for_long_paths` for the regression test.
pub(crate) fn fit_path_to_width(value: &str, pane_width: usize) -> String {
    let label_chars = "path: ".chars().count(); // = 6
    let available = pane_width.saturating_sub(label_chars);
    let value_chars = value.chars().count();
    if value_chars <= available {
        return value.to_string();
    }
    if available <= 1 {
        return "\u{2026}".to_string();
    }
    // Keep the trailing `(available - 1)` chars and prefix with `…`.
    let skip = value_chars - (available - 1);
    let tail: String = value.chars().skip(skip).collect();
    format!("\u{2026}{tail}")
}

/// Render the preview pane for a synthetic "broken" row. Surfaces the file
/// path, the underlying YAML error message, the line/column when derivable,
/// and a stable remediation hint pinned by tests.
fn render_broken_entity_preview(frame: &mut Frame<'_>, inner: Rect, err: &EntityParseError) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let red = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let path_display = err.path.display().to_string();
    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Preview  \u{00B7}  ", dim),
        Span::styled(format!("Cannot parse {path_display}"), red),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Error: ", dim),
        Span::raw(err.message.clone()),
    ]));
    if let (Some(line), Some(column)) = (err.line, err.column) {
        lines.push(Line::from(vec![
            Span::styled("Location: ", dim),
            Span::raw(format!("line {line}, column {column}")),
        ]));
    }
    lines.push(Line::from(Span::styled(BROKEN_ENTITY_HINT, dim)));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn wrapped_lines_height(lines: &[Line<'_>], width: u16) -> u16 {
    let width = usize::from(width.max(1));
    lines
        .iter()
        .map(|line| {
            let len = line_width(line).max(1);
            len.div_ceil(width) as u16
        })
        .sum()
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::cell::Cell;

    fn dummy(_body: &str, width: u16) -> Vec<Line<'static>> {
        vec![Line::from(format!("w{width}"))]
    }

    #[test]
    fn repeated_identical_render_is_a_cache_hit() {
        let mut cache = MarkdownCache::default();
        let calls = Cell::new(0usize);
        let path = PathBuf::from("/w/task-001.md");
        let _ = cache.get_or_render(&path, "body", true, 40, |b, w| {
            calls.set(calls.get() + 1);
            dummy(b, w)
        });
        let _ = cache.get_or_render(&path, "body", true, 40, |b, w| {
            calls.set(calls.get() + 1);
            dummy(b, w)
        });
        assert_eq!(calls.get(), 1, "identical second render must hit the cache");
    }

    #[test]
    fn body_edit_invalidates_same_path() {
        // Regression R5: a live-reload edit to the same task file must not serve
        // stale rendered lines — the content hash drops the key.
        let mut cache = MarkdownCache::default();
        let calls = Cell::new(0usize);
        let path = PathBuf::from("/w/task-001.md");
        let _ = cache.get_or_render(&path, "old body", true, 40, |b, w| {
            calls.set(calls.get() + 1);
            dummy(b, w)
        });
        let _ = cache.get_or_render(&path, "new body", true, 40, |b, w| {
            calls.set(calls.get() + 1);
            dummy(b, w)
        });
        assert_eq!(
            calls.get(),
            2,
            "edited body must re-render, not serve stale"
        );
    }

    #[test]
    fn wrap_toggle_invalidates() {
        let mut cache = MarkdownCache::default();
        let calls = Cell::new(0usize);
        let path = PathBuf::from("/w/task-001.md");
        let _ = cache.get_or_render(&path, "body", true, 40, |b, w| {
            calls.set(calls.get() + 1);
            dummy(b, w)
        });
        let _ = cache.get_or_render(&path, "body", false, 40, |b, w| {
            calls.set(calls.get() + 1);
            dummy(b, w)
        });
        assert_eq!(calls.get(), 2, "wrap toggle must invalidate the cache");
    }

    #[test]
    fn both_two_pass_widths_are_retained() {
        // Mirrors the wrap+scrollbar two-pass (full width, then narrowed width):
        // re-using the full width next frame must hit, not thrash.
        let mut cache = MarkdownCache::default();
        let calls = Cell::new(0usize);
        let path = PathBuf::from("/w/task-001.md");
        for w in [40u16, 39, 40] {
            let _ = cache.get_or_render(&path, "body", true, w, |b, ww| {
                calls.set(calls.get() + 1);
                dummy(b, ww)
            });
        }
        assert_eq!(
            calls.get(),
            2,
            "both pass widths cached; width 40 must re-hit"
        );
    }

    #[test]
    fn returns_the_render_output_for_the_requested_width() {
        let mut cache = MarkdownCache::default();
        let path = PathBuf::from("/w/task-001.md");
        let out = cache.get_or_render(&path, "body", true, 42, dummy);
        assert_eq!(out, vec![Line::from("w42".to_string())]);
    }
}
