---
id: "061"
title: Preview header gives source and worktree separate lines
status: verify
source: captain request 2026-06-12
kind: bugfix
risk: low
milestone: v1-maintenance
proof: Ratatui preview rendering regression plus make lint
started: 2026-06-12T12:53:54Z
completed:
verdict:
score: 0.66
worktree: .worktrees/spacedock-ensign-061-preview-header-source-worktree-lines
issue:
pr: "#62"
mod-block: merge:pr-merge
---

The preview page header currently renders fields such as `source: ...` and
`worktree: ...` in a compact header row. These values can be long, especially
when a task source names a detailed user request or a worktree path includes a
long branch/entity slug. The preview header should put `source:` and `worktree:`
on their own lines so long values do not crowd the rest of the header.

## Scope

- Kind: bugfix
- Risk: low
- Milestone: v1-maintenance
- Touches: UI
- Non-goals: changing entity parsing, changing task metadata semantics, changing
  workflow markdown, or adding any write behavior.

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- Preview source renders on its own header line.**
When the selected task has a long `source` value, the preview page header renders
`source: ...` on a dedicated line instead of sharing a compact metadata row with
other fields.
Verified by:

**AC-2 -- Preview worktree renders on its own header line.**
When the selected task has a `worktree` value, including the empty/default `-`
display, the preview page header renders `worktree: ...` on a dedicated line
instead of sharing a compact metadata row with `source` or other fields.
Verified by:

**AC-3 -- Long header metadata does not overlap or hide primary preview content.**
Long `source` or `worktree` text is clipped, wrapped, or otherwise constrained
according to the existing preview layout rules, and it does not overlap the
title, controls, body text, border, or neighboring UI.
Verified by:

**AC-4 -- Non-preview screens keep their current metadata layout.**
The change is scoped to the preview page header and does not unintentionally
alter task list rows, footer text, workflow tabs, picker, help popup, or
Workflow Definition rendering.
Verified by:

## Proof plan

- Lowest test layer: Ratatui `TestBackend` rendering regression for the preview
  page with long `source` and `worktree` metadata.
- Required command: `make lint`
- Manual check, if any: run `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev`,
  open a task preview with long source/worktree values, and confirm the header
  remains readable.
- Docs/policy update needed: update help/footer text only if user-facing
  documented behavior changes.

## Implementation plan

### Owned modules and scope boundary

- Modify `crates/spacetop/src/ui/preview.rs`, specifically
  `build_preview_header_lines`. Keep the change inside preview-header line
  construction.
- Do not change parser/domain models, app selection state, markdown body
  rendering, preview split placement, footer/help text, workflow tabs, picker,
  task list rows, graph rendering, Workflow Definition, or any workflow
  markdown write path.
- Preserve existing source/worktree metadata semantics: `source` still falls
  back to `n/a`, `worktree` still displays the basename when set, and empty
  worktree still displays the current empty marker.

### Steps

1. Write the failing Ratatui regression tests first.

   - In `crates/spacetop/src/ui/tests/overview.rs`, replace the stale
     `bottom_preview_compacts_metadata_into_one_line` expectation with
     `bottom_preview_renders_source_and_worktree_on_dedicated_lines`.
   - Build an item with a long `source` string and a long
     `.worktrees/spacedock-ensign-061-preview-header-source-worktree-lines`
     value, render with `TestBackend::new(80, 180)` so the preview uses bottom
     placement, and assert:
     - `status:`, `source:`, and `worktree:` all render.
     - `source:` and `worktree:` have different y coordinates.
     - neither `source:` nor `worktree:` shares the status/score row.
     - the body divider and first body text render below the metadata rows.
   - Update `preview_renders_em_dash_for_empty_worktree` or add a companion
     bottom-placement test so the empty/default worktree line is also proven to
     be dedicated, not part of the source row.
   - Add a long-value overlap regression in `crates/spacetop/src/ui/tests/preview.rs`
     for a narrow-enough preview pane that forces wrapping. Use the existing
     `find_text` helper to compare row coordinates for `source:`,
     `worktree:`, `path:`, `-- body`, and stable body text.

2. Run the new tests and confirm they fail on the current compact bottom-header
   implementation.

   - `cargo test -p spacetop bottom_preview_renders_source_and_worktree_on_dedicated_lines`
   - `cargo test -p spacetop preview_header_long_source_and_worktree_do_not_overlap_body`
   - Expected failure: source/worktree are found on the same row, or body/divider
     placement does not satisfy the new dedicated-line assertions.

3. Implement the smallest preview-header change.

   - In `build_preview_header_lines`, keep the title row, status/score row,
     `path:` row, body divider, archived verdict/completed behavior, and
     existing `worktree_segment` basename logic.
   - For active bottom placement, render one line for status plus score, then
     a dedicated `source: ...` line, then a dedicated `worktree: ...` line.
   - For archived bottom placement, use the same dedicated `source:` and
     `worktree:` lines while preserving `verdict:` and `completed:` rendering.
   - Leave left placement functionally unchanged except for any small local
     helper extraction needed to remove duplicated source/worktree line
     construction.
   - Do not change `render_preview`, `wrapped_lines_height`, `fit_path_to_width`,
     or markdown/diff rendering unless a failing test proves those mechanics are
     directly responsible for overlap.

4. Run targeted regression coverage.

   - `cargo test -p spacetop bottom_preview_renders_source_and_worktree_on_dedicated_lines`
   - `cargo test -p spacetop preview_header_long_source_and_worktree_do_not_overlap_body`
   - `cargo test -p spacetop preview_renders_em_dash_for_empty_worktree`
   - `cargo test -p spacetop archived_preview_includes_worktree_segment`

5. Run broader verification before completion.

   - `cargo fmt`
   - `cargo test -p spacetop`
   - `make lint`
   - Optional manual check: `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev`,
     open the preview for an entity with long metadata, and confirm the preview
     header is readable in both bottom and right-side placements.

### Docs impact decision

No README, help popup, footer, policy, parser, or workflow-schema docs update is
expected because this is a preview layout bugfix and does not change keybindings,
commands, metadata names, or workflow semantics. If implementation changes any
stable user-facing label text beyond row placement, update the pinned UI tests
and nearby docs in the same change.

## Stage Report: plan

- DONE: Plan names the exact preview rendering components/modules and the scope boundary that prevents unintended non-preview layout changes.
  Plan limits implementation to `crates/spacetop/src/ui/preview.rs::build_preview_header_lines` plus Ratatui UI tests, and excludes parser, app state, footer/help, tabs, picker, task list, graph, definition, and workflow writes.
- DONE: Plan specifies Ratatui `TestBackend` regression coverage for dedicated `source` and `worktree` header lines with long values and no overlap.
  Plan adds bottom-placement and long-value preview regressions using row-coordinate assertions for `source:`, `worktree:`, divider, and body text.
- DONE: Plan includes the proof path through `make lint` and a clear docs-impact decision.
  Proof path lists targeted `cargo test -p spacetop ...`, `cargo fmt`, full `cargo test -p spacetop`, `make lint`, and states no docs update is expected unless stable labels change.

### Summary

Created a concrete implementation plan for separating preview `source:` and
`worktree:` metadata into dedicated header lines. The plan keeps the change
inside preview rendering, requires Ratatui regression tests before code edits,
and preserves the docs/read-only boundaries for the later implementation stage.
