---
id: 064
title: Centralize entity identity module
status: implement
source: Architecture refactor scan on 2026-06-17; implement using Ponytail mode
kind: refactor
risk: medium
milestone: v2-later
proof: cargo test -p spacetop-core entity_identity && cargo test -p spacetop app::tests && make lint
started: 2026-06-17T02:33:11Z
completed:
verdict:
score: 0.86
worktree: .worktrees/spacedock-ensign-centralize-entity-identity
issue:
pr:
---

Create a focused refactor that centralizes workflow entity identity rules so parser, worktree merge, index lookup, and TUI selection stop re-implementing slug derivation independently.

Implementation must use Ponytail mode. If the Ponytail plugin/tooling is unavailable in the implementing session, the worker must stop and report that blocker instead of silently proceeding in another mode.

## Scope

- Kind: refactor
- Risk: medium
- Milestone: v2-later
- Touches: parser / app-state / core index
- Non-goals: do not split the Cargo workspace; do not split ui/graph.rs; do not refactor watcher.rs; do not broaden workflow write behavior.

## Acceptance criteria

**AC-1 -- Entity identity has one core-owned interface.**
Verified by: parser item parsing, worktree merge, WorkflowIndex lookup, and OverviewState selection all call the same core identity helper/module for flat `{slug}.md` and folder-form `{slug}/index.md` paths.

**AC-2 -- Existing behavior is preserved for active, archived, and worktree-sourced tasks.**
Verified by: focused tests covering flat files, folder-form `index.md`, archived slug checks, worktree-only items, and selection preservation after reload/sort.

**AC-3 -- TUI code no longer owns workflow schema identity rules.**
Verified by: no TUI-local slug derivation duplicate remains in `crates/spacetop/src/app/overview.rs`; app tests still pass through the public app interface.

**AC-4 -- Read-first product contract remains unchanged.**
Verified by: no new workflow markdown write path is introduced, and `crates/spacetop-core/tests/no_write_git_calls.rs` plus `make lint` pass.

**AC-5 -- Implementation used Ponytail mode.**
Verified by: the stage report explicitly names Ponytail mode/tooling used, or records a blocker if Ponytail mode was unavailable.

## Proof plan

- Lowest test layer: core identity/parser/worktree/index tests first; app tests only for selection behavior.
- Required command: `cargo test -p spacetop-core`, `cargo test -p spacetop app::tests`, and `make lint`.
- Manual check, if any: none expected unless tests reveal a TUI-only selection edge case.
- Docs/policy update needed: update nearby comments only if identity naming changes; no README product behavior change expected.

## Implementation Plan

### Ponytail constraint

Use Ponytail full mode: one small core helper module, no new dependency, no trait, no workspace split, no UI-side abstraction. If this plan starts to grow beyond centralizing path identity and migrating current callers, stop and split follow-up work.

### Core interface

Add `crates/spacetop-core/src/entity_identity.rs` and export it from `crates/spacetop-core/src/lib.rs`.

The module should own exactly the workflow entity path identity rules:

- `pub fn entity_slug(path: &Path) -> Option<String>`
  - flat item: `{slug}.md` -> `slug`
  - folder-form item: `{slug}/index.md` -> `slug`
- `pub(crate) fn entity_slug_os(path: &Path) -> Option<OsString>` if `parser::worktree` still needs allocation-free `OsString` keys for archive path joins.
- `pub(crate) fn archived_entity_paths(archive_root: &Path, slug: &OsStr) -> (PathBuf, PathBuf)` or the smallest equivalent private helper if it keeps `archived_slug_exists` from rebuilding path identity rules inline.

Do not move frontmatter `id` fallback, stage validation, archive loading policy, worktree root discovery, or TUI selection state into this module.

### Call sites to migrate

- `crates/spacetop-core/src/parser/worktree.rs`
  - Replace local `slug_of_path` with `entity_identity::entity_slug_os` or a local `OsString` conversion around `entity_slug`.
  - Use the helper in `merge_worktree_items`, result sorting, and `archived_slug_exists`.
  - Keep SHA-1 comparison, main-frontmatter/worktree-body merge semantics, `.worktrees`/`.claude/worktrees` scan order, and stale archived suppression behavior unchanged.
- `crates/spacetop-core/src/index.rs`
  - Replace local `slug_of` in `rebuild_lookup_maps` with `entity_identity::entity_slug`.
  - Delete the local duplicate helper.
  - Keep `entity_by_slug` behavior and active/archived map precedence unchanged.
- `crates/spacetop/src/app/overview.rs`
  - Import `spacetop_core::entity_identity::entity_slug`.
  - Replace app-local `slug_of` in `reload_from_index` and `cycle_sort_mode`.
  - Delete the TUI-local slug derivation helper and update nearby comments from "file stem" to "core entity slug".
