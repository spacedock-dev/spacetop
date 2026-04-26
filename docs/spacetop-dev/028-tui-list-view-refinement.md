---
id: "028"
title: "Refine TUI list view — Tokyo Night palette, DAG header, task row layout"
status: review
source: feature request
started: 2026-04-26T05:37:33Z
completed:
verdict:
score: 0.85
worktree: .worktrees/spacedock-ensign-028-tui-list-view-refinement
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

## Implementation Plan

### Step 1 — oklch palette + color infrastructure

**File:** `src/domain/mod.rs` (and optionally extracted to `src/palette.rs`)

**What:**
- Add a pure-Rust `oklch_to_srgb(l: f32, c: f32, h_deg: f32) -> (u8, u8, u8)` function using the standard pipeline:
  1. oklch → oklab: `a = c * cos(h_deg.to_radians())`, `b = c * sin(h_deg.to_radians())`
  2. oklab → linear-sRGB via the published 3×3 matrix
  3. Apply sRGB gamma (`if c <= 0.0031308 { 12.92*c } else { 1.055*c.powf(1.0/2.4) - 0.055 }`)
  4. Clamp to [0.0, 1.0] and multiply by 255 for `u8`
- Replace `assign_stage_colors` to use this function with lightness=0.78, chroma=0.12, hue=`index * (360.0 / n as f32)` for each stage index.
- Delete `GRAPH_PALETTE`, `preferred_color`, and `pick_color` (they are fully replaced by the oklch computation).
- Keep `stage_color()` as a legacy fallback only for `WorkflowDefinition::stage_color_for` when stage is not in the map (unknown archived stages).
- Update `WorkflowDefinition::stage_color_for` — no interface change needed; it already calls `assign_stage_colors` at parse time.

**Verification:**
```
cargo test -p spacetop domain -- --nocapture
```
Add a unit test `oklch_palette_produces_rgb_values` that calls `oklch_to_srgb` for 5 evenly-spaced hues and asserts all returned values are distinct `Color::Rgb` variants in range [0, 255].

---

### Step 2 — Header strip

**File:** `src/ui/mod.rs` — `render_header_bar` (lines ~179–223)

**What:**
- **Badge style:** change active badge from `fg(Color::Yellow)` to `fg(Color::Black).bg(Color::Yellow)` (filled yellow bg, dark fg text).
- **Archived hint format:** current: `"archived: {n}  (press a)"`. Spec: `"archived: (press a)"` when count unknown, and `"archived: {n}  (press a)"` when known — the `(press a)` part should be styled as a distinct key hint span.
- **Left-truncation for path:** compute available width after all other spans; if `path_str` is longer, replace the leftmost characters with `…` so the string fits.
- Drop the `─` separator fill; instead use left-truncation as the overflow guard and trailing space padding.

**Verification:**
```
cargo test -p spacetop ui::tests::header -- --nocapture
```
New snapshot test `header_strip_badge_style_and_path_truncation`: render at width=60 with a long path; assert `[active]` cell has yellow bg, path is left-truncated with `…`.

---

### Step 3 — DAG glyphs and rollback arc color

**File:** `src/ui/graph.rs`

**What:**
- **Single glyph per stage:** `build_node_text` currently stacks `gate + worktree + initial` leading glyphs. Change to resolve exactly one prefix glyph per stage in priority order: `initial` → `▶`, `terminal` → append `■` suffix (keep existing), `gate` → `⚑`, `worktree` (branchable) → `⎇`, else no prefix. This makes every stage have at most one leading glyph.
- **Per-stage oklch colors in ribbon:** `render_wide` already calls `definition.stage_color_for(&col.stage_name)` which now returns `Color::Rgb` from the new palette — no change to rendering logic needed; the color source changes.
- **Active stage inversion:** keep `Modifier::REVERSED` for active stage (bg/fg swap on terminal); spec says "bg = phase color, fg = bg color, bold" which is what REVERSED achieves in a color terminal. No change needed.
- **Rollback arc in red:** add `Style::default().fg(Color::Red)` to the arc line spans in `render_wide`. Currently arc lines are pushed as `Line::from(format!(...))` raw strings with no color — change to `Line::from(Span::styled(..., Style::default().fg(Color::Red)))`.
- **Counts row dimmed:** apply `Style::default().add_modifier(Modifier::DIM)` to counts spans (currently unstyled for non-active counts).

**Verification:**
```
cargo test -p spacetop graph -- --nocapture
```
Existing tests continue to pass. Add/update:
- `dag_single_glyph_per_stage`: asserts no stage node text has two consecutive glyphs from the vocabulary set.
- `rollback_arc_is_red`: render with TestBackend, check that cells containing `╰` or `╯` have `fg = Color::Red`.

