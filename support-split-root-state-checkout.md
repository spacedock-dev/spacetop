---
title: Support split-root workflows (read entities from the state checkout)
status: shape
source: captain report (session) — spacetop-dev migrated to split-root; task list renders empty
kind: feature
risk: medium
id: 073
started: 2026-06-22T07:39:16Z
---

## Problem statement

After spacetop-dev migrated to a split-root state backend, spacetop renders an
empty task list. The README declares `state: .spacedock-state`; active entities
and `_archive/` now live in the state checkout (`docs/spacetop-dev/.spacedock-state/`),
not beside the README. spacetop assumes the README directory and the entity
directory are the same directory, so it scans the README directory, finds only
`README.md`/`_mods`/`recce.yml`, and shows nothing.

## Target outcome

A user opening or discovering a split-root workflow sees its active and archived
entities — the same view they got before the migration. Single-root workflows
(entities beside the README) continue to work unchanged.

## Where the single-root assumption is baked in

The README (definition) directory and the entity (state) directory are the same
`Path` everywhere today. Discovery finds the README dir; everything downstream
reuses that one path as both the definition source and the entity scan root.

- `crates/spacetop-core/src/parser/snapshot.rs` — `load_workflow_dir(path, repo_root)`:
  `path` is used both for `parse_workflow_readme(path.join("README.md"))` (line 12)
  AND for `collect_active_item_paths(path)` (line 18). This is the core conflation.
- `crates/spacetop-core/src/parser/archive.rs` — `archive_dir(workflow_dir)` =
  `workflow_dir.join("_archive")`; `load_archived_items*` scans relative to the
  passed dir, which today is the README dir.
- `crates/spacetop-core/src/parser/readme.rs` — `RawWorkflowFrontmatter` has no
  `state:` field, so the declaration is invisible. `WorkflowDefinition.root` is
  set to the README's parent (line 67).
- `crates/spacetop-core/src/sources.rs` — `WorkflowSources::load_active` /
  `load_archive` and `WorkingTreeSource` / `ArchiveSource` thread one
  `workflow_dir` to both readme and entity loads.
- `crates/spacetop-core/src/index.rs` — `WorkflowIndex::load(workflow_dir, repo_root)`
  passes the discovered README dir straight through.
- `crates/spacetop/src/app/overview.rs` — `OverviewState::load(workflow_dir)`,
  `empty`, refresh, and `load_archive` (line 602) all use the single
  `workflow_dir` (the discovered README dir). The watcher also watches this dir.
- `crates/spacetop/src/headless.rs` and `crates/spacetop/src/lib.rs` — discovery
  yields `DiscoveredWorkflow.root` (the README dir) and feeds it as `workflow_dir`.

The fix introduces a distinction the codebase does not currently have: a
**definition directory** (where `README.md` lives, what discovery returns) and an
**entity directory** (where `*.md` entities and `_archive/` are read). Today they
are forced equal; split-root makes them differ.

## The `state:` field contract spacetop must honor

Read `state:` from README frontmatter in `parse_workflow_readme` (a new optional
field on `RawWorkflowFrontmatter`) and resolve it to the entity directory:

- `state:` is a **relative path** (e.g. `.spacedock-state`): the entity directory
  is `definition_dir.join(state)`. Active entities and `_archive/` are read from
  there. The README/definition is still read from `definition_dir`.
