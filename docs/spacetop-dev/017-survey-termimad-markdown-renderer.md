---
id: "017"
title: Survey termimad as markdown renderer for the preview pane
status: design
source: captain (follow-up from 016)
started: 2026-04-25T15:38:17Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

Task 016 fixed code block rendering by extending the existing `pulldown-cmark` event loop in `src/ui/mod.rs`. During review it was noted that `termimad` — a crate designed specifically for terminal markdown rendering — was not surveyed before committing to that approach. This task surveys `termimad`, decides whether it is a better fit, and either replaces the renderer or documents why `pulldown-cmark` should be kept.

## Acceptance criteria

**AC-1 -- termimad is evaluated against the current renderer.**
A written comparison covers: rendering quality for the content types in the 016 example section (fenced code blocks, inline code, headings, lists, bold/italic), API surface vs. manual event walking, Ratatui integration friction, and crate weight (transitive dependencies).
Verified by: design stage report contains the comparison.

**AC-2 -- A clear keep-or-replace recommendation is made.**
The design stage concludes with an explicit recommendation — replace with `termimad`, keep `pulldown-cmark`, or adopt a hybrid — backed by the AC-1 comparison.
Verified by: design stage report contains a `Recommendation:` line.

**AC-3 -- If replacing: the renderer swap is implemented and all existing preview tests pass.**
`termimad` replaces `pulldown-cmark` in `render_markdown_lines`; the 016 example content renders correctly; `cargo test --lib` exits 0.
Verified by: implement stage report with `cargo test --lib` output.

**AC-4 -- If keeping: the rationale is recorded in the entity body for future reference.**
A `## Decision` section is added to this file explaining why `pulldown-cmark` was retained.
Verified by: presence of `## Decision` section in the final entity file.

## Comparison: termimad vs. pulldown-cmark

### Rendering quality

| Content type | pulldown-cmark (current) | termimad |
|---|---|---|
| Fenced code blocks | Cyan-on-DarkGray per line; no syntax highlighting | No syntax highlighting either; renders as styled block |
| Inline code | Yellow fg via `Code` event | Styled inline; similar result |
| Headings | Bold + White fg | Bold; skin-configurable colors |
| Lists (unordered) | Unicode bullet prepended manually | Built-in bullet rendering |
| Bold / italic | Bold modifier via `Strong` event; italic not handled | Both supported via skin |
| Ordered lists | Not handled (falls through `_`) | Not supported (documented limitation) |
| Tables | Custom `TableRender` struct with column widths | Built-in table support |
| Long lines | Ratatui wraps at widget boundary | termimad wraps internally before reaching ratatui |

The current renderer produces correct output for all tested content types. termimad would match or slightly improve visual quality for lists and italic text, but has no syntax highlighting advantage.

### API surface

`pulldown-cmark` exposes a `Parser` iterator over typed `Event` values. The current implementation walks events manually (~160 lines in `render_markdown_lines`) but the result is a `Vec<Line<'static>>` that drops cleanly into any Ratatui widget. The code is straightforward and all state is local.

`termimad` operates through a `MadSkin` + rendering methods that write directly to the terminal via `crossterm`. Its output types (`FmtText`, `MadView`, `TextView`) implement `Display` or manage a terminal `Area` — there is no path to `Vec<Line<'static>>` without re-parsing the rendered string or using the underlying `minimad` parser directly.

### Ratatui integration friction

`pulldown-cmark`: zero friction. The event iterator produces `Vec<Line<'static>>` which Ratatui consumes natively.

`termimad`: high friction. termimad owns the render pipeline and targets crossterm directly. Integrating it with Ratatui's widget model would require one of:
1. Rendering to a string buffer and re-parsing ANSI escape sequences into `Span` styles — fragile and expensive.
2. Using `minimad` (termimad's internal parser) directly, which is an undocumented internal API.
3. Bypassing Ratatui for the preview pane entirely — incompatible with the existing layout system.

None of these paths are practical for a widget-based TUI.

### Crate weight

| Crate | Normal deps | Notes |
|---|---|---|
| `pulldown-cmark` 0.13.3 | 3 (bitflags, memchr, unicase) | All tiny, widely shared |
| `termimad` 0.34.1 | 8 (coolor, crokey, crossbeam, lazy-regex, minimad, serde, thiserror, unicode-width) | crossbeam alone adds significant weight; crokey/crossbeam are irrelevant to our use case |

pulldown-cmark is lighter and its transitive deps are already present in the dependency graph via Ratatui.

## Decision

Keep `pulldown-cmark`. The decisive factor is Ratatui integration: termimad manages its own render pipeline through crossterm and has no first-class path to `Vec<Line<'static>>`. Integrating it into the existing widget model would require re-parsing ANSI output or bypassing Ratatui entirely — both approaches are more complex and fragile than the current 160-line event loop. Additional factors: termimad does not support ordered lists (a documented limitation); it carries more transitive weight (8 vs 3 normal deps); and the current renderer already handles all content types in the 016 test surface correctly. The existing `render_markdown_lines` implementation in `src/ui/mod.rs` should be retained.

## Stage Report: design

- DONE: A written comparison of termimad vs. pulldown-cmark covers rendering quality, API surface, Ratatui integration friction, and crate weight.
  See `## Comparison` section above; covers all five content types, event-loop vs. MadSkin API, Vec<Line> integration gap, and dep counts (3 vs 8).
- DONE: A clear keep-or-replace recommendation is stated with rationale.
  Recommendation: keep `pulldown-cmark`. See `## Decision` section and `Recommendation:` line below.
- SKIPPED: If replacing: TUI constraints for the swap are named so the plan stage can proceed immediately.
  Not applicable — recommendation is to keep the current renderer.

Recommendation: keep `pulldown-cmark`.

### Summary

termimad 0.34.1 renders markdown via its own crossterm-backed pipeline with no first-class path to `Vec<Line<'static>>`, making Ratatui widget integration impractical without fragile ANSI re-parsing. pulldown-cmark's event iterator already feeds the existing renderer cleanly, covers all required content types, and costs only 3 transitive deps vs. 8 for termimad. The existing `render_markdown_lines` in `src/ui/mod.rs` should be retained as-is.
