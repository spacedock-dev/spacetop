---
id: "023"
title: "spacetop shows stale worktree status instead of main-branch status for active entities"
status: plan
source: bug report
started: 2026-04-26T02:59:52Z
completed:
verdict:
score: 0.85
worktree:
issue:
pr:
---

When an entity has an active worktree, spacetop reads the entity file from the worktree path and displays whatever `status` field is in that copy. But the first officer owns and advances `status` on the main branch — the worktree copy's `status` field is whatever value was present when the worktree branch was created, and the implement worker never updates it. This causes spacetop to display a stale (earlier) stage name while the entity is actually further along.

## Reproduction

Task 021 (`021-scan-discovery-io-error`) demonstrates this:

- Main branch entity: `status: review`
- Worktree entity (`docs/spacetop-dev/021-scan-discovery-io-error.md` on branch `spacedock-ensign/021-scan-discovery-io-error`): `status: plan`
- spacetop displays: `plan` (wrong)

The worktree was branched from main when main had `status: plan`. The FO subsequently advanced main to `implement` then `review`, but the worktree copy was never updated to reflect those transitions.

## Root cause

spacetop's workflow scanner resolves the entity file path through the worktree when the `worktree` frontmatter field is set, and reads the entire frontmatter — including `status` — from that worktree copy. Since the FO is the sole writer of `status` transitions on main and ensigns do not mirror those writes into the worktree, the worktree copy's `status` is always one or more stages behind.

## Fix direction

For FO-owned frontmatter fields (`status`, `worktree`, `pr`, `mod-block`, `completed`, `verdict`), spacetop should read from the **main-branch copy** of the entity file, not the worktree copy. Body content and stage reports live in the worktree and should still be read from there. A merged-view approach: read frontmatter from main, body from worktree (or main when no worktree is active).

## Acceptance criteria

**AC-1 -- Status reflects main-branch value when a worktree is active.**
For an entity with an active worktree whose main-branch `status` differs from the worktree copy, spacetop displays the main-branch `status`.
Verified by: unit test using a fixture with mismatched main vs worktree frontmatter; assert displayed status matches main.

**AC-2 -- Body/stage-report content is still read from the worktree.**
The preview pane shows the worktree copy's body (which may contain the latest stage report not yet merged to main), not the main-branch body.
Verified by: unit test asserting body content comes from the worktree copy when both differ.

**AC-3 -- Entities without an active worktree are unaffected.**
When `worktree` is empty, spacetop reads both frontmatter and body from main as before.
Verified by: existing tests continue to pass; add an explicit test for the no-worktree path.

## Stage Report: design

- DONE: Problem statement names the exact code path in spacetop that reads frontmatter from the worktree copy instead of main.
  `src/parser.rs` `merge_worktree_items` (line 307): when a worktree item's SHA-1 differs from the main-branch item (line 335-337), the worktree `WorkItem` fully replaces the main-branch one, including its `status` field parsed from the worktree copy. Upstream: `scan_worktrees` calls `parse_work_item` on worktree paths (line 284), reading frontmatter including `status` from the frozen worktree copy.
- DONE: Acceptance criteria cover the merged-view requirement: frontmatter from main, body from worktree, no-worktree unchanged.
  AC-1, AC-2, and AC-3 in the entity body cover all three cases: main-branch frontmatter wins for status (AC-1), worktree body wins for body content (AC-2), and no-worktree path is unchanged (AC-3).

### Summary

The exact bug site is `merge_worktree_items` in `src/parser.rs`: when hashes differ and the worktree item wins, it replaces the entire `WorkItem` including `status`, discarding the main-branch frontmatter. The fix must produce a merged `WorkItem` that keeps FO-owned frontmatter fields (`status`, `worktree`, `pr`, `completed`, `verdict`, `score`) from the main-branch item while taking `body` from the worktree item. The acceptance criteria in the entity file already name all three paths required for correctness: status from main (AC-1), body from worktree (AC-2), no-worktree unchanged (AC-3).
