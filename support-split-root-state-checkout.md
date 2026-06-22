---
title: Support split-root workflows (read entities from the state checkout)
status: verify
source: captain report (session) — spacetop-dev migrated to split-root; task list renders empty
kind: feature
risk: medium
id: 073
started: 2026-06-22T07:39:16Z
worktree: .worktrees/spacedock-ensign-support-split-root-state-checkout
mod-block: merge:pr-merge
pr: pr-merge:75
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

## Implementation Plan (plan stage)

### Spike decision

**No spike needed.** The mechanism is proven: `state: .spacedock-state` is already
a top-level scalar in the real `docs/spacetop-dev/README.md`, the state checkout
already exists with active + `_archive/` entities, and `split_frontmatter` +
`serde_yaml` already parse the README frontmatter. The change is an additive
optional field plus a path-join; no new dependency, no unproven I/O. The
regression test (AC-3) is itself the proof harness.

### Design: definition dir vs entity dir

Introduce one new typed fact — the resolved **entity directory** — and thread it
where entity/archive scans happen, while README parse and `WorkflowDefinition.root`
stay on the definition dir. Resolution lives in the parser (domain-before-UI); no
UI code reads `state:`.

Resolution rule (pure, testable):
- `entity_dir(definition_dir, state) = match state { None | "" | "$inline" => definition_dir, rel => definition_dir.join(rel) }`.

### Step 1 — Parse `state:` in `parser/readme.rs` (parser layer)

- Add `#[serde(default)] state: Option<String>` to `RawWorkflowFrontmatter`
  (`crates/spacetop-core/src/parser/readme.rs:236`).
- Add `pub state: Option<String>` to `WorkflowDefinition`
  (`crates/spacetop-core/src/domain/mod.rs`, the struct with `root`/`stages`),
  and set it from `raw.state` in `parse_workflow_readme` (around `readme.rs:66`).
  `root` stays the README parent (unchanged).
- Update the four `WorkflowDefinition { ... }` literals that construct the struct
  by hand (`app/overview.rs:148` `empty`, `sources.rs:104` test, `index.rs`
  tests, any others) to add `state: None`. `cargo build` will flag each.
- Add a pure unit test: `state:` round-trips (`Some(".spacedock-state")` when
  present, `None` when absent).

### Step 2 — Resolve + thread the entity dir in `parser/snapshot.rs`

- Add a pure helper `pub(crate) fn resolve_entity_dir(definition_dir: &Path, state: Option<&str>) -> PathBuf`
  implementing the rule above (treat `""`/`"$inline"` as single-root).
- In `load_workflow_dir(path, repo_root)` (`crates/spacetop-core/src/parser/snapshot.rs:11`):
  - parse README from `path` (unchanged).
  - `let entity_dir = resolve_entity_dir(path, definition.state.as_deref());`
  - `collect_active_item_paths(&entity_dir)` instead of `path` (line 18).
  - worktree-merge base: pass `&entity_dir` to `merge_worktree_items(...)` (line 38)
    so `archived_slug_exists` consults the entity dir's `_archive/`. The
    `scan_worktrees` strip-prefix stays on `path`/repo_root (definition dir) —
    split-root state entities are not mirrored under `.worktrees`, so this stays
    a no-op for them; do not retarget the worktree scan at the state checkout.
- Add a unit test on `resolve_entity_dir` (relative join; `$inline`/`None`/`""`
  → definition dir).

### Step 3 — Thread the entity dir to archive loading in `sources.rs`

- `WorkflowSources::load_archive(workflow_dir, definition)`
  (`crates/spacetop-core/src/sources.rs:46`) currently joins `_archive` onto
  `workflow_dir`. Change it to resolve the entity dir from
  `definition.root` + `definition.state` (it already has the `definition`) and
  pass that resolved dir to `ArchiveSource::load`. This keeps the call sites in
  `app/overview.rs:602`, `headless.rs:117/200`, and `index.rs` unchanged — they
  keep passing the definition `workflow_dir`; resolution happens inside.