---

### Step 4 — Task row layout and stage_tag() removal

**File:** `src/ui/mod.rs` — `stage_tag` (lines ~80–94), `build_task_list_items` (lines ~430–481), `render_task_list` (lines ~373–428)

**Call sites of `stage_tag`:** only one — `build_task_list_items` at line ~449:
```rust
let tag = stage_tag(&item.status);
```
No other call site exists in the codebase (confirmed by search).

**What:**
- **Delete `stage_tag()`** entirely.
- **Phase column:** replace `tag` with a fixed-width 12-char phase name. Helper:
  ```rust
  fn phase_col(stage: &str) -> String {
      let s: String = stage.chars().take(11).collect();
      if stage.chars().count() > 12 {
          format!("{s}\u{2026}") // append …
      } else {
          format!("{:<12}", stage)
      }
  }
  ```
  Preserves user casing exactly; no glyph.
- **ID column:** change from `{:>3}` to `{:>4}` (4-char right-aligned).
- **Gutter column:** stop using `List::highlight_symbol("> ")`. Switch to including gutter as the first span in each row:
  - selected: `Span::styled("\u{258C} ", ...)` — `▌` in yellow (2 chars: glyph + space) for yellow left border effect
  - unselected: `Span::raw("  ")` (2 spaces)
  - Set `highlight_symbol("")` on the `List` widget (empty, since gutter is now in the spans).
- **Selected row bg-2 fill:** apply `Style::default().bg(Color::Rgb(41, 45, 62))` to all spans of the selected row (a dark navy approximating Tokyo Night bg-2). The selected index is available from `state.selected_index()`.
- **Yellow left border:** style the gutter span of selected row as `Style::default().fg(Color::Yellow).bg(Color::Rgb(41, 45, 62))`.
- **Archived rows:** keep the verdict glyph appended as before.

**Verification:**
```
cargo test -p spacetop ui::tests -- --nocapture
```
New snapshot tests:
- `task_row_phase_column_12_char_fixed`: render a task with status "implement"; assert phase cell is exactly 12 chars, user casing preserved.
- `task_row_long_phase_name_ellipsis`: render a task with status "averylongphasename" (18 chars); assert phase cell is 12 chars ending with `…`.
- `task_row_selected_gutter`: render with a selection; assert selected row gutter contains `▌` and unselected rows have `  ` (two spaces).
- `task_row_no_uppercase_phase`: assert no rendered row cell is all-uppercase for a known stage like "implement".
- `task_row_no_glyphs_in_phase_col`: assert phase column cells do not contain `▶`, `⎇`, `⚑`, or `■`.

---

### Step 5 — Footer pill hints

**File:** `src/ui/mod.rs` — `render_status_footer` (lines ~275–301)

**What:**
- Replace the single joined string with a `Vec<Span>` where each hint is its own styled span: `Span::styled(hint, Style::default().fg(Color::Black).bg(Color::Rgb(59, 66, 82)))` (a muted slate background approximating Tokyo Night bg-2 for pill effect), interspersed with `Span::raw("  ")` separators.
- Key letters within each hint (e.g. `?`, `/`, `Enter`, `a`, `q`) can optionally have a slightly brighter bg to differentiate the key from the description text — implement as two sub-spans: `Span::styled(key, key_style)` + `Span::styled(" description", hint_style)`.
- For now, implement as a single bg-colored span per hint (simpler); refine to sub-spans if time allows.

**Verification:**
```
cargo test -p spacetop ui::tests::footer -- --nocapture
```
New test `footer_hints_have_background`: render footer, assert at least one cell has a non-default background color.

---

### Step 6 — Half-width variant (narrow DAG wrap + footer split)

**Files:** `src/ui/graph.rs` — `render_narrow`, `src/ui/mod.rs` — `render_status_footer`

**What (DAG):**
- `render_narrow` currently renders a compact single-line summary. Spec requires 2 DAG rows at ~80 cols: row 1 ends at the last stage before the midpoint (carry the reject arc), row 2 continues.
- Algorithm: compute midpoint index `mid = stages.len() / 2`. Row 1 = stages `[0..mid]` rendered with `build_node_text` and forward arrows; row 2 = stages `[mid..]`. Append reject arc lines to row 1's output if any `feedback_to` spans stages in row 1.
- The existing `pick_width_tier` already selects `WidthTier::Narrow` at ~80 cols; `render_narrow` just needs its output changed to 2 ribbon rows instead of the compact summary.

