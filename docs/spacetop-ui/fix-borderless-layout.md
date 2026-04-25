---
id: "004"
title: Fix borderless pane layout to match classic refined design
status: implement
source: captain feedback 2026-04-25
started: 2026-04-25T10:05:53Z
completed:
verdict:
score: 0.95
worktree: .worktrees/spacedock-ensign-fix-borderless-layout
issue:
pr:
---

The current dashboard wraps both the task list and preview pane in `Block::default().borders(Borders::ALL)` which draws box-drawing borders that the classic refined design does not have. The design (dir1-classic.jsx) uses borderless flex panels separated only by a single vertical rule `│` between the two panes. Additionally the pane split is 42%/58% but the design shows 50%/50%. The `── body ───` divider in the preview header is clipped because the box border shrinks the inner area.

## Design spec (dir1-classic.jsx)

```
┌ header bar (1 row) ──────────────────────────────────────┐
│ Workflow [active]  (press a)  /path  ─────────────────── │
├ phase rail (with top+bottom rule only) ──────────────────┤
│  ▶ pending ──► ⚑ ⎇ hypothesis ──► ⎇ smoke ──►  ...     │
│              0          1            1                    │
├─ body area (no borders) ─────────────────────────────────┤
│ Tasks  ·  3              │  Preview  ·  #001  Title       │
│   001  DES  Task one     │  ● status: design              │
│   002  IMP  Task two     │    score:  n/a                 │
│                          │    source: …                   │
│                          │    path:   …                   │
│                          │  ── body ────────────────────  │
│                          │  Body content here             │
├ footer ──────────────────────────────────────────────────┤
│             ?: help   a: archive   q: quit                │
└──────────────────────────────────────────────────────────┘
```

Key differences from current:
- **No `Borders::ALL`** on task list or preview blocks — use `Block::default()` (no borders)
- **50/50 split** between task list and preview (`Constraint::Percentage(50)` each)
- **Single `│` separator** between the two panes: render preview pane with `Borders::LEFT` only (one column wide)
- **Graph/phase rail**: keep current graph but change block to `Borders::TOP | Borders::BOTTOM` only (no left/right border) so the content spans edge to edge
- **Section headers**: "Tasks  ·  N" and "Preview  ·  #id  title" become the visual titles instead of block title strings — already present in code, just need the border removed so they have room

## Implementation notes

In `render_overview` (`src/ui/mod.rs`):
- Change `Constraint::Percentage(42)` / `Constraint::Percentage(58)` → both `Constraint::Percentage(50)`

In `render_task_list`:
- Change `Block::default().title(title).borders(Borders::ALL)` → `Block::default()` (no borders, no title — section header line already handles it)
- The `inner` area now equals the full `area` rect — no inset from borders

In `render_preview`:
- Change `Block::default().title("Preview").borders(Borders::ALL)` → `Block::default().borders(Borders::LEFT)` — keeps the single `│` separator
- The inner area loses only 1 column on the left (the `│`)

In `render_stage_graph` (`src/ui/graph.rs`):
- Change the graph block borders from `Borders::ALL` → `Borders::TOP | Borders::BOTTOM`

## Test compatibility notes

- Tests asserting `"Tasks"` text still pass (section header renders it)
- Tests asserting `"Preview"` text in the preview area still pass (section header renders it) **but** tests asserting the "Preview" block title specifically may need updating
- Tests for `dashboard_pane_spans_full_terminal_width` and `graph_ribbon_node_row_is_horizontally_centered_in_pane` may need adjustment if the graph border change shifts rows
- Run `cargo test` after each change and fix any broken assertions

## Acceptance criteria

**AC-1 — No `Borders::ALL` on task list or preview panes in the rendered output.**
Verified by: `cargo test` passes; visual inspection shows no `┌`, `┐`, `└`, `┘` corner glyphs in the task/preview area.

**AC-2 — The `── body ───` divider is visible in the preview pane.**
Verified by: render test at width=160, height=30 — buffer contains `\u{2500}\u{2500} body` in the preview column range.

**AC-3 — A single `│` column separates the task list from the preview pane.**
Verified by: buffer at the midpoint column (width/2) has `│` glyph for several rows in the content area.

**AC-4 — All existing `cargo test` assertions pass (adjust any that check box-border glyphs if necessary).**
Verified by: `cargo test` exits 0.
