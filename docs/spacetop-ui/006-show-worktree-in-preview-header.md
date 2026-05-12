---
id: "006"
title: Show worktree info in Preview header
status: design
source: captain
started:
completed:
verdict:
score:
worktree:
issue:
pr:
---

Surface the entity's `worktree` frontmatter field in the Preview pane header area so the captain can see at a glance whether a task currently has a live worktree (and where it is on disk) without opening the file.

Today the Preview pane renders `Preview · #id Title` on the first line and then a metadata block of `status:`, `score:`, `source:`, and `path:` (see `render_preview_header_lines` in `src/ui/mod.rs`, around lines 692–784). The `worktree` value is parsed by the entity loader but never shown. When the field is non-empty it is the strongest signal that an agent is (or recently was) dispatched on this entity — much more useful for the operator than `path:`, which is static.

The change should:

- Add a `worktree:` metadata line/segment alongside the existing `status`/`score`/`source` block, rendered for both `PreviewPlacement::Bottom` (single combined line, joined by `  ·  `) and `PreviewPlacement::Left` (its own line, matching the existing per-line style).
- When the field is empty, render it dimmed as `worktree: —` (em dash) or `worktree: none` so the absence is explicit rather than collapsing the row. Choose whichever reads cleanest with the rest of the header during design.
- When the field is non-empty, render the path value in the default (non-dim) style, matching how `source` is rendered today.
- Apply to both the active view and the archived view (archived entities also carry historical `worktree` values worth seeing).

## Acceptance criteria

**AC-1 — Worktree appears in Preview header for entities with a non-empty `worktree` field.**
Verified by: a `cargo test` rendering assertion that loads a fixture entity with `worktree: .worktrees/ensign-foo` set and asserts that the rendered preview buffer contains the substring `worktree: .worktrees/ensign-foo`.

**AC-2 — Worktree row renders an explicit empty marker when the field is unset.**
Verified by: a `cargo test` rendering assertion against a fixture entity with `worktree:` empty that asserts the rendered preview buffer contains `worktree: ` followed by the chosen empty marker (e.g. `—` or `none`) on a non-DIM-only line.

**AC-3 — Both Bottom and Left placements include the worktree segment.**
Verified by: `cargo test` cases that render the preview in each `PreviewPlacement` and assert the worktree text is present in the resulting buffer.

**AC-4 — No regression in existing header tests.**
Verified by: `make lint` passes and `cargo test` passes with the existing `src/ui/mod.rs` and `tests/` assertions unchanged or updated in lockstep with the new field.
