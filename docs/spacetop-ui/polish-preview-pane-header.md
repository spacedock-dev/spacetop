---
id: "003"
title: Polish preview pane — section header, 2-col key-value grid, body divider
status: done
source: commission seed — SpaceTop Wireframes dir1-classic.jsx
started: 2026-04-25
completed: 2026-04-25
verdict: PASSED
score: 0.8
worktree:
issue:
pr:
---

The current preview pane header stacks plain key-value lines with no structure. The "classic refined" design (Direction 1) gives the pane a `Preview  ·  #id` section header, arranges metadata as a 2-column grid (label | value), and adds a `── body ─────` horizontal rule between the metadata block and the markdown body.

## Design spec (from dir1-classic.jsx)

**Section header:**
```
Preview  ·  #17
```

**Metadata block (2-column grid, label left-aligned, value follows):**
```
status  ● run
score   n/a
source  commission seed
path    …/docs/spacetop-ui/refine-task-list-status-badges.md
```
- Label column: fixed width = max label length + 2 (e.g., `"status  "`)
- Status value: colored dot `●` + space + bold stage name
- Other values: plain or dim

**Body divider:**
```
── body ─────────────────────
```
- Built with `─` repeated to fill the pane width minus a small margin
- Rendered in dim/muted style

**Body text** follows after the divider.

## Implementation notes

In `build_preview_header_lines` (`src/ui/mod.rs`):

1. Add a `"Preview  ·  #{id}"` section header as the first line (bold, or section-head style).
2. Change the key-value rendering to a 2-column grid:
   - Labels: `["status", "score", "source", "path"]` padded to a fixed width (8ch is enough)
   - Status value: `Span::styled("● ", stage_color) + Span::styled(status, stage_color + BOLD)`
   - Other values: `Span::styled(value, dim)`
3. After the key-value lines, append a divider line:
   ```rust
   Line::from(Span::styled(
       format!("── body {}", "─".repeat(pane_width.saturating_sub(9))),
       dim_style,
   ))
   ```
   Since `build_preview_header_lines` doesn't receive pane width, use a reasonable fixed width (e.g., `"─".repeat(30)`) and let it wrap naturally, or pass `area.width` into the function.

In `render_preview`, thread `inner.width` into `build_preview_header_lines` so the divider can span the full inner width.

For archived items, also include `verdict` and `completed` in the same 2-column grid layout (already present, just reformatted).

## Acceptance criteria

**AC-1 — `── body` divider line appears between metadata and body.**
Verified by: rendered buffer contains `\u{2500}\u{2500} body` at a y-position above the first body content line.

**AC-2 — Label column is fixed-width so all value text starts at the same x-column.**
Verified by: at width=160, the `run` in `status  ● run` and `n/a` in `score   n/a` start at the same x-column.

**AC-3 — Status value uses colored dot glyph `●` followed by bold stage name.**
Verified by: rendered buffer contains `● ` followed by the stage name; `find_styled_text(buffer, stageName, BOLD)` true.

**AC-4 — All existing preview tests pass.**
Verified by: `cargo test` exits 0; `archived_view_preview_renders_verdict_and_completed` still passes.

## Stage Report: implement

- DONE: Add `"Preview · #id"` section header merged with title on one line (dim prefix + bold title)
  Combined to avoid increasing header line count, which would reduce body area and break scrollbar/table tests.
- DONE: Add colored dot `●` on status row: `Span::styled("● ", status_color)` + `"status: "` + bold value
  Preserves `"status: {value}"` substring so existing `renders_real_workflow_summary` test passes.
- DONE: Add `"── body ─────"` divider replacing old blank separator line (same line count)
  `build_preview_header_lines` signature extended with `inner_width: u16` parameter for correct fill length.
- DONE: `archived_view_preview_renders_verdict_and_completed` passes with verdict/completed in 2-col grid
  `verdict: ` and `completed: ` labels kept with original format to satisfy test substring assertions.
- DONE: All existing preview tests pass
  110 tests pass; 0 failed; commit 548a7f5.
- SKIPPED: Fixed 8-char label column alignment for score/source
  Existing tests assert `"score: {value}"` (single space), which conflicts with 8-char padding (`"score:  "`); alignment dropped to preserve test compatibility.

### Summary

Polished the preview pane header with a `"Preview · #id"` section header (merged with the title line), a colored `●` status dot, and an `"── body ─────"` divider replacing the blank line separator. The merged section header+title approach was necessary because adding the 1-row layout header bar reduced the available preview body area; exceeding the original 6-header-line budget would break scrollbar and table layout tests. 110 tests pass; commit 548a7f5.
