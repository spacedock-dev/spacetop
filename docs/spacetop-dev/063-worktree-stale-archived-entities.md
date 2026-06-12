---
id: "063"
title: Worktree copies of archived entities reappear as active tasks
status: plan
source: captain bug report 2026-06-12
kind: bugfix
risk: medium
milestone: v1-maintenance
proof: parser/app-state regression plus make lint
started: 2026-06-12T13:43:03Z
completed:
verdict:
score: 0.9
worktree:
issue:
pr:
---

Spacetop merges task files from workflow copies under `.worktrees/*` so an
active task can show a worktree marker and preview the task body from the
worker's branch while keeping root frontmatter as the status source. This breaks
when a worktree contains stale copies of other tasks that have already been
completed and archived on the root workflow.

Example:

- Root workflow has active task `0x0c` with `worktree:
  .worktrees/0x0c-xxxx`.
- Root workflow has tasks `0x0a` and `0x0b` completed and moved under
  `_archive/`.
- The `0x0c-xxxx` worktree still contains old `0x0a.md` and `0x0b.md` files in
  its mirrored workflow directory because that worktree branch was created
  before the archive moves.

Today, Spacetop can scan those worktree-only files and treat `0x0a` and `0x0b`
as active tasks again. That makes the active task list and status model
incorrect. Root workflow state and root archive placement should be authoritative
for membership and status. Worktree files should be an overlay for active task
content, not a way to resurrect archived tasks.

## Scope

- Kind: bugfix
- Risk: medium
- Milestone: v1-maintenance
- Touches: parser / index-query / app-state / UI
- Non-goals: changing Spacedock archive semantics, deleting stale files from
  worktrees, adding workflow markdown write support, or removing the existing
  worktree marker/preview feature for the active task being processed.

## Acceptance criteria

Each AC names a property of the finished task, not a stage action.

**AC-1 -- Root active and archive state decide active-list membership.**
If an entity is archived in the root workflow, a stale markdown copy of that
same entity under any scanned worktree does not make it appear in the active task
list.
Verified by:

**AC-2 -- Root frontmatter remains authoritative for active worktree-backed tasks.**
For an entity that is still active in the root workflow and has a worktree copy,
Spacetop keeps status, title, source, worktree path, issue, PR, and other
frontmatter from the root entity while using the worktree body only for preview
content and diff behavior.
Verified by:

**AC-3 -- Worktree overlay is scoped to the correct active entity.**
When a worktree belongs to active task `0x0c`, stale copies of unrelated archived
tasks such as `0x0a` and `0x0b` are ignored for active scope, while the `0x0c`
preview can still use the worktree body and show the worktree icon/path.
Verified by:

**AC-4 -- Archived scope stays anchored to root archive files.**
The archived view shows `0x0a` and `0x0b` from the root `_archive/` directory,
not from stale worktree copies, and archived counts remain consistent after a
refresh.
Verified by:

**AC-5 -- Legitimate worktree-only behavior is explicitly decided and tested.**
If Spacetop still supports worktree-only tasks that do not exist in root active
or root archive state, that behavior is documented in code/tests and does not
apply to entities whose slug or ID exists in root `_archive/`.
Verified by:

**AC-6 -- Spacetop remains read-only toward workflow markdown.**
The fix changes how Spacetop interprets root and worktree files, but it does not
delete stale worktree files or mutate workflow markdown.
Verified by:

## Proof plan

- Lowest test layer: `spacetop-core` parser/index regression with a fixture
  containing root active `0x0c`, root archived `0x0a`/`0x0b`, and a `0x0c`
  worktree that still contains all three files.
- App/UI coverage: focused app or Ratatui test proving the active list contains
  only `0x0c`, the worktree marker/preview behavior remains for `0x0c`, and the
  archived scope still contains `0x0a`/`0x0b`.
- Required command: `make lint`
- Manual check, if any: create or reuse a local workflow/worktree with the
  example shape, open `spacetop --workflow-dir <workflow>`, and confirm active
  and archived scopes match root state.
- Docs/policy update needed: update README or development policy only if the
  supported meaning of worktree-only tasks changes.

## Closure Plan

Decision: close this task as covered by task `059-archive-move-refresh-active-list`
after `059` lands on `main`; do not implement a second production change for
`063` unless `059` is rejected or its branch changes before merge.

Evidence from comparing `059`:

- `059` commit `ff0cc6b` changes the parser-owned merge path, not UI code.
  `load_workflow_dir` collects root `_archive` slugs by path before worktree
  merge, and `merge_worktree_items` skips worktree-only rows whose slug exists
  in that root archive set.