- `state:` is `$inline`, or **absent/empty**: single-root. The entity directory
  equals the definition directory (today's behavior, unchanged).
- Resolution is **relative to the definition (workflow/README) directory**, never
  to the repo root or cwd. An absolute `state:` is out of scope for this task
  (treat as relative-join; do not special-case unless a fixture demands it).

Implementation seam: keep discovery returning the definition dir (README-based,
unchanged). Resolve `state:` once when loading a snapshot, and pass the resolved
**entity dir** to the entity/archive scans while continuing to pass the
**definition dir** to the README parse. Smallest viable change: have
`load_workflow_dir` parse the README first, compute the entity dir from
`definition + state`, and point `collect_active_item_paths` + the worktree merge
base at the entity dir; thread the entity dir to `load_archive` the same way
(carry it on the definition/snapshot or recompute it from the definition root +
state). `WorkflowDefinition.root` should remain the definition dir so the
definition view and discovery keep working.

## Worktree-merge and archive interaction (resolved)

- **Worktree merge** (`.worktrees/*/<workflow>`, `scan_worktrees` in
  `parser/worktree.rs`): keyed off `path.strip_prefix(repo_root)` in
  `snapshot.rs:33`. For a split-root workflow the entities are NOT mirrored under
  `.worktrees/<task>/<workflow>` (worktree-stage deliverables live in the code
  worktree; entity state lives in the shared state checkout per the split-root
  contract). Scope decision: **the worktree-merge scan stays anchored on the
  definition dir's repo-relative path and is effectively a no-op for split-root
  entities** (no entity `*.md` under the code-branch workflow dir to mirror).
  Do not try to merge `.worktrees` copies of state entities. The
  `archived_slug_exists` check inside the merge must consult the resolved entity
  dir's `_archive/`, not the definition dir's.
- **`_archive/` loading**: `archive_dir` must be computed from the resolved
  **entity dir** (`entity_dir.join("_archive")`), so split-root archives load
  from `.spacedock-state/_archive/` and single-root archives load from
  `definition_dir/_archive/` as before.
- **Discovery is unaffected**: the state checkout has no `README.md`, so it is
  never itself discovered as a workflow. The `.spacedock-state` dir is a git
  worktree (its `.git` is a gitdir file), but discovery matches on README
  frontmatter, so it is correctly ignored. No new prune entry is required.

## Scope boundaries (non-goals)

- No writes to the state checkout. spacetop stays read-only; this task only
  changes which directory entities are read FROM.
- No change to the `Y` git-sync path, the watcher write surface, or config/session
  path rules.
- No multi-state or per-stage state roots; one `state:` per workflow.
- Absolute `state:` paths and `state:` pointing outside the repo are out of scope.

## Acceptance criteria

1. README frontmatter `state:` is parsed into typed definition/snapshot state in
   `spacetop-core` (parser layer), not inferred from strings in UI code.
2. A relative `state:` resolves the entity directory to `definition_dir.join(state)`;
   `$inline`/absent keeps the entity directory equal to the definition directory.
3. **Regression (primary proof):** a split-root workflow fixture (README with
   `state: <subdir>` + entities and `_archive/` under `<subdir>`) renders its
   active AND archived entities. A parser/index test on this fixture must assert
   non-empty active items and non-empty archived items where the equivalent
   single-root layout would render them today — and must fail against `main`
   (which renders empty). Add a `TestBackend` render assertion if the empty-list
   regression is best pinned at the UI layer.
4. The existing single-root fixtures and the real `docs/spacetop-dev` definition
   view continue to pass unchanged (no behavior change when `state:` is absent).
5. `cargo fmt`, `cargo test`, and `make lint` (clippy `-D warnings`) pass.
6. Docs (README "Current Product Shape" + AGENTS.md "Workflow Parsing Rules" /
   Code Map) updated in the same change to describe `state:`-resolved entity
   loading.

## Risk and milestone

- **Risk: medium** (confirmed). It touches the parser/index loading spine that
  all rendering reads from, but the change is additive and gated on a new optional
  field; single-root behavior is preserved by the absent-field default.
- **Product contract touched:** parser/discovery (entity-dir resolution),
  domain/index (typed `state:`), TUI rendering (active + archived list), and docs.
  No git-write, watcher-write, or config/session contract changes.

## Stage Report: shape

- DONE: Locate where spacetop bakes in the single-root assumption — how discovery/parser resolve the entity directory vs the README (definition) directory — and name the modules/functions that must change to read entities from a state checkout
  Named the conflation in `parser/snapshot.rs` `load_workflow_dir` (one `path` used for both README parse and `collect_active_item_paths`) plus `parser/archive.rs`, `parser/readme.rs`, `sources.rs`, `index.rs`, `app/overview.rs`, `headless.rs`/`lib.rs` in the spec.
- DONE: Define the state: field contract spacetop must honor: a relative path resolves to the state checkout dir (entities + _archive live there), $inline or absent means single-root (entities beside the README); resolve the checkout relative to the workflow/definition dir
  Spec section "The `state:` field contract" pins relative→`definition_dir.join(state)`, `$inline`/absent→single-root, resolution relative to the definition dir; absolute paths out of scope.
- DONE: Write acceptance criteria including the concrete regression — a split-root workflow fixture must render its active and archived entities (today it renders empty) — and confirm risk level and milestone
  Six ACs written; AC-3 is the failing-on-`main` split-root fixture regression (non-empty active + archived). Risk confirmed medium; product contract touched = parser/discovery, domain/index, TUI, docs (no git/watcher/config changes).

### Summary

Mapped the single-root assumption to its root cause: `load_workflow_dir` uses one
directory as both the definition (README) dir and the entity-scan dir, and
`state:` is never parsed. Shape proposes a definition-dir vs entity-dir split
resolved from a new optional `state:` field, keeping discovery and single-root
behavior unchanged. Resolved the two open questions from the draft: the
`.worktrees` merge is a no-op for split-root state entities (anchor stays on the
definition dir), and `_archive/` must load from the resolved entity dir. No
implementation here — this is a read-only shaping pass.