**What (footer):**
- At narrow width (area.width <= 90), split hints into 2 groups; render as 2 lines.
- `render_status_footer` currently renders 1 `Paragraph` line. Change to a 2-line `Paragraph` when narrow: first line has primary hints (`?`, `Enter`, `a`, `q`), second line has secondary hints (`←/→`, `PgUp/PgDn`, `w`).
- Footer area height is currently 1; if narrow, the overview layout must allocate 2 rows for footer. Change `Constraint::Length(1)` for footer in `render_overview` to `Constraint::Length(footer_height(area.width))` where `footer_height` returns 2 when width <= 90, else 1.

**Verification:**
```
cargo test -p spacetop ui::tests::narrow -- --nocapture
```
New snapshot test `narrow_dag_wraps_to_two_rows`: render at width=80, height=20; assert the rendered buffer contains two separate lines with stage names from a known workflow (first half on row N, second half on row N+1).

---

### Test strategy summary for all 6 ACs

| AC | Test name | File | Assertion |
|----|-----------|------|-----------|
| AC-1 | `header_strip_badge_style_and_path_truncation` | `src/ui/mod.rs` tests | yellow bg on badge cell, `…` prefix on truncated path |
| AC-2 | `dag_single_glyph_per_stage`, `dag_oklch_colors_are_rgb` | `src/ui/graph.rs` tests | single prefix glyph, `Color::Rgb` on node cells |
| AC-3 | `rollback_arc_is_red` | `src/ui/graph.rs` tests | arc cells have `fg = Color::Red` |
| AC-4 | `task_row_phase_column_12_char_fixed`, `task_row_selected_gutter`, `task_row_no_uppercase_phase`, `task_row_no_glyphs_in_phase_col` | `src/ui/mod.rs` tests | column widths, gutter chars, casing, no-glyph |
| AC-5 | `narrow_dag_wraps_to_two_rows` | `src/ui/mod.rs` tests | two ribbon rows at width=80 |
| AC-6 | `task_row_no_uppercase_phase`, `task_row_no_glyphs_in_phase_col` | `src/ui/mod.rs` tests | combined anti-pattern assertions |

All tests use `ratatui::backend::TestBackend` with fixed width/height to inspect `Buffer` cell content and styles. No TUI session required; all tests run headless with `cargo test`.

### Execution order and independence

Steps 1–3 are logically independent (palette infra, header, DAG) and can be implemented in parallel by separate worktree branches. Steps 4–6 depend only on Step 1 (for the color lookup). Recommended sequence for a single-worktree execution:

1 → 2 → 3 → 4 → 5 → 6

Each step has a `cargo test` verification command so work can be committed and tested incrementally.

## Stage Report: plan

- DONE: Plan breaks work into independently testable steps: (a) oklch palette + color infra, (b) header strip, (c) DAG glyphs/arc, (d) task row layout with stage_tag removal, (e) footer, (f) half-width variant.
  Six steps defined above with files, what-to-change, and cargo test commands for each.
- DONE: Plan addresses the stage_tag() removal explicitly — names every call site and the replacement approach (raw stage name, 12-char fixed column).
  Only call site: `build_task_list_items` line ~449. Replacement: `phase_col()` helper producing 12-char fixed-width user-casing string. `stage_tag()` deleted entirely.
- DONE: Plan specifies the snapshot test strategy for all 6 ACs.
  AC-to-test mapping table above lists test name, file, and assertion for each of the 6 acceptance criteria.

### Summary

The plan breaks implementation into 6 independently testable steps mapped to specific functions in `src/ui/mod.rs`, `src/ui/graph.rs`, and `src/domain/mod.rs`. The single `stage_tag()` call site in `build_task_list_items` is explicitly identified and replaced with a `phase_col()` helper preserving user casing in a 12-char fixed column. All 6 ACs are covered by named snapshot tests using `ratatui::backend::TestBackend`, runnable headless with `cargo test`.

### Feedback Cycles

**Cycle 1** — review rejected implement. Findings routed back to implement.

Defects:
1. AC-5 not implemented — `render_narrow` unchanged; no 2-row DAG wrap at ~80 columns; `narrow_dag_wraps_to_two_rows` test missing.
2. Named-color fallback still present in `stage_color()` (src/domain/mod.rs lines 67–102); checklist said all named-color fallbacks gone.
3. AC-6 glyph test trivially-true — `task_row_no_glyphs_in_phase_col` never scans the rendered buffer; only checks helper in isolation.

**Cycle 2** — gate rejection (captain). Selected task row highlight too similar to background.

Current: `BG2 = Color::Rgb(41, 45, 62)` on bg `~Rgb(26, 27, 38)` — only ~15 brightness delta, not conspicuous enough.
Required: Use a more visually distinct selection color. Tokyo Night's selection/visual color `Rgb(40, 52, 84)` or the blue-tinted `Rgb(52, 73, 116)` would give enough contrast. Also consider bolding the selected row title for extra pop.