- That is the generalized stale-worktree archived-entity fix for the reported
  Spacedock shape: worktree branches are mirrored copies, archive moves preserve
  the entity slug, and stale files under `.worktrees/*/<workflow>` no longer
  resurrect after the root file moves to `_archive/`.
- `059` keeps active worktree overlays intact. When the root entity still exists
  and the worktree body differs, root frontmatter remains authoritative while
  the worktree body, `worktree_source`, and `main_body` provide preview/diff
  data.
- `059` preserves legitimate worktree-only behavior when no matching root active
  slug and no matching root archived slug exists.

Remaining gap if the `063` wording is interpreted literally: `059` suppresses
by root archive slug, not by a parsed archived entity `id` with a different
filename. That is acceptable for the current bug because Spacedock archive moves
preserve the entity slug and because slug/path is already the worktree merge
identity. Do not add ID-based suppression in this task unless a real archive
rename case is reported; doing so would require extra archive-frontmatter parsing
in the active snapshot path and a new decision about malformed archived files.

Lowest-layer coverage plan for closure:

1. Reuse the `059` parser regressions as the primary proof:
   `archived_main_slug_suppresses_stale_worktree_copy` proves root archive
   placement blocks a stale worktree copy from active scope, and
   `malformed_archived_slug_still_suppresses_stale_worktree_copy` proves the
   guard is path/slug based rather than dependent on successful archive parsing.
2. Reuse existing active-overlay parser coverage:
   `worktree_divergent_keeps_main_frontmatter_and_records_main_body` proves root
   frontmatter remains authoritative while worktree body/diff data is retained.
   If verify wants tighter coverage for this task, extend that test to assert
   `source`, `worktree`, `issue`, and `pr` are also retained from root.
3. Reuse the `059` app regression:
   `archive_move_reload_removes_stale_worktree_copy_from_active_scope` proves an
   app reload removes the stale worktree copy from active scope and the same
   reload exposes the moved task in archived scope.
4. Reuse existing worktree-only parser coverage:
   `worktree_only_item_has_worktree_source_tag`, `worktree_only_items_shown`,
   and the partial-overlap worktree scan coverage prove worktree-only rows still
   work when they are not present in root active or root archive state.
5. Optional readability-only test, if a reviewer asks for the exact `063`
   scenario: add one parser fixture with root active `0x0c`, root archived
   `0x0a`/`0x0b`, and a `0x0c` worktree containing stale copies of all three.
   Expected active IDs: only `0x0c`; expected archived IDs: `0x0a` and `0x0b`.
   This should be test-only after `059`, with no production code change.

Read-only, lint, and docs decision:

- Read-only safety is preserved because the fix is parser interpretation only:
  it reads root archive paths before merging worktree items and does not delete
  stale files, mutate workflow markdown, broaden `git_sync`, or add write paths.
- `059` verify report records focused parser/app tests, the no-write git guard,
  `cargo test`, and `make lint` passing in the implementation worktree. For
  `063` closure after `059` lands, rerun the focused parser/app tests plus
  `make lint` on `main`; no `cargo test -- --ignored` is needed unless watcher
  production code changes.
- No README or policy update is needed if `059` remains the closure path, because
  legitimate worktree-only semantics are unchanged. If a future task changes
  identity from slug/path to ID-aware suppression, document that semantic change
  with the parser tests in the same patch.

## Stage Report: plan

- DONE: Determine whether task `059` already fixes the generalized stale-worktree archived-entity bug, and name any remaining gap if it does not.
  `059` fixes the current slug-preserving archive-move bug in commit `ff0cc6b`; the only remaining literal gap is ID-only suppression for renamed archive files, which is not recommended without a real case.
- DONE: Plan lowest-layer regression coverage for root active/archive authority, active worktree body overlay, archived scope counts, and legitimate worktree-only behavior.
  See `Lowest-layer coverage plan for closure`; it maps `063` to existing/focused parser and app tests from `059`, with one optional scenario fixture if reviewers want exact-shape proof.
- DONE: Explain read-only safety, `make lint` proof, and docs-impact decision for worktree-only semantics.
  See `Read-only, lint, and docs decision`; no Rust code changed in this plan stage, and `059` remains the implementation/lint proof source until it lands.

### Summary

Compared `063` against the current `059` worktree and found that `059` is the
right implementation vehicle for the reported stale archived worktree copies.
The closure plan avoids duplicate production work, preserves read-only behavior,
and names the only non-current edge case: suppressing by parsed archived ID when
the archive filename slug no longer matches the stale worktree file.
