---
id: "028"
title: "Refine TUI list view — Tokyo Night palette, DAG header, task row layout"
status: plan
source: feature request
started: 2026-04-26T05:37:33Z
completed:
verdict:
score: 0.85
worktree:
issue:
pr:
---

Overhaul the spacetop list view to a strict terminal-safe aesthetic (text + box-drawing only) with a Tokyo Night-ish muted palette, a centered workflow DAG header, and a redesigned task row layout. All output must survive a plain terminal font with no emoji or decorative icons outside the defined glyph vocabulary.

## Spec

### Header strip (single line)

```
Workflow [active]  archived: (press a)  /path/to/workflow
```

- `[active]` — yellow filled badge, bg-colored bold text
- `archived: (press a)` — muted; `(press a)` is a key hint
- Path — dimmed; truncate from the **left** with `…` on overflow

---

### Workflow DAG (centered, monospace grid)

**Main row:**

```
▶ pending  ──►  ⚑ ⎇ hypothesis  ──►  ⎇ smoke  ──►  ⎇ run  ──►  ⚑ analyze  ──►  ⎇ n-runs  ──►  ⎇ holdout  ──►  done ■
```

**Glyph vocabulary** (fixed, not derived from phase names):

| Glyph | Meaning |
|-------|---------|
| `▶` | Start of pipeline (initial stage) |
| `⎇` | Branchable / regular stage |
| `⚑` | Gate / checkpoint stage |
| `■` | Terminal stage |

**Per-phase color:** oklch-derived, shared lightness 0.78 and chroma 0.12, hue varies per stage index. Active phase: `bg = phase color`, `fg = bg color`, bold.

**Counts row:** integer count centered under each phase label, dimmed.

**Rollback arc** (drawn below the main row in red, box-drawing chars):

```
          ↑   reject     │
          ╰──────────────╯
```

Spans from the anchor under the first char of the `feedback-to` target stage to the anchor under the first char of the stage being rejected (i.e. the `feedback-to` arrow in reverse). Uses box-drawing characters only.

---

### Task list rows

```
  run         17  README ablation arm 1 — direct single agent with minimal README
▸ smoke       18  README ablation arm 2 — direct single agent with methodology …
  hypothesis  11  Query batching — all queries visible, shared exploration
```

- **Gutter:** 2-char wide; `▸` for selected row, space otherwise
- **Phase column:** phase name only — no glyph, user's original casing preserved, fixed 12-char width, ellipsized with `…` if longer. Color = phase color. No auto-uppercasing or title-casing.
- **ID column:** 4-char right-aligned
- **Title column:** fills remaining width, ellipsized with `…`
- **Selected row:** `bg-2` background fill + 2-cell yellow left border
- **No glyphs in the task list phase column** — color alone carries the signal there

---

### Section headers

```
Tasks · 12    Preview · #17
```

Muted, normal (sentence) case, no uppercasing. Slight letter-spacing where the terminal supports it.

---

### Footer

Muted hint bar. Key hints (`?` `/` `⏎` `→` `a` `q`) shown in subtle `bg-2` pill style.

---

### Half-width tmux variant (~80–90 columns)

- Header condenses to one tight line
- DAG wraps to 2 rows: row 1 ends at the last stage before the midpoint (carry the reject arc on row 1), row 2 continues from the next stage
- Tasks and preview stack vertically (tasks on top, preview below)
- Footer key hints split across 2 short lines

---

### Anti-patterns to avoid

- **No 3-letter abbreviations** — do not auto-derive `RUN`, `HYP`, `DON` from phase names
- **No force-casing** — preserve user-defined phase name casing exactly
- **No glyphs in the task-list phase column** — glyphs appear only in the DAG
- **No decorative emoji or icons** outside the glyph vocabulary (`▶ ⎇ ⚑ ■ ──► ↑ ╰ ╯ ▸ …`)
- **No Unicode that won't render** in a typical terminal font

## Acceptance criteria

**AC-1 -- Header strip renders correctly.**
Single-line header shows: yellow badge for active count, muted archived hint with key callout, left-truncated dimmed path. Verified by: snapshot test asserting badge color, muted style, and left-truncation on a short terminal width.

**AC-2 -- DAG glyphs and colors are correct.**
Each stage is prefixed with the correct glyph (`▶`, `⎇`, `⚑`, `■`) based on its `initial`/`gate`/`terminal` properties. Active stage renders with inverted bg/fg. Non-active stages use per-stage hue with shared oklch lightness/chroma. Verified by: snapshot test on a known workflow fixture.

**AC-3 -- Rollback arc renders with box-drawing chars only.**
When a stage has `feedback-to`, the rollback arc is drawn below the DAG row using `↑ ╰ ─ ╯` characters in red. Verified by: snapshot test asserting arc characters and color.

**AC-4 -- Task list rows match spec.**
Gutter shows `▸` on selected row. Phase column is 12 chars fixed, user casing preserved, no glyphs, colored. ID is 4-char right-aligned. Title ellipsized. Selected row has yellow left border and bg-2 fill. Verified by: snapshot tests for selected/unselected rows and long phase name ellipsis.

**AC-5 -- Half-width variant wraps correctly.**
At ~80 columns the DAG wraps to 2 rows and tasks/preview stack vertically. Verified by: snapshot test at width=80.