- `crates/spacetop-core/src/parser/snapshot.rs`
  - Add folder-form active item collection only if current tests show active `{slug}/index.md` is not loaded. Keep `_mods`, `_archive`, README, and nested non-item exclusions unchanged.
- `crates/spacetop-core/src/parser/archive.rs`
  - Keep folder-form archive loading, but add/adjust tests so it is explicitly covered by the shared identity helper.

### Non-goals

- Do not split the Cargo workspace or add a crate.
- Do not split `crates/spacetop/src/ui/graph.rs`.
- Do not refactor `crates/spacetop-core/src/watcher.rs`.
- Do not broaden workflow write behavior, git write commands, editor behavior, config/session paths, or sync behavior.
- Do not change entity display IDs or `id-style: slug` parsing unless a failing identity test proves the current behavior is coupled to path slug derivation.
- Do not replace `WorkflowIndex` query APIs or make the TUI inspect raw schema vectors.

### Proof strategy

- AC-1, one core-owned interface:
  - Add `crates/spacetop-core/src/entity_identity.rs` unit tests for flat paths, folder-form `index.md`, missing filenames, and non-UTF-8-safe lossy behavior matching current `to_string_lossy` callers.
  - Add a source grep check during review: no local `fn slug_of`, `fn slug_of_path`, or direct `file_stem() == "index"` identity helper remains in `crates/spacetop/src/app/overview.rs`, `crates/spacetop-core/src/index.rs`, or `crates/spacetop-core/src/parser/worktree.rs`.
- AC-2, preserve active/archive/worktree behavior:
  - Parser tests in `crates/spacetop-core/src/parser/tests.rs`:
    - active flat file still loads;
    - active folder-form `{slug}/index.md` loads if currently unsupported and this plan adds it;
    - archived flat file and archived folder-form suppress stale worktree copies;
    - worktree-only flat file remains visible and tagged with `worktree_source`;
    - worktree-only folder-form `{slug}/index.md` is covered only if active folder-form support is added.
  - Index tests in `crates/spacetop-core/src/index.rs`:
    - `WorkflowIndex::entity_by_slug` resolves flat and folder-form entities from active and archived snapshots.
- AC-3, no TUI-owned schema identity:
  - App tests in `crates/spacetop/src/app/tests.rs`:
    - `reload_from_index_preserves_selection_by_slug` still passes for flat files;
    - add a folder-form selection-preservation case using public `OverviewState` construction or reload APIs.
  - Keep selection fallback by clamped index when slug disappears.
- AC-4, read-first contract:
  - Run `cargo test -p spacetop-core --test no_write_git_calls`.
  - Confirm no production code adds filesystem writes beyond existing config/session and `git pull --ff-only` sync paths.
  - Run `make lint`.
- AC-5, Ponytail proof:
  - The implement stage report must state that Ponytail full mode was available and used, or stop and report it unavailable.

### Required commands

```bash
cargo test -p spacetop-core entity_identity
cargo test -p spacetop-core parser::tests
cargo test -p spacetop-core index::tests
cargo test -p spacetop-core --test no_write_git_calls
cargo test -p spacetop app::tests
make lint
```

If the implementer changes Rust code outside the listed owned files, they must name the reason in the implement stage report.

## Stage Report: plan

- DONE: Plan names the exact core entity-identity interface/module, all call sites to migrate, and the non-goals that must remain untouched.
  The plan adds `crates/spacetop-core/src/entity_identity.rs`, exports it from core, and migrates `parser/worktree.rs`, `index.rs`, and `app/overview.rs` away from their duplicate slug derivation helpers. It also names the conditional parser collection checks for active/archive folder-form items and explicitly preserves the workspace shape, watcher, UI graph, git/write, sync, editor, config/session, and ID parsing boundaries.
- DONE: Proof strategy maps each acceptance criterion to the lowest practical test layer and required commands.
  AC-1 is covered by core identity unit tests plus duplicate-helper grep review, AC-2 by parser and index tests, AC-3 by app selection tests through `OverviewState`, AC-4 by `no_write_git_calls` and `make lint`, and AC-5 by the implement report naming Ponytail full mode or stopping as blocked.
- DONE: Ponytail mode is explicitly used for planning, or the stage report records Ponytail mode/tooling as unavailable and stops without substituting another mode.
  Ponytail full mode was available in this Codex session via `/Users/kent/.codex/plugins/cache/ponytail/ponytail/4.7.0/skills/ponytail/SKILL.md` and used to keep the plan to one small core module plus direct caller migration.

### Summary

Created a focused implementation handoff for centralizing entity identity in core. The shortest safe path is one exported `entity_identity` helper module, deleting duplicate slug derivation from worktree merge, index lookup, and TUI selection, with parser/index/app tests proving flat, folder-form, archived, worktree-only, reload, and read-only behavior.