- `archive_dir`/`load_archived_items*` in `parser/archive.rs` stay as-is (they
  already take whatever dir they're handed); only the dir handed in changes.

### Step 4 — No app/lib/watcher signature changes required

- `OverviewState`, `WorkflowIndex::load`, discovery, and the watcher keep
  operating on the definition dir (`DiscoveredWorkflow.root`). The watcher
  watches the definition dir recursively today; for split-root, entity edits land
  in `<definition_dir>/.spacedock-state/`, which IS under the watched dir, so
  refresh still fires. Confirm (not change) this in the plan's verification.
- Definition view (`ui/definition.rs`), tabs, picker all read
  `definition.root` — correct to stay on the definition dir.

### Lowest-practical-layer proof (AC-2/AC-3)

- **Parser/index test (primary).** Add a split-root fixture under
  `tests/fixtures/` (or build it in a `tempdir` in `parser/tests.rs`): a
  `README.md` with `state: state-sub` + `stages`, entity `*.md` files and an
  `_archive/*.md` under `state-sub/`, and NO entity files beside the README.
  Assert `load_workflow_dir(def_dir, repo_root).items` is non-empty AND
  `WorkflowSources::load_archive(def_dir, &definition).entities` is non-empty.
  This test fails against `main` (which scans the README dir and finds zero).
- **Single-root regression guard.** Keep an existing single-root fixture test
  green (state absent → entities beside README still load) — the many existing
  `load_workflow_dir(&wf, &root)` tests in `parser/tests.rs` already cover this;
  add one explicit `state: $inline` case asserting identical behavior to absent.
- **UI render assertion (only if needed).** The empty-list regression is fully
  pinned at the parser/index layer above, so a `TestBackend` test is optional.
  Add one only if review wants the list-rows render proven; the existing
  `ui/tests.rs` `from_sources` harness can host it without new scaffolding.

Exact commands (run all before completion):
- `cargo fmt`
- `cargo test` (parser, index, app, ui suites)
- `make lint` (`cargo clippy --all-targets --all-features -- -D warnings`)
- Manual smoke (evidence, not a gate): `cargo run -p spacetop -- --workflow-dir docs/spacetop-dev`
  must now show the split-root task list instead of empty.

### Docs to update in the same change

- `README.md` "Current Product Shape": note that entity/archive loading resolves
  the README `state:` field (relative path → state checkout dir; `$inline`/absent
  → entities beside the README).
- `AGENTS.md` "Workflow Parsing Rules": add the `state:`-resolution contract; and
  "Code Map" for `parser/snapshot.rs` / `parser/readme.rs` to mention entity-dir
  resolution. No `docs/code-review-policy.md` change needed.

### Owned files

- `crates/spacetop-core/src/parser/readme.rs` (parse `state:`)
- `crates/spacetop-core/src/domain/mod.rs` (`WorkflowDefinition.state`)
- `crates/spacetop-core/src/parser/snapshot.rs` (`resolve_entity_dir`, thread)
- `crates/spacetop-core/src/sources.rs` (`load_archive` resolves entity dir)
- `crates/spacetop-core/src/parser/tests.rs` (split-root + `$inline` fixtures)
- struct-literal callers needing `state: None` (build-flagged)
- `README.md`, `AGENTS.md` (docs)

### Read-only / Clean Code guardrails

- No writes added; the change only redirects which directory entities are READ
  from. `no_write_git_calls.rs` and `no_terminal_deps.rs` guardrails are
  untouched and must stay green. `state:` resolution is a pure helper with unit
  tests; UI code never parses it.

## Stage Report: plan

- DONE: Produce a step-by-step implementation plan for the definition-dir vs entity-dir split (parse `state:` in readme.rs, resolve in snapshot.rs, thread to collect_active_item_paths / archive_dir/load_archive / worktree-merge base; keep WorkflowDefinition.root/discovery on definition dir) with exact files, functions, signature changes
  Steps 1-4 above name each file/function: `RawWorkflowFrontmatter.state`, `WorkflowDefinition.state`, `resolve_entity_dir` in snapshot.rs, `load_archive` resolving internally; worktree scan deliberately left on the definition dir.
- DONE: Specify the lowest-practical-layer proof: split-root fixture + parser/index tests asserting non-empty active AND archived (must fail against main), single-root unchanged, optional TestBackend; name exact commands
  Proof section pins the parser/index split-root fixture test as primary (fails on main), adds a `$inline` single-root guard, marks the `TestBackend` test optional; commands listed: `cargo fmt`, `cargo test`, `make lint`, plus a manual smoke.
- DONE: Identify docs updates in the same change (README "Current Product Shape" + AGENTS.md "Workflow Parsing Rules"/Code Map) and state spike need
  Docs section lists README + AGENTS.md edits; spike decision recorded as "no spike needed" with the proven mechanism (existing `state:` scalar + serde_yaml + path-join).

### Summary

Plan keeps the change additive and parser-local: a new optional `state:` field on
the README frontmatter and `WorkflowDefinition`, a pure `resolve_entity_dir`
helper, and re-pointing the active-item scan, archive load, and worktree-merge
base at the resolved entity dir while README parse, discovery, the watcher, and
`WorkflowDefinition.root` stay on the definition dir. No spike needed; the
split-root fixture parser/index test is the proof and fails against `main`. App,
lib, and watcher need no signature changes because resolution happens inside the
parser/sources layer.

## Stage Report: implement

- DONE: Implement the parser change per the plan: add optional state: to RawWorkflowFrontmatter and WorkflowDefinition, add the pure resolve_entity_dir helper, and thread the resolved entity dir to collect_active_item_paths, load_archive, and the worktree-merge base — keeping README parse, discovery, watcher, and WorkflowDefinition.root on the definition dir
  `readme.rs` parses `state:`; `domain/mod.rs` carries `WorkflowDefinition.state`; `snapshot.rs::resolve_entity_dir` (pure) threads to `collect_active_item_paths` + `merge_worktree_items` base; `sources.rs::load_archive` resolves internally. Commit f3b5d70.
- DONE: Add the split-root fixture parser/index test asserting non-empty active AND archived items (must fail against main) plus a $inline single-root guard; keep existing single-root fixtures green
  `split_root_loads_active_and_archived_from_state_checkout` (asserts non-empty active + archived; fails on main) and `inline_state_keeps_single_root_behavior` added in `parser/tests.rs`; plus `resolve_entity_dir` and `state:` round-trip unit tests. All 373 core + spacetop suites green.
- DONE: Run and record reproducible evidence (cargo fmt, cargo test, make lint) and update README "Current Product Shape" + AGENTS.md (Workflow Parsing Rules and Code Map) in the same change
  `cargo fmt --check` clean, `cargo test` all suites pass, `make lint` (clippy -D warnings) clean, `no_terminal_deps`/`no_write_git_calls` guardrails pass. README "Status" + AGENTS.md "Workflow Parsing Rules"/"Code Map" updated in the same commit.

### Summary

Split the definition dir from the entity dir behind a new optional README
`state:` field, resolved by one pure `resolve_entity_dir` helper that the active
scan, worktree-merge base, and archive load all consult; discovery, the watcher,
and `WorkflowDefinition.root` stay on the definition dir. A declared-but-absent
state checkout now renders empty rather than erroring, which is also why
real-fixture entity/archive tests were repointed onto synthetic single-root
fixtures (the migrated `docs/spacetop-dev` keeps its entities/archive only in the
state checkout, absent from code worktrees). Headless `export` against the real
split-root workflow loads 2 active + 70 archived entities, proving the
empty-list regression is fixed; the same export on a code worktree without the
state checkout returns 0/0 without crashing.

## Stage Report: verify

- DONE: Independently re-run the verification commands against the worktree branch (cargo fmt --check, cargo test, make lint) and record actual output — do not trust the implement report's claims
  Re-ran on branch f3b5d70 in the worktree: `cargo fmt --check` exit 0; `cargo test` all suites green (199 passed, 0 failed, 3 watcher tests ignored-by-design); `make lint` (clippy --all-targets --all-features -D warnings) exit 0.
- DONE: Confirm every acceptance criterion (AC-1..AC-6) has concrete evidence: AC-3 split-root fixture test genuinely fails against main and passes on the branch; AC-4 single-root/$inline behavior unchanged; state: parsed in the parser layer (not UI); docs (README + AGENTS.md) updated in the same change
  AC-1: `state:` parsed into `RawWorkflowFrontmatter`+`WorkflowDefinition.state` in parser; no spacetop UI code reads it (grep confirms only unrelated `state` field names). AC-2: `resolve_entity_dir` unit tests (relative→join; None/""/"  "/"$inline"→def dir). AC-3: branch test `split_root_loads_active_and_archived_from_state_checkout` passes; a temporary repro built against main's API rendered 0 active/0 archived for the same layout (then removed, uncommitted) — fail-on-main proven behaviorally, not just by inference. AC-4: `inline_state_keeps_single_root_behavior` passes; existing single-root suite green. AC-5: fmt/test/lint all pass. AC-6: README "Status" + AGENTS.md "Workflow Parsing Rules"/"Code Map" updated in commit f3b5d70.
- DONE: Render a verification verdict (PASSED or REJECTED) with any defects or missing evidence, and confirm read-only/git/config contracts and the no_terminal_deps/no_write_git_calls guardrails are intact
  Verdict PASSED. Guardrails `no_terminal_deps` (1/1) and `no_write_git_calls` (2/2) green; change adds no writes, no git-write subcommands, no config/session path changes. One Low defect: `domain/mod.rs:101` doc comment links `[Self::entity_root]`, a method that does not exist (resolver is the free fn `resolve_entity_dir`); broken intra-doc link, non-blocking (clippy does not run rustdoc so lint stays green).

### Summary

End-to-end verified. The branch-built binary loads 2 active + 70 archived
entities from the real split-root `docs/spacetop-dev` (matching on-disk counts,
`definition.state=.spacedock-state`); the main-built binary renders 0/0 for the
same workflow — the reported empty-list regression, now fixed. All six ACs have
concrete evidence; fmt/test/lint and both safety guardrails pass. One Low,
non-blocking defect: a broken `[Self::entity_root]` rustdoc link in
`domain/mod.rs` (should point at `resolve_entity_dir`). Verdict: PASSED.
