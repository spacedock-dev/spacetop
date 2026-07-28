---
title: Keep long slug IDs compact and copyable
status: plan
source: "UI feedback from task-list screenshot on 2026-07-28"
kind: feature
risk: medium
milestone: v1-maintenance
proof: Ratatui task-list width tests plus mouse double-click copy tests
started: 2026-07-28T03:26:42Z
completed:
verdict:
score: 0.78
worktree:
issue:
pr:
id: 074
---

Long slug-style IDs currently expand the task-list ID column without an upper bound, leaving too little room for the title. Keep the visible slug ID within a bounded, responsive width so task titles retain useful space.

The full slug must remain easy to retrieve: double-clicking the ID cell should copy the complete, untruncated slug even when the visible value is ellipsized. This must work with Spacetop mouse capture enabled.

## Acceptance criteria

- **AC-1:** In an `id-style: slug` workflow, long IDs have a bounded visible width and cannot compress the title below the intended minimum title space.
- **AC-2:** Long visible IDs are ellipsized while short slug IDs and numeric or sequential IDs remain readable without unnecessary truncation.
- **AC-3:** Double-clicking inside an entity ID cell copies the exact complete slug, not the displayed truncated value.
- **AC-4:** Double-clicking outside the ID cell does not copy an ID, and existing single-click row selection and mouse scrolling continue to work.
- **AC-5:** The TUI provides brief, non-disruptive confirmation after copying an ID.
- **AC-6:** Ratatui rendering tests cover long and short IDs at narrow and wide terminal sizes; app/input tests cover double-click detection, hit testing, and the exact copied value.

## Implementation context

`crates/spacetop/src/ui/list.rs` currently sizes the ID column from the longest visible ID with no upper clamp. `crates/spacetop/src/lib.rs` enables Crossterm mouse capture, so the copy interaction must preserve existing mouse navigation.