**AC-6 -- Anti-patterns are absent.**
No uppercase phase names, no glyphs in task-list phase column, no emoji outside the vocabulary. Verified by: snapshot test assertions on rendered buffer.

## Stage Report: design

- DONE: Problem statement maps each spec section (header, DAG, task rows, footer, half-width) to the exact src/ui files and structs that need to change.
  See mapping below.
- DONE: oklch color approach is confirmed feasible in ratatui — either via Color::Rgb approximations or an oklch-to-srgb conversion at init time.
  ratatui 0.29/0.30 exposes `Color::Rgb(u8, u8, u8)` (confirmed in ratatui-0.29.0/src/style/color.rs); no external crate is needed.

### File-to-spec mapping

**Header strip** (`src/ui/mod.rs` — `render_header_bar`, lines ~179–223):
- Needs left-truncation logic for path (currently appended verbatim).
- Badge style currently sets `fg(Color::Yellow)` only; spec calls for a filled yellow bg with dark fg text.
- `archived_hint` format needs to separate count from the key hint with the right spacing.

**DAG / graph** (`src/ui/graph.rs` — entire file; `src/domain/mod.rs` — `assign_stage_colors`, `GRAPH_PALETTE`, `stage_color`):
- `GlyphSet.worktree` is currently used for branchable stages (⎇); spec calls `⎇` the "branchable" glyph and wants `⚑` for gate stages — mapping already matches but `build_node_text` stacks leading glyphs; the spec shows a single glyph per stage, so the per-stage role must resolve to exactly one prefix glyph.
- `assign_stage_colors` in `src/domain/mod.rs` uses named `Color::*` variants; must be replaced with oklch-to-srgb conversion producing `Color::Rgb` values at shared lightness=0.78, chroma=0.12, hue varying by stage index.
- Rollback arc currently uses `↑ ╰ ─ ╯` glyphs (already correct), but arc drawing logic is in `render_wide` inside `graph.rs` — confirm the arc is drawn red and spans correct columns per spec.

**Task list rows** (`src/ui/mod.rs` — `render_task_list`, `build_task_list_items`, lines ~373–481):
- `stage_tag()` (lines 80–94) produces 3-letter uppercase abbreviations (e.g., `RUN`, `DES`) and is the primary anti-pattern to remove.
- `build_task_list_items` must switch to the raw stage name (user casing), fixed 12-char column with `…` ellipsis, and right-aligned 4-char ID.
- Selected row currently uses ratatui `List` `highlight_symbol("> ")` + `REVERSED | BOLD`; spec requires a 2-cell yellow left border + `bg-2` background fill — this may require switching from `List` to manual `Paragraph` row rendering or a custom `ListItem` with explicit styled spans for each cell.
- Gutter column (▸ vs space) replaces the `"> "` highlight symbol.

**Section headers** (`src/ui/mod.rs` — inside `render_task_list`, lines ~386–402):
- Already muted; format is `Tasks  ·  N` — spec says `Tasks · 12` with single spaces around `·`. Minor spacing tweak only.

**Footer** (`src/ui/mod.rs` — `render_status_footer`, lines ~275–301):
- Currently a plain dim joined string. Spec requires pill-style subtle `bg-2` wrapping for each key hint. Each hint needs its own `Span` with a background highlight.

**Half-width variant** (`src/ui/mod.rs` — `preview_placement` at line ~167, `render_overview` layout constraints at lines ~123–138, `graph.rs` — `pick_width_tier`):
- `pick_width_tier` already has `WidthTier::Narrow` and `WidthTier::VeryNarrow`; the narrow path (`render_narrow`) must produce 2 DAG rows at ~80 cols.
- `preview_placement` already switches to `Bottom` at ~80 cols; tasks-on-top / preview-below stacking is already implemented.
- Footer 2-line split at narrow width must be added to `render_status_footer`.

### oklch-to-srgb approach

The spec's formula (lightness=0.78, chroma=0.12, hue=stage_index × (360/N)) can be computed in pure Rust at init time using the standard oklch→linear-sRGB→gamma-sRGB pipeline:
1. Convert oklch → oklab: `a = chroma * cos(hue_rad)`, `b = chroma * sin(hue_rad)`.
2. Convert oklab → linear-sRGB via the published 3×3 matrix.
3. Apply sRGB gamma (`if c <= 0.0031308 { 12.92*c } else { 1.055*c.powf(1.0/2.4) - 0.055 }`).
4. Clamp to [0,1] and scale to `u8` for `Color::Rgb`.
No external crate required. The conversion is ~15 lines of Rust and can live in a new `src/palette.rs` module or inline in `src/domain/mod.rs`.

### Summary

Surveyed all five spec sections against the live source. Each section maps to one or two specific functions in `src/ui/mod.rs`, `src/ui/graph.rs`, or `src/domain/mod.rs`. The biggest behavioral changes are: (1) removing `stage_tag()` and switching to raw user-casing phase names in the task list, (2) replacing the `GRAPH_PALETTE` named-color system with an oklch-derived `Color::Rgb` computation, and (3) reworking the selected-row highlight from ratatui's `List` highlight to explicit per-cell span styling. `Color::Rgb` is confirmed available in ratatui 0.29/0.30 with no additional dependencies needed.
