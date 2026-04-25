---
id: "014"
title: Fix preview pane scrollbar — unbounded offset drift and thumb not reaching bottom
status: review
source: bug report — captain feedback 2026-04-25
started: 2026-04-25T10:37:39Z
completed:
verdict:
score: 0.9
worktree: .worktrees/spacedock-ensign-fix-preview-scrollbar-bugs
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

## Implementation Plan

### Step 1 — Add `max_preview_scroll` to `OverviewState` (`src/app.rs`)

File: `src/app.rs`

1a. Add import at top of file (it already uses `std::path`, keep both):
```rust
use std::cell::Cell;
```

1b. Add field to `OverviewState` struct:
```rust
pub max_preview_scroll: Cell<usize>,
```
The field is `pub` so `render_preview` in `src/ui/mod.rs` can call `.set()` on it with an immutable `&OverviewState` reference.

1c. Initialize the field in every constructor / reset site:
- `OverviewState::empty` → `max_preview_scroll: Cell::new(usize::MAX)`
- `OverviewState::from_snapshot` → same
- `OverviewState::reload_from_snapshot` → `self.max_preview_scroll.set(usize::MAX);`
- `OverviewState::toggle_scope` → `self.max_preview_scroll.set(usize::MAX);`
- `OverviewState::set_scope_index` (preview reset branch) → `self.max_preview_scroll.set(usize::MAX);`

Initializing to `usize::MAX` means that until the first render writes the real cap, `scroll_preview_down` can advance freely — this is the same behaviour as today, so no existing tests break.

1d. Update `scroll_preview_down`:
```rust
fn scroll_preview_down(&mut self) {
    let max = self.max_preview_scroll.get();
    self.preview_scroll = self.preview_scroll.saturating_add(6).min(max);
}
```

Note: `OverviewState` derives `Clone` and `PartialEq`. `Cell<usize>` implements both, so no derive changes are needed.

### Step 2 — Write `max_scroll` back from `render_preview` (`src/ui/mod.rs`)

File: `src/ui/mod.rs`

After the existing line:
```rust
let max_scroll = usize::from(content_height.saturating_sub(body_area.height));
```

Add:
```rust
state.max_preview_scroll.set(max_scroll);
```

This is safe because `render_preview` takes `state: &OverviewState` (immutable reference) and `Cell::set` requires only `&Cell<T>`.

### Step 3 — Fix `ScrollbarState` parameterisation (`src/ui/mod.rs`)

File: `src/ui/mod.rs`, inside `render_preview`, in the `if show_scrollbar { … }` block.

Replace:
```rust
let mut scrollbar_state = ScrollbarState::new(content_height as usize)
    .viewport_content_length(body_inner.height as usize)
    .position(scroll_position);
```

With:
```rust
let mut scrollbar_state = ScrollbarState::new(max_scroll + 1)
    .position(scroll_position);
```

`ScrollbarState::new(n)` sets `content_length = n`. Omitting `viewport_content_length` leaves it at 0, which causes ratatui 0.29 to render the thumb positioned linearly: at `position = 0` it is at the track top; at `position = max_scroll` it is at the track bottom.

### Step 4 — Add unit tests (`src/app.rs`)

Add to the `#[cfg(test)]` block in `src/app.rs`:

**Test AC-1a: offset cap — no phantom drift:**
```rust
#[test]
fn scroll_preview_down_is_capped_at_max_scroll() {
    use std::cell::Cell;
    let mut state = OverviewState::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_items(1),
    );
    // Simulate render having set max_scroll = 10.
    state.max_preview_scroll.set(10);

    // Press PageDown 20 times — should not exceed 10.
    for _ in 0..20 {
        state.scroll_preview_down();
    }
    assert!(
        state.preview_scroll() <= 10,
        "preview_scroll must not exceed max_scroll"
    );
}
```

**Test AC-1b: phantom drift unwinds correctly:**
```rust
#[test]
fn scroll_preview_up_responds_immediately_after_capped_down() {
    let mut state = OverviewState::from_snapshot(
        PathBuf::from("workflow"),
        snapshot_with_items(1),
    );
    state.max_preview_scroll.set(10);

    // Press down many times (capped at 10).
    for _ in 0..30 {
        state.scroll_preview_down();
    }
    assert_eq!(state.preview_scroll(), 10);

    // One PageUp should immediately decrease position.
    state.scroll_preview_up();
    assert!(
        state.preview_scroll() < 10,
        "first PageUp must decrease scroll after capped drift"
    );
}
```

### Step 5 — Add render tests (`src/ui/mod.rs`)

Add to the `#[cfg(test)]` block in `src/ui/mod.rs`.

