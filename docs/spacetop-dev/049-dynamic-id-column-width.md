---
id: "049"
title: "Dynamic ID column width in the task list (slug-ID overflow fix)"
status: plan
source: captain
started: 2026-06-09T02:55:50Z
completed:
verdict:
score:
worktree:
issue:
pr:
---

The task list ID column is hardcoded to `{:>4}` chars in `src/ui/list.rs`, which fit numeric IDs (`047`, `048`) but overflows for slug IDs (`adversarial-review`, `roadmap-v5`), causing the Title column to misalign. The fix is a dynamic column width: measure the longest ID in the visible item list on each render and size the column to fit, so Title always aligns regardless of ID style.

Flagged as a follow-up in task 048's plan and implement stage reports.

## Acceptance criteria

**AC-1 — Title column aligns when slug IDs are present.**  
Verified by: `src/ui/list.rs` computes `id_col_width = items.iter().map(|i| i.id.len()).max().unwrap_or(4).max(4)` (or equivalent) and uses it for both the ID and Title column offsets. Confirmed by a unit test or snapshot asserting the rendered row for `adversarial-review` has the title starting at the correct offset.

**AC-2 — Short numeric IDs still render at a minimum 4-char width.**  
Verified by: the dynamic width has a floor of `max(4, longest_id_len)` so `047`-style workflows are visually unchanged.

**AC-3 — No regression on existing task list tests.**  
Verified by: `cargo test` passes; `make lint` clean.
