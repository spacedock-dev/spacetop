---
id: "014"
title: Fix preview pane scrollbar — unbounded offset drift and thumb not reaching bottom
status: plan
source: bug report — captain feedback 2026-04-25
started: 2026-04-25T10:37:39Z
completed:
verdict:
score: 0.9
worktree:
issue:
pr:
---

Two related scrollbar bugs in the preview pane (`src/ui/mod.rs` + `src/app.rs`):

**Bug A — Scroll offset drifts past the end.**
`scroll_preview_down()` in `src/app.rs` always adds 6 with no cap:
```rust
fn scroll_preview_down(&mut self) {
    self.preview_scroll = self.preview_scroll.saturating_add(6);
}
```
`preview_scroll` accumulates indefinitely even after the content end is reached. The render layer clamps visually (`scroll_position = state.preview_scroll().min(max_scroll)`) so the display looks frozen, but the internal counter keeps growing. Consequence: after pressing PageDown 30 times at the bottom, pressing PageUp 30 times does nothing — the user must press it many more times to unwind the phantom drift before scrolling actually starts moving.

**Bug B — Scrollbar thumb does not touch the bottom of the track at max scroll.**
The scrollbar state is constructed as:
```rust
ScrollbarState::new(content_height as usize)
    .viewport_content_length(body_inner.height as usize)
    .position(scroll_position)
```
Ratatui computes thumb position as integer: `position * (track_height - thumb_height) / (content_length - viewport_length)`. With typical content heights (40–80 lines) and viewport heights (15–25 rows), integer truncation leaves the thumb 1–2 rows above the bottom even when `scroll_position == max_scroll`. The scrollbar track also spans the full `body_inner` area, which includes the header lines — widening the perceived mismatch.

## Root cause detail

`max_scroll` is computed only inside `render_preview` and is never fed back to the app state. `scroll_preview_down` therefore cannot cap against it. This is the core architecture gap.

For Bug B, the correct `ScrollbarState` parameterisation for ratatui's vertical scrollbar is:
- `content_length` = `max_scroll` (the maximum position value, i.e. total content rows minus viewport rows)
- `viewport_content_length` = 1 (treat position as a direct offset, not a proportional index — this makes the thumb height ratatui-managed at a sensible minimum and positions it correctly at the extremes)
- `position` = `scroll_position` (clamped)

Alternatively, keep proportional sizing but use `content_length = max_scroll + 1` and `viewport_content_length = 1` to ensure the thumb reaches position 0 and max_scroll exactly.

## Fix approach

### Bug A — cap scroll offset at app layer

Add a `max_preview_scroll` field to `OverviewState` (default `usize::MAX`). Update it from `render_preview` by using an interior-mutable cell (`Cell<usize>`) so the render can write back without requiring `&mut`:

```rust
// In OverviewState:
max_preview_scroll: std::cell::Cell<usize>,
```

In `render_preview`, after computing `max_scroll`:
```rust
state.max_preview_scroll.set(max_scroll);
```

In `scroll_preview_down`:
```rust
fn scroll_preview_down(&mut self) {
    let max = self.max_preview_scroll.get();
    self.preview_scroll = self.preview_scroll.saturating_add(6).min(max);
}
```

Alternative (simpler, no Cell): store `last_known_content_height: usize` on state. Compute an approximate `max_scroll` in `scroll_preview_down` using the stored height and a heuristic viewport height. This avoids cross-layer coupling but is less accurate.

### Bug B — fix ScrollbarState parameterisation

Replace:
```rust
ScrollbarState::new(content_height as usize)
    .viewport_content_length(body_inner.height as usize)
    .position(scroll_position)
```
With:
```rust
ScrollbarState::new(max_scroll + 1)
    .position(scroll_position)
```
`ScrollbarState::new(n)` sets content_length to `n`. With `viewport_content_length` omitted (defaults to 0), ratatui renders a fixed-height thumb positioned linearly across the track: at `position = 0` the thumb sits at the top; at `position = max_scroll` it sits at the very bottom. This matches the user expectation exactly.

## Acceptance criteria

**AC-1 — Pressing PageDown at the bottom of a long item does not accumulate phantom offset.**
Verified by: unit test — load an item with 60 lines, press PageDown 30 times, assert `preview_scroll() <= max_scroll`. Then press PageUp once and assert the position decreases by 6 (not still at max due to phantom drift).

**AC-2 — The scrollbar thumb visually touches the bottom of the track when scrolled to the end.**
Verified by: render test at width=160, height=30 with a 60-line body — after scrolling to max, assert the scrollbar thumb glyph (`█`) appears at row `body_inner.y + body_inner.height - 1` (the last row of the body area).

**AC-3 — The scrollbar thumb starts at the top of the track when scroll is 0.**
Verified by: same fixture, scroll=0, assert `█` appears at row `body_inner.y`.

**AC-4 — Existing scrollbar and page-scroll tests pass.**
Verified by: `cargo test` exits 0; `preview_draws_scrollbar_when_content_overflows` and `preview_page_down_scrolls_visible_markdown_content` pass.