Fixture helper (add once):
```rust
fn scrollable_app_at_max() -> (App, u16) {
    // 60 content lines, each separated by a blank → ~120 rendered rows.
    let body = (0..60)
        .map(|i| format!("Line {:02}", i))
        .collect::<Vec<_>>()
        .join("\n\n");
    let app = app_with_items(vec![item("001", "Scrollable", &body)]);
    // Scroll until capped. We simulate a render first to set max_scroll.
    // For the test we just apply many PageDowns and let the cap do its job.
    (app, 30) // height used for the test terminal
}
```

**Test AC-2: thumb at bottom when scroll == max:**
```rust
#[test]
fn preview_scrollbar_thumb_reaches_bottom_at_max_scroll() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let body = (0..60)
        .map(|i| format!("Line {:02}", i))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut app = app_with_items(vec![item("001", "Scrollable", &body)]);
    let width: u16 = 160;
    let height: u16 = 30;

    // Run several render+scroll cycles so max_preview_scroll is set by
    // render_preview before scroll_preview_down reads it.
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    for _ in 0..30 {
        terminal.draw(|frame| render(frame, &app)).unwrap();
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    }
    // Final render at max scroll.
    terminal.draw(|frame| render(frame, &app)).unwrap();

    let buffer = terminal.backend().buffer();
    let right_edge = width - 1;
    let bottom_row = height - 2; // last row before footer
    let thumb_at_bottom = buffer[(right_edge, bottom_row)].symbol() == "\u{2588}";
    assert!(
        thumb_at_bottom,
        "scrollbar thumb must reach bottom row at max scroll (col={right_edge}, row={bottom_row})"
    );
}
```

**Test AC-3: thumb at top when scroll == 0:**
```rust
#[test]
fn preview_scrollbar_thumb_starts_at_top_at_zero_scroll() {
    let body = (0..60)
        .map(|i| format!("Line {:02}", i))
        .collect::<Vec<_>>()
        .join("\n\n");
    let app = app_with_items(vec![item("001", "Scrollable", &body)]);
    let width: u16 = 160;
    let height: u16 = 30;

    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|frame| render(frame, &app)).unwrap();

    let buffer = terminal.backend().buffer();
    let right_edge = width - 1;

    // Determine where body content starts: after header rows. The preview
    // pane occupies cols width/2..width. The header is 7 lines (preview
    // title + 3 metadata + divider = depends on item). Find first thumb row.
    let first_thumb_row = (1..height - 1)
        .find(|&y| buffer[(right_edge, y)].symbol() == "\u{2588}")
        .expect("scrollbar thumb must be visible at scroll=0");

    // The body area starts after the graph ribbon (7 rows) + header row (1)
    // + preview header lines (~7). In practice, look for it being in the
    // upper half of the terminal.
    assert!(
        first_thumb_row < height / 2,
        "at scroll=0, thumb must sit in the upper half of the track (got row {first_thumb_row})"
    );
}
```

### Step 6 — Verify

Run: `cargo test 2>&1 | tail -5`

All existing tests plus the four new tests must pass. Specifically confirm:
- `scroll_preview_down_is_capped_at_max_scroll` — PASS
- `scroll_preview_up_responds_immediately_after_capped_down` — PASS
- `preview_scrollbar_thumb_reaches_bottom_at_max_scroll` — PASS
- `preview_scrollbar_thumb_starts_at_top_at_zero_scroll` — PASS
- `preview_draws_scrollbar_when_content_overflows` — PASS
- `preview_page_down_scrolls_visible_markdown_content` — PASS

## Stage Report: plan

- DONE: Step-by-step plan for Bug A (offset cap via Cell<usize>) and Bug B (ScrollbarState parameterisation), naming exact functions and files.
  Plan covers Steps 1–3 with exact field names, constructor sites, and code replacements in `src/app.rs` and `src/ui/mod.rs`.
- DONE: Test strategy: proposed assertions for AC-1 (offset cap), AC-2 (thumb at bottom), AC-3 (thumb at top).
  Steps 4–5 specify exact test names, fixtures, terminal dimensions, assertion logic, and the render+scroll interleave pattern needed for Cell writeback.

### Summary

The plan addresses Bug A by adding `max_preview_scroll: Cell<usize>` to `OverviewState`, writing the clamped max from `render_preview` via `Cell::set`, and reading it in `scroll_preview_down` to cap the counter. Bug B is fixed by replacing the three-parameter `ScrollbarState` construction with `ScrollbarState::new(max_scroll + 1).position(scroll_position)`, which gives ratatui a linear thumb placement that lands exactly at the track extremes. The test strategy covers unit-level offset-cap assertions (no render needed) plus render-level thumb-position checks using a render+scroll interleave to ensure the Cell is populated before assertions run.
