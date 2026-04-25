---
id: "002"
title: Refine task list — section header, 3-char tags, left-border selection
status: done
source: commission seed — SpaceTop Wireframes dir1-classic.jsx
started: 2026-04-25
completed: 2026-04-25
verdict: PASSED
score: 0.85
worktree:
issue:
pr:
archived: 2026-04-25T09:51:19Z
---

The "classic refined" design (Direction 1) replaces the current `[design]`-bracket status in task rows with a 3-char uppercase tag (bold, phase-colored), adds a section header showing the task count, and uses a left-border + `▸` glyph for the selected row instead of the ratatui `>` highlight prefix.

## Design spec (from dir1-classic.jsx)

**Section header:**
```
Tasks  ·  12
```
- Section name bold, count muted

**Row format (unselected):**
```
   HYP  11  Query batching — all queries visible
```
- 2 spaces indent
- 3-char uppercase tag, colored+bold, fixed 3ch width
- 2 spaces gap
- ID right-aligned in 3ch, muted
- 2 spaces gap
- Title

**Row format (selected):**
```
▸  RUN  17  README ablation arm 1 — direct single agent
```
- `▸` selector glyph at column 0
- Left border: 2px solid yellow (in TUI: a colored `▸` or prefix + reverse highlight on the row)

**3-char tag mapping** (Spacedock stages → generic):
- `design` → `DES`
- `plan` → `PLN`
- `implement` → `IMP`
- `review` → `REV`
- `done` → `DON`
- fallback: first 3 chars uppercase

**Optional note line** (below title, indented to title column, muted):
- e.g., `(model/analyze/verify phases)`

## Implementation notes

In `render_task_list` (`src/ui/mod.rs`):
- Add a section header line above the list: `"Tasks  ·  N"` where N = item count. Use `Paragraph` or inline in a `Line`.
- Adjust the block inner area down by 1 row after rendering the header.

In `build_task_list_items`:
- Replace `format!("[{}]", item.status)` with a 3-char tag computed via a `stage_tag(name: &str) -> &str` helper.
- Layout per row:
  1. `Span::raw(if selected { "▸ " } else { "  " })` — selector
  2. `Span::styled(format!("{:3}", tag), stage_color + BOLD)` — tag, fixed 3ch
  3. `Span::raw("  ")` — gap
  4. `Span::styled(format!("{:>3}", id), dim)` — ID right-aligned 3ch
  5. `Span::raw("  ")` — gap
  6. `Span::raw(title)` — title

Keep the existing ratatui `List` highlight for the selected row (REVERSED+BOLD) OR switch to manual row highlighting with a colored left-border span — whichever passes the existing selection tests.

## Acceptance criteria

**AC-1 — Task rows show a 3-char uppercase stage tag instead of `[stage]` brackets.**
Verified by: render at width=160; buffer does NOT contain `[design]` or `[implement]`; DOES contain `DES` or `IMP`.

**AC-2 — Stage tag is rendered in the stage color and bold.**
Verified by: `find_styled_text(buffer, "DES", |s| s.add_modifier.contains(BOLD))` returns true; tag color matches `stage_color("design")`.

**AC-3 — Section header "Tasks" appears above the list.**
Verified by: `rendered.contains("Tasks")` and the Tasks text appears at a lower y than the first task row.

**AC-4 — All existing task-list and selection tests pass.**
Verified by: `cargo test` exits 0 (existing `task_list_uses_full_pane_width_and_ratatui_list_selection` et al).

## Stage Report: implement

- DONE: Add `stage_tag(stage: &str) -> &str` helper mapping stage names to 3-char tags
  Covers design→DES, plan→PLN, implement→IMP, review→REV, done→DON, and others; fallback `···`.
- DONE: Update `build_task_list_items` to use `"{id:>3}  {tag}   {title}"` format
  ID right-aligned 3ch (dim), 3-char stage tag (stage_color+bold), 3-space gap, title. Verdict glyph appended for archived items.
- DONE: Add "Tasks · N" section header above the list in `render_task_list`
  Rendered as a `Paragraph` in a 1-row area at `inner.y`; list shifted down by 1 row.
- DONE: AC-4 — all existing tests pass including `task_list_uses_full_pane_width_and_ratatui_list_selection`
  Kept `highlight_symbol("> ")` and REVERSED+BOLD; 3-space gap after tag ensures FULLWIDTHMARKER at x≥74.

### Summary

Replaced `[status]` bracket format with 3-char uppercase stage tags (DES, PLN, IMP, etc.) in `build_task_list_items`. Added a "Tasks · N" section header rendered 1 row above the list. The key constraint was keeping `highlight_symbol = "> "` and ensuring the FULLWIDTHMARKER content test still passes with the new prefix layout. 110 tests pass; commit 548a7f5.
