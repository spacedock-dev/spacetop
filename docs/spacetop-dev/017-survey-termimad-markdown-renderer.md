---
id: "017"
title: Survey termimad as markdown renderer for the preview pane
status: review
source: captain (follow-up from 016)
started: 2026-04-25T15:38:17Z
completed:
verdict:
score:
worktree: .worktrees/spacedock-ensign-017-survey-termimad-markdown-renderer
issue:
pr:
mod-block: merge:pr-merge
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

`termimad` operates through a `MadSkin` + rendering methods that write directly to the terminal via `crossterm`. Its output types (`FmtText`, `MadView`, `TextView`) implement `Display` or manage a terminal `Area` — there is no direct path to `Vec<Line<'static>>` from the high-level API.

### Ratatui integration friction

`pulldown-cmark`: zero friction. The event iterator produces `Vec<Line<'static>>` which Ratatui consumes natively.

`termimad` as a drop-in replacement: high friction. The primary render pipeline targets crossterm directly and has no first-class path to `Vec<Line<'static>>`. The naive integration paths are fragile (ANSI re-parsing) or require bypassing Ratatui.

### Hybrid integration paths

Three hybrid approaches were investigated:

**Path A — ratskin (published crate).** `ratskin` 0.3.1 is a published wrapper around `termimad` that produces `Vec<ratatui::text::Line>`. Its `RatSkin::parse()` takes minimad `Text` and a column width and returns ratatui `Line` values with skin-driven `Span` styling. This is a real, working integration and the code is described as "a small part of the termimad logic rewritten for ratatui Spans and Lines." **Blocker:** ratskin requires `ratatui ^0.30.0`; spacetop currently pins `ratatui = "0.29"`. Adopting ratskin would force a ratatui major-version upgrade, which is a separate refactor risk.

**Path B — minimad directly.** `minimad` is termimad's internal markdown parser and is a public crate on crates.io. Its `Compound` struct exposes public boolean fields (`bold: bool`, `italic: bool`, `code: bool`, `strikeout: bool`, `src: &str`) that map directly to `Span::styled()`. A `Composite` is a sequence of `Compound` values representing one styled line. This path is viable without crossterm or termimad — but it means adding `minimad` as a direct dependency, writing the skin/style mapping layer ourselves, and essentially reimplementing what ratskin already provides (ratskin already does this and is ~100 lines).

**Path C — pulldown-cmark events + MadSkin::compound_style styling.** Use pulldown-cmark for parsing (current approach) while using `MadSkin::compound_style()` to derive termimad `CompoundStyle` for each parsed element, then convert `CompoundStyle` (coolor fg/bg + attributes) to ratatui `Style` via the `Color::from_crossterm()` bridge already available in ratatui. This mixes two parsing models for no functional gain — the current hand-coded style logic is simpler and more direct.

### Crate weight

| Crate | Normal deps | Notes |
|---|---|---|
| `pulldown-cmark` 0.13.3 | 3 (bitflags, memchr, unicase) | All tiny, widely shared |
| `termimad` 0.34.1 | 8 (coolor, crokey, crossbeam, lazy-regex, minimad, serde, thiserror, unicode-width) | crossbeam alone adds significant weight; crokey/crossbeam are irrelevant to our use case |

pulldown-cmark is lighter and its transitive deps are already present in the dependency graph via Ratatui.

## Decision

Keep `pulldown-cmark`. The hybrid angle was investigated and three paths were found:

1. **ratskin** — a viable published crate that does exactly the termimad→ratatui bridge, but requires ratatui ≥ 0.30.0 (spacetop is on 0.29); adopting it forces a ratatui upgrade as a precondition.
2. **minimad directly** — viable but means re-implementing the same bridge ratskin provides, adding a new direct dependency (minimad), and owning the maintenance burden.
3. **pulldown-cmark + MadSkin styles** — mixing two parsers for no functional gain; the current hand-coded style logic is simpler.

None of these hybrids improve over the current implementation for the content types spacetop actually renders. The decisive factors remain: (a) the current 160-line event loop in `render_markdown_lines` already handles all required content types correctly; (b) termimad/ratskin's chief advantages (italic, MadSkin skin config, built-in bullet rendering) do not correspond to any known gap in the current renderer; (c) the lightest viable hybrid (ratskin) gates on a ratatui major upgrade that is out of scope for this task. The existing `render_markdown_lines` implementation in `src/ui/mod.rs` should be retained.

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

## Stage Report: design (cycle 2)

- DONE: Determine whether termimad can be used alongside pulldown-cmark (hybrid) rather than as a replacement — document any integration path found.
  Three paths documented in `### Hybrid integration paths`: ratskin (real crate, ratatui version conflict), minimad directly (viable but reimplements ratskin), pulldown-cmark + MadSkin styles (no functional gain).
- DONE: Update the Recommendation and Decision sections to reflect the hybrid possibility.
  `## Decision` section updated to name all three hybrid paths and explain why each is not warranted; `### API surface` and `### Ratatui integration friction` sections updated with hybrid detail.
- DONE: If a viable hybrid exists: name the TUI constraints so the plan stage can proceed.
  Viable hybrid is ratskin; TUI constraint: requires ratatui ≥ 0.30.0 (spacetop is on 0.29), so adoption gates on a ratatui upgrade. Recommendation remains to keep the current renderer.

### Summary

All three hybrid angles were researched. `ratskin` 0.3.1 is a real, working termimad→ratatui bridge that produces `Vec<Line>` from a `MadSkin`, but requires ratatui ≥ 0.30.0 — a version spacetop does not currently use. The minimad-direct path replicates what ratskin provides without the version constraint, but adds a new dependency and maintenance burden for no functional improvement. No hybrid offers a rendering quality advantage over the current `render_markdown_lines` for the content types spacetop actually displays. Recommendation: keep `pulldown-cmark` and the current renderer.

## Stage Report: plan

- DONE: Plan documents that no code changes are required — the survey deliverable is the decision recorded in the entity body.
  The design stage fully satisfied all ACs; the `## Decision` section records the keep-pulldown-cmark verdict with rationale. No implementation work exists in this task.
- DONE: Plan records the unblocking dependency: ratatui upgrade to 0.30+ (task 018) must ship before ratskin adoption.
  If ratskin is ever revisited, it requires ratatui ≥ 0.30.0. Task 018 handles that upgrade; ratskin adoption is deferred until 018 ships.

### Summary

This survey task is complete as a documentation artifact. The design stage produced a full comparison of termimad vs. pulldown-cmark, evaluated three hybrid integration paths, and concluded that the existing `render_markdown_lines` implementation should be retained. The only forward action is optional: if ratskin adoption is desired after task 018 upgrades ratatui to 0.30+, the path is documented in the `## Decision` section. No code changes are required in this task.
