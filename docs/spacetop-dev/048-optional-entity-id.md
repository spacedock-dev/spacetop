---
id: 048
title: Support optional entity ID (id-style: slug)
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

The newer Spacedock README format declares `id-style: slug` in the workflow frontmatter, which means entity files carry a blank `id:` field — identity comes from the filename slug instead of a numeric or sd-b32 value. SpaceTop currently expects a populated `id` field on every entity; it should handle the slug-identity case gracefully and display the slug as the effective ID.

Reference workflow: `/Users/kent/Dev/InfuseAI/paper-work/roadmap-playground/docs/roadmap-workflow`  
Example entities: `roadmap-v5.md`, `adversarial-review.md` (both have `id:` blank, `id-style: slug` declared in README).

## Acceptance criteria

**AC-1 — No crash on blank `id:` field.**  
Verified by: `cargo test` passes against a fixture workflow that uses `id-style: slug` with entities whose `id:` is empty or absent.

**AC-2 — Slug used as display ID when `id:` is blank.**  
Verified by: the status table (overview UI) shows the entity slug in the ID column when `id:` is blank, confirmed by an integration test or a snapshot of terminal output.

**AC-3 — `id-style` read from README frontmatter.**  
Verified by: `parser.rs` (or its domain model) exposes `id_style` from the workflow README, and the test fixture exercises both `sequential` and `slug` values.

**AC-4 — Existing sequential-id workflows unaffected.**  
Verified by: all existing tests still pass; no regression in the spacetop-dev workflow's own numeric IDs.
